use crate::game::stone::{Acceleration, Position, Velocity};
use crate::unit;
use crate::unit::{seconds, Time};
use crate::vector::{EuclideanNorm, Vector2};
use derive_more::{Add, Sub};
use roots::{find_roots_linear, find_roots_quadratic, find_roots_quartic, Roots};
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;
use uom::si::velocity::foot_per_second;

#[derive(Clone, Copy, Debug, Default, Add, Sub)]
pub struct Uniform {
    pub s0: Position,
    pub v: Velocity,
}

impl Uniform {
    pub fn when_at_x(&self, x: unit::Length) -> Option<Time> {
        Self::when_at_position(self.s0.x, self.v.x, x)
    }

    pub fn when_at_y(&self, y: unit::Length) -> Option<Time> {
        Self::when_at_position(self.s0.y, self.v.y, y)
    }

    fn when_at_position(s0: unit::Length, v: unit::Velocity, s: unit::Length) -> Option<Time> {
        let a1 = v.get::<foot_per_second>();
        let a0 = (s0 - s).get::<foot>();
        let roots = find_roots_linear(a1, a0);
        min_non_negative_root(roots).map(seconds)
    }
}

#[derive(Clone, Copy, Debug, Default, Add, Sub)]
pub struct UniformlyAccelerated {
    pub s0: Position,
    pub v0: Velocity,
    pub a: Acceleration,
}

impl UniformlyAccelerated {
    pub fn position(&self, t: Time) -> Position {
        self.s0 + self.v0 * t + self.a * t * t / 2.0
    }

    pub fn velocity(&self, t: Time) -> Velocity {
        self.v0 + self.a * t
    }

    pub fn when_at_x(&self, x: unit::Length) -> Option<Time> {
        Self::when_at_position(self.s0.x, self.v0.x, self.a.x, x)
    }

    pub fn when_at_y(&self, y: unit::Length) -> Option<Time> {
        Self::when_at_position(self.s0.y, self.v0.y, self.a.y, y)
    }

    fn when_at_position(
        s0: unit::Length,
        v0: unit::Velocity,
        a: unit::Acceleration,
        s: unit::Length,
    ) -> Option<Time> {
        let a2 = (a / 2.0).get::<foot_per_second_squared>();
        let a1 = v0.get::<foot_per_second>();
        let a0 = (s0 - s).get::<foot>();
        let roots = find_roots_quadratic(a2, a1, a0);
        min_non_negative_root(roots).map(seconds)
    }

    pub fn when_enters_circle(&self, r: unit::Length) -> Option<Time> {
        let a = self.a.map(|a| a.get::<foot_per_second_squared>());
        let v0 = self.v0.map(|v| v.get::<foot_per_second>());
        let s0 = self.s0.map(|s| s.get::<foot>());
        let r = r.get::<foot>();
        let a4 = (a.x * a.x + a.y * a.y) / 4.0;
        let a3 = v0.x * a.x + v0.y * a.y;
        let a2 = v0.x * v0.x + v0.y * v0.y + s0.x * a.x + s0.y * a.y;
        let a1 = 2.0 * (v0.x * s0.x + v0.y * s0.y);
        let a0 = s0.x * s0.x + s0.y * s0.y - r * r;
        let roots = find_roots_quartic(a4, a3, a2, a1, a0);
        roots
            .as_ref()
            .iter()
            .filter(|&&t| {
                let derivative = 4.0 * a4 * t * t * t + 3.0 * a3 * t * t + 2.0 * a2 * t + a1;
                t >= 0.0 && derivative < 0.0
            })
            .copied()
            .next()
            .map(seconds)
    }
}

impl From<Uniform> for UniformlyAccelerated {
    fn from(uniform: Uniform) -> Self {
        Self { s0: uniform.s0, v0: uniform.v, a: Vector2::default() }
    }
}

fn min_non_negative_root(roots: Roots<f32>) -> Option<f32> {
    roots.as_ref().iter().filter(|&&t| t >= 0.0).copied().next()
}

