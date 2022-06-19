use crate::unit::{feet_per_second_squared, seconds, Acceleration, Length, Time, Velocity};
use crate::vector::Vector2;
use derive_more::{Add, Sub};
use roots::{find_roots_linear, find_roots_quadratic, find_roots_quartic, Roots};
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;
use uom::si::velocity::foot_per_second;

#[derive(Clone, Copy, Debug, Default, Add, Sub)]
pub struct Uniform {
    pub s0: Length,
    pub v: Velocity,
}

impl Uniform {
    pub fn when_at_position(&self, s: Length) -> Option<Time> {
        let a1 = self.v.get::<foot_per_second>();
        let a0 = (self.s0 - s).get::<foot>();
        let roots = find_roots_linear(a1, a0);
        min_non_negative_root(roots).map(seconds)
    }
}

#[derive(Clone, Copy, Debug, Default, Add, Sub)]
pub struct UniformlyAccelerated {
    pub s0: Length,
    pub v0: Velocity,
    pub a: Acceleration,
}

impl UniformlyAccelerated {
    pub fn when_at_position(&self, s: Length) -> Option<Time> {
        let a2 = (self.a / 2.0).get::<foot_per_second_squared>();
        let a1 = self.v0.get::<foot_per_second>();
        let a0 = (self.s0 - s).get::<foot>();
        let roots = find_roots_quadratic(a2, a1, a0);
        min_non_negative_root(roots).map(seconds)
    }
}

impl From<Uniform> for UniformlyAccelerated {
    fn from(uniform: Uniform) -> Self {
        Self { s0: uniform.s0, v0: uniform.v, a: feet_per_second_squared(0.0) }
    }
}

fn min_non_negative_root(roots: Roots<f32>) -> Option<f32> {
    roots.as_ref().iter().filter(|&&t| t >= 0.0).copied().next()
}

pub fn when_cross_circle(motion: Vector2<UniformlyAccelerated>, r: Length) -> Option<Time> {
    let a = motion.map(|m| m.a.get::<foot_per_second_squared>());
    let v0 = motion.map(|m| m.v0.get::<foot_per_second>());
    let s0 = motion.map(|m| m.s0.get::<foot>());
    let r = r.get::<foot>();
    let a4 = (a.x * a.x + a.y + a.y) / 4.0;
    let a3 = v0.x * a.x + v0.y * a.y;
    let a2 = v0.x * v0.x + v0.y * v0.y + s0.x * a.x + s0.y * a.y;
    let a1 = 2.0 * (v0.x * s0.x + v0.y * s0.y);
    let a0 = s0.x * s0.x + s0.y * s0.y - r * r;
    let roots = find_roots_quartic(a4, a3, a2, a1, a0);
    min_non_negative_root(roots).map(seconds)
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

        let run_case = |case: (Velocity, Option<Time>, Option<Time>)| {
            let (v, expect_near, expect_far) = case;
            let motion = Uniform { s0, v };
            if let Some(expect_near) = expect_near {
                assert_approx_eq!(motion.when_at_position(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion.when_at_position(s_near), None)
            }
            assert_approx_eq!(motion.when_at_position(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far {
                assert_approx_eq!(motion.when_at_position(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion.when_at_position(s_far), None,)
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

        let run_case = |case: (Velocity, Acceleration, Option<Time>, Option<Time>)| {
            let (v0, a, expect_near, expect_far) = case;
            let motion = UniformlyAccelerated { s0, v0, a };
            if let Some(expect_near) = expect_near {
                assert_approx_eq!(motion.when_at_position(s_near).unwrap(), expect_near);
            } else {
                assert_eq!(motion.when_at_position(s_near), None)
            }
            assert_approx_eq!(motion.when_at_position(s0).unwrap(), seconds(0.0));
            if let Some(expect_far) = expect_far {
                assert_approx_eq!(motion.when_at_position(s_far).unwrap(), expect_far);
            } else {
                assert_eq!(motion.when_at_position(s_far), None,)
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
}
