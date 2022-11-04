use crate::game::sheet::stone::{Acceleration, Position, Velocity};
use crate::unit;
use crate::unit::{feet_squared_per_second_squared, seconds, Time};
use crate::vector::{EuclideanNorm, Vector2};
use derive_more::{Add, Sub};
use roots::{find_roots_linear, find_roots_quadratic, find_roots_quartic, Roots};
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;
use uom::si::velocity::foot_per_second;

#[derive(Clone, Copy, Debug, Default, Add, Sub, PartialEq)]
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

#[derive(Clone, Copy, Debug, Default, Add, Sub, PartialEq)]
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

    pub fn when_hits_circle(&self, r: unit::Length) -> Option<Time> {
        let a = self.a.map(|a| a.get::<foot_per_second_squared>());
        let v0 = self.v0.map(|v| v.get::<foot_per_second>());
        let s0 = self.s0.map(|s| s.get::<foot>());
        let r = r.get::<foot>();
        let a4 = (a.x * a.x + a.y * a.y) / 4.0;
        let a3 = v0.x * a.x + v0.y * a.y;
        let a2 = v0.x * v0.x + v0.y * v0.y + s0.x * a.x + s0.y * a.y;
        let a1 = 2.0 * (v0.x * s0.x + v0.y * s0.y);
        let a0 = s0.x * s0.x + s0.y * s0.y - r * r;
        let already_in_circle = s0.x * s0.x + s0.y * s0.y < r * r;
        let derivative_at_t0 = a1;
        if already_in_circle && derivative_at_t0 < -1e-6 {
            Some(seconds(0.0))
        } else {
            let roots = find_roots_quartic(a4, a3, a2, a1, a0);
            roots
                .as_ref()
                .iter()
                .filter(|&&t| {
                    let derivative = 4.0 * a4 * t * t * t + 3.0 * a3 * t * t + 2.0 * a2 * t + a1;
                    t >= 0.0 && derivative < -1e-6
                })
                .copied()
                .next()
                .map(seconds)
        }
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
    let offset = s_b - s_a;
    let hit_dir = offset / offset.norm();
    let v_given_by_a = hit_dir * v_a.dot(hit_dir);
    let v_given_by_b = -hit_dir * v_b.dot(-hit_dir);
    let new_v_a = v_a + v_given_by_b - v_given_by_a;
    let new_v_b = v_b + v_given_by_a - v_given_by_b;
    (new_v_a, new_v_b)
}