pub fn velocity_after_collision(
    s_a: Position,
    v_a: Velocity,
    s_b: Position,
    v_b: Velocity,
) -> (Velocity, Velocity) {
    let offset = dbg!(s_b - s_a);
    let hit_dir = dbg!(offset / offset.norm());
    let v_given_by_a = dbg!(hit_dir * dbg!(v_a.dot(hit_dir)));
    let v_given_by_b = dbg!(-hit_dir * dbg!(v_b.dot(-hit_dir)));
    let new_v_a = dbg!(v_a + v_given_by_b - v_given_by_a);
    let new_v_b = dbg!(v_b + v_given_by_a - v_given_by_b);
    (new_v_a, new_v_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{assert_approx_eq, feet, feet_per_second, feet_per_second_squared};

    #[test]
    fn when_at_position_uniform() {
        let s_near = feet(5.0);
        let s_far = feet(20.0);
        let s0 = feet(10.0);

        let run_case = |case: (unit::Velocity, Option<Time>, Option<Time>)| {
            let (v, expect_near, expect_far) = case;
            let motion_x = Uniform {
                s0: Vector2 { x: s0, y: unit::Length::default() },
                v: Vector2 { x: v, y: unit::Velocity::default() },
            };
            let motion_y = Uniform {
                s0: Vector2 { x: unit::Length::default(), y: s0 },
                v: Vector2 { x: unit::Velocity::default(), y: v },
            };
            if let Some(expect_near) = expect_near {
                assert_approx_eq!(motion_x.when_at_x(s_near).unwrap(), expect_near);
                assert_approx_eq!(motion_y.when_at_y(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion_x.when_at_x(s_near), None);
                assert_eq!(motion_y.when_at_y(s_near), None);
            }
            assert_approx_eq!(motion_x.when_at_x(s0).unwrap(), seconds(0.0));
            assert_approx_eq!(motion_y.when_at_y(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far {
                assert_approx_eq!(motion_x.when_at_x(s_far).unwrap(), expect_far);
                assert_approx_eq!(motion_y.when_at_y(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion_x.when_at_y(s_far), None);
                assert_eq!(motion_y.when_at_y(s_far), None);
            }
        };

        for case in [
            (feet_per_second(1.0), None, Some(seconds(10.0))),
            (feet_per_second(-1.0), Some(seconds(5.0)), None),
            (feet_per_second(0.0), None, None),
        ] {
            run_case(case);
        }
    }

    #[test]
    fn when_at_position_uniformly_accelerated() {
        let s_near = feet(0.0);
        let s_far = feet(20.0);
        let s0 = feet(10.0);

        let run_case = |case: (unit::Velocity, unit::Acceleration, Option<Time>, Option<Time>)| {
            let (v0, a, expect_near, expect_far) = case;
            let motion_x = UniformlyAccelerated {
                s0: Vector2 { x: s0, y: unit::Length::default() },
                v0: Vector2 { x: v0, y: unit::Velocity::default() },
                a: Vector2 { x: a, y: unit::Acceleration::default() },
            };
            let motion_y = UniformlyAccelerated {
                s0: Vector2 { x: unit::Length::default(), y: s0 },
                v0: Vector2 { x: unit::Velocity::default(), y: v0 },
                a: Vector2 { x: unit::Acceleration::default(), y: a },
            };
            if let Some(expect_near) = expect_near {
                assert_approx_eq!(motion_x.when_at_x(s_near).unwrap(), expect_near);
                assert_approx_eq!(motion_y.when_at_y(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion_x.when_at_x(s_near), None);
                assert_eq!(motion_y.when_at_y(s_near), None);
            }
            assert_approx_eq!(motion_x.when_at_x(s0).unwrap(), seconds(0.0));
            assert_approx_eq!(motion_y.when_at_y(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far {
                assert_approx_eq!(motion_x.when_at_x(s_far).unwrap(), expect_far);
                assert_approx_eq!(motion_y.when_at_y(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion_x.when_at_y(s_far), None);
                assert_eq!(motion_y.when_at_y(s_far), None);
            }
        };
        use feet_per_second as feet_ps;
        use feet_per_second_squared as feet_pss;

        for case in [
            (feet_ps(1.0), feet_pss(0.2), None, Some(seconds(5.0 * (5.0_f32.sqrt() - 1.0)))),
            (feet_ps(1.0), feet_pss(-0.2), Some(seconds(5.0 * (5.0_f32.sqrt() + 1.0))), None),
            (feet_ps(-1.0), feet_pss(-0.2), Some(seconds(5.0 * (5.0_f32.sqrt() - 1.0))), None),
            (feet_ps(-1.0), feet_pss(0.2), None, Some(seconds(5.0 * (5.0_f32.sqrt() + 1.0)))),
            (
                feet_ps(1.0),
                feet_pss(-0.05),
                Some(seconds(20.0 * (2.0_f32.sqrt() + 1.0))),
                Some(seconds(20.0)),
            ),
        ] {
            run_case(case);
        }
    }

    #[test]
    fn when_enters_circle() {
        let run_case =
            |(s0, v0, a, r, expected_t): ((f32, f32), (f32, f32), (f32, f32), f32, Option<f32>)| {
                let motion = UniformlyAccelerated {
                    s0: Vector2::from(s0).map(feet),
                    v0: Vector2::from(v0).map(feet_per_second),
                    a: Vector2::from(a).map(feet_per_second_squared),
                };
                let result = motion.when_enters_circle(feet(r));
                if let Some(expected_t) = expected_t {
                    assert_approx_eq!(result.unwrap(), seconds(expected_t), ulps = 6);
                } else {
                    assert_eq!(result, None);
                }
            };

        for case in [
            ((-10.0, 0.0), (1.0, 0.0), (0.0, 0.0), 2.0, Some(8.0)),
            ((-10.0, 0.0), (-1.0, 0.0), (0.0, 0.0), 2.0, None),
            ((-10.0, 0.0), (-1.0, 0.0), (0.0, 0.0), 11.0, None),
            ((-10.0, 0.0), (0.0, 0.0), (1.0, 0.0), 2.0, Some(4.0)),
            ((-10.0, 0.0), (0.0, 0.0), (-1.0, 0.0), 2.0, None),
            ((0.0, -10.0), (0.0, 1.0), (0.0, 0.0), 2.0, Some(8.0)),
            ((0.0, -10.0), (0.0, -1.0), (0.0, 0.0), 2.0, None),
            ((0.0, -10.0), (0.0, 0.0), (0.0, 1.0), 2.0, Some(4.0)),
            ((0.0, -10.0), (0.0, 0.0), (0.0, -1.0), 2.0, None),
            ((-6.0, 8.0), (0.3, -0.4), (0.0, 0.0), 5.0, Some(10.0)),
            ((-6.0, 8.0), (0.3, 0.0), (0.0, -0.08), 5.0, Some(10.0)),
            ((-6.0, 8.0), (0.0, 0.0), (0.06, -0.08), 5.0, Some(10.0)),
        ] {
            run_case(case);
        }
    }

    #[test]
    fn velocity_after_collision() {
        let run_case = |(s_a, v_a, s_b, v_b, expected_v_a, expected_v_b): (
            (f32, f32),
            (f32, f32),
            (f32, f32),
            (f32, f32),
            (f32, f32),
            (f32, f32),
        )| {
            let s_a = Vector2::from(s_a).map(feet);
            let v_a = Vector2::from(v_a).map(feet_per_second);
            let s_b = Vector2::from(s_b).map(feet);
            let v_b = Vector2::from(v_b).map(feet_per_second);
            let expected_v_a = Vector2::from(expected_v_a).map(feet_per_second);
            let expected_v_b = Vector2::from(expected_v_b).map(feet_per_second);
            let (new_v_a, new_v_b) = super::velocity_after_collision(s_a, v_a, s_b, v_b);
            assert_approx_eq!(new_v_a.x, expected_v_a.x, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_a.y, expected_v_a.y, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_b.x, expected_v_b.x, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_b.y, expected_v_b.y, ulps = 6, epsilon = 1e-6);
        };

        for case in [
            ((-10.0, 0.0), (1.0, 0.0), (-5.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)),
            ((-1.0, 0.0), (2.0, 2.0), (1.0, 0.0), (0.0, 0.0), (0.0, 2.0), (2.0, 0.0)),
            ((-1.0, -1.0), (2.0, 2.0), (1.0, 1.0), (0.0, 0.0), (0.0, 0.0), (2.0, 2.0)),
        ] {
            run_case(case);
        }
    }
}