pub fn decrease_energy(v: Velocity, energy_drop: unit::AvailableEnergy) -> Velocity {
    let v_norm = v.norm();
    let new_v_norm_squared = v_norm * v_norm - 2.0 * energy_drop;
    if new_v_norm_squared >= feet_squared_per_second_squared(0.0) {
        v * new_v_norm_squared.sqrt() / v_norm
    } else {
        Velocity::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{
        assert_approx_eq, feet, feet_per_second as feet_ps, feet_per_second_squared as feet_pss,
    };

    #[test]
    fn when_at_position_uniform() {
        let s_near = feet(5.0);
        let s_far = feet(20.0);
        let s0 = feet(10.0);

        struct Case {
            v: f32,
            expect_near: Option<f32>,
            expect_far: Option<f32>,
        }

        let run_case = |Case { v, expect_far, expect_near }: Case| {
            let motion_x = Uniform {
                s0: Vector2 { x: s0, y: unit::Length::default() },
                v: Vector2 { x: feet_ps(v), y: unit::Velocity::default() },
            };
            let motion_y = Uniform {
                s0: Vector2 { x: unit::Length::default(), y: s0 },
                v: Vector2 { x: unit::Velocity::default(), y: feet_ps(v) },
            };
            if let Some(expect_near) = expect_near.map(seconds) {
                assert_approx_eq!(motion_x.when_at_x(s_near).unwrap(), expect_near);
                assert_approx_eq!(motion_y.when_at_y(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion_x.when_at_x(s_near), None);
                assert_eq!(motion_y.when_at_y(s_near), None);
            }
            assert_approx_eq!(motion_x.when_at_x(s0).unwrap(), seconds(0.0));
            assert_approx_eq!(motion_y.when_at_y(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far.map(seconds) {
                assert_approx_eq!(motion_x.when_at_x(s_far).unwrap(), expect_far);
                assert_approx_eq!(motion_y.when_at_y(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion_x.when_at_y(s_far), None);
                assert_eq!(motion_y.when_at_y(s_far), None);
            }
        };

        #[rustfmt::skip]
        for case in [
            Case { v: 1.0,  expect_near: None,      expect_far: Some(10.0) },
            Case { v: -1.0, expect_near: Some(5.0), expect_far: None       },
            Case { v: 0.0,  expect_near: None,      expect_far: None       },
        ] {
            run_case(case);
        };
    }

    #[test]
    fn when_at_position_uniformly_accelerated() {
        let s_near = feet(0.0);
        let s_far = feet(20.0);
        let s0 = feet(10.0);

        struct Case {
            v0: f32,
            a: f32,
            expect_near: Option<f32>,
            expect_far: Option<f32>,
        }

        let run_case = |Case { v0, a, expect_near, expect_far }: Case| {
            let motion_x = UniformlyAccelerated {
                s0: Vector2 { x: s0, y: unit::Length::default() },
                v0: Vector2 { x: feet_ps(v0), y: unit::Velocity::default() },
                a: Vector2 { x: feet_pss(a), y: unit::Acceleration::default() },
            };
            let motion_y = UniformlyAccelerated {
                s0: Vector2 { x: unit::Length::default(), y: s0 },
                v0: Vector2 { x: unit::Velocity::default(), y: feet_ps(v0) },
                a: Vector2 { x: unit::Acceleration::default(), y: feet_pss(a) },
            };
            if let Some(expect_near) = expect_near.map(seconds) {
                assert_approx_eq!(motion_x.when_at_x(s_near).unwrap(), expect_near);
                assert_approx_eq!(motion_y.when_at_y(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion_x.when_at_x(s_near), None);
                assert_eq!(motion_y.when_at_y(s_near), None);
            }
            assert_approx_eq!(motion_x.when_at_x(s0).unwrap(), seconds(0.0));
            assert_approx_eq!(motion_y.when_at_y(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far.map(seconds) {
                assert_approx_eq!(motion_x.when_at_x(s_far).unwrap(), expect_far);
                assert_approx_eq!(motion_y.when_at_y(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion_x.when_at_y(s_far), None);
                assert_eq!(motion_y.when_at_y(s_far), None);
            }
        };

        let sqrt5 = 5.0_f32.sqrt();
        let sqrt2 = 2.0_f32.sqrt();
        #[rustfmt::skip]
        for case in [
            Case { v0: 1.0,  a: 0.2,   expect_near: None,                       expect_far: Some(5.0 * (sqrt5 - 1.0)) },
            Case { v0: 1.0,  a: -0.2,  expect_near: Some(5.0 * (sqrt5 + 1.0)),  expect_far: None },
            Case { v0: -1.0, a: -0.2,  expect_near: Some(5.0 * (sqrt5 - 1.0)),  expect_far: None },
            Case { v0: -1.0, a: 0.2,   expect_near: None,                       expect_far: Some(5.0 * (sqrt5 + 1.0)) },
            Case { v0: 1.0,  a: -0.05, expect_near: Some(20.0 * (sqrt2 + 1.0)), expect_far: Some(20.0) },
        ] {
            run_case(case);
        };
    }

    #[test]
    fn when_enters_circle() {
        struct Case {
            s0: (f32, f32),
            v0: (f32, f32),
            a: (f32, f32),
            r: f32,
            expect_t: Option<f32>,
        }

        let run_case = |Case { s0, v0, a, r, expect_t }: Case| {
            let motion = UniformlyAccelerated {
                s0: Vector2::from(s0).map(feet),
                v0: Vector2::from(v0).map(feet_ps),
                a: Vector2::from(a).map(feet_pss),
            };
            let result = motion.when_hits_circle(feet(r));
            if let Some(expect_t) = expect_t {
                assert_approx_eq!(result.unwrap(), seconds(expect_t), ulps = 6);
            } else {
                assert_eq!(result, None);
            }
        };

        #[rustfmt::skip]
        for case in [
            Case { s0: (-10.0, 0.0), v0: (1.0, 0.0),  a: (0.0, 0.0),    r: 2.0,  expect_t: Some(8.0)  },
            Case { s0: (-10.0, 0.0), v0: (-1.0, 0.0), a: (0.0, 0.0),    r: 2.0,  expect_t: None       },
            Case { s0: (-10.0, 0.0), v0: (-1.0, 0.0), a: (0.0, 0.0),    r: 11.0, expect_t: None       },
            Case { s0: (-10.0, 0.0), v0: (0.0, 0.0),  a: (1.0, 0.0),    r: 2.0,  expect_t: Some(4.0)  },
            Case { s0: (-10.0, 0.0), v0: (0.0, 0.0),  a: (-1.0, 0.0),   r: 2.0,  expect_t: None       },
            Case { s0: (0.0, -10.0), v0: (0.0, 1.0),  a: (0.0, 0.0),    r: 2.0,  expect_t: Some(8.0)  },
            Case { s0: (0.0, -10.0), v0: (0.0, -1.0), a: (0.0, 0.0),    r: 2.0,  expect_t: None       },
            Case { s0: (0.0, -10.0), v0: (0.0, 0.0),  a: (0.0, 1.0),    r: 2.0,  expect_t: Some(4.0)  },
            Case { s0: (0.0, -10.0), v0: (0.0, 0.0),  a: (0.0, -1.0),   r: 2.0,  expect_t: None       },
            Case { s0: (-6.0, 8.0),  v0: (0.3, -0.4), a: (0.0, 0.0),    r: 5.0,  expect_t: Some(10.0) },
            Case { s0: (-6.0, 8.0),  v0: (0.3, 0.0),  a: (0.0, -0.08),  r: 5.0,  expect_t: Some(10.0) },
            Case { s0: (-6.0, 8.0),  v0: (0.0, 0.0),  a: (0.06, -0.08), r: 5.0,  expect_t: Some(10.0) },
        ] {
            run_case(case);
        };
    }

    #[test]
    fn velocity_after_collision() {
        struct Case {
            s_a: (f32, f32),
            v_a: (f32, f32),
            s_b: (f32, f32),
            v_b: (f32, f32),
            expect_v_a: (f32, f32),
            expect_v_b: (f32, f32),
        }

        let run_case = |Case { s_a, v_a, s_b, v_b, expect_v_a, expect_v_b }: Case| {
            let s_a = Vector2::from(s_a).map(feet);
            let v_a = Vector2::from(v_a).map(feet_ps);
            let s_b = Vector2::from(s_b).map(feet);
            let v_b = Vector2::from(v_b).map(feet_ps);
            let expect_v_a = Vector2::from(expect_v_a).map(feet_pss);
            let expect_v_b = Vector2::from(expect_v_b).map(feet_pss);
            let (new_v_a, new_v_b) = super::velocity_after_collision(s_a, v_a, s_b, v_b);
            assert_approx_eq!(new_v_a.x, expect_v_a.x, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_a.y, expect_v_a.y, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_b.x, expect_v_b.x, ulps = 6, epsilon = 1e-6);
            assert_approx_eq!(new_v_b.y, expect_v_b.y, ulps = 6, epsilon = 1e-6);
        };

        #[rustfmt::skip]
        for case in [
            Case { s_a: (-10.0, 0.0), v_a: (1.0, 0.0), s_b: (-5.0, 0.0), v_b: (0.0, 0.0), expect_v_a: (0.0, 0.0), expect_v_b: (1.0, 0.0)},
            Case { s_a: (-1.0, 0.0),  v_a: (2.0, 2.0), s_b: (1.0, 0.0),  v_b: (0.0, 0.0), expect_v_a: (0.0, 2.0), expect_v_b: (2.0, 0.0)},
            Case { s_a: (-1.0, -1.0), v_a: (2.0, 2.0), s_b: (1.0, 1.0),  v_b: (0.0, 0.0), expect_v_a: (0.0, 0.0), expect_v_b: (2.0, 2.0)},
        ] {
            run_case(case);
        };
    }

    #[test]
    fn decreasing_energy() {
        struct Case {
            v: (f32, f32),
            e: f32,
            expect_v: (f32, f32),
        }

        let run_case = |Case { v, e, expect_v }: Case| {
            let v = Vector2::from(v).map(feet_ps);
            let e = feet_squared_per_second_squared(e);
            let expect_v = Vector2::from(expect_v).map(feet_ps);
            let new_v = super::decrease_energy(v, e);
            assert_approx_eq!(new_v.x, expect_v.x, ulps = 10);
            assert_approx_eq!(new_v.y, expect_v.y, ulps = 10);
        };

        #[rustfmt::skip]
        for case in [
            Case { v: (10.0, 0.0), e: 9.5,        expect_v: (9.0, 0.0)      },
            Case { v: (0.3, -0.4), e: 55.0/512.0, expect_v: (0.1125, -0.15) },
        ] {
            run_case(case);
        };
    }
}
