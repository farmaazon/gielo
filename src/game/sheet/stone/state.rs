use crate::game::sheet::stone::{motion, Position, Rotation, Velocity};
use crate::unit;
use crate::unit::Time;
use crate::vector::{EuclideanNorm, Vector2};

#[derive(Clone, Debug, PartialEq)]
pub struct BeingDelivered {
    pub release_time: Time,
    pub starting_point: Position,
    pub delivering_off: Vector2<unit::Length>,
    pub rotation: Rotation,
}

impl BeingDelivered {
    pub fn position(&self, t: Time) -> Position {
        self.starting_point + self.delivering_off * t / self.release_time
    }

    pub fn velocity(&self) -> Velocity {
        self.delivering_off / self.release_time
    }

    pub fn motion(&self) -> motion::Uniform {
        motion::Uniform { s0: self.starting_point, v: self.velocity() }
    }

    pub fn release_point(&self) -> Position {
        self.starting_point + self.delivering_off
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Moving {
    pub t0: Time,
    pub motion: motion::UniformlyAccelerated,
    pub rotation: Rotation,
}

impl Moving {
    pub fn position(&self, t: Time) -> Position {
        self.motion.position(t - self.t0)
    }

    pub fn velocity(&self, t: Time) -> Velocity {
        self.motion.velocity(t - self.t0)
    }

    pub fn when_stop(&self, friction: unit::Acceleration) -> Time {
        let v = self.motion.v0.norm();
        self.t0 + (v / friction)
    }

    pub fn motion_at_t(&self, t: Time) -> motion::UniformlyAccelerated {
        motion::UniformlyAccelerated {
            s0: self.position(t),
            v0: self.velocity(t),
            a: self.motion.a,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    BeingDelivered(BeingDelivered),
    Moving(Moving),
    Stationary(Position),
    Out,
}

impl Default for State {
    fn default() -> Self {
        Self::Stationary(Position::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::unit::{
        assert_approx_eq, feet, feet_per_second, feet_per_second_squared, inches, seconds,
    };

    use super::*;

    #[test]
    fn delivered_stone_properties() {
        let state = BeingDelivered {
            release_time: seconds(3.0),
            starting_point: Vector2 { x: inches(6.0), y: feet(1.0) },
            delivering_off: Vector2 { x: -feet(3.0), y: feet(12.0) },
            rotation: Rotation::None,
        };
        let position = state.position(seconds(1.0));
        assert_approx_eq!(position.x, inches(6.0) - feet(1.0));
        assert_approx_eq!(position.y, feet(5.0));

        let position = state.position(seconds(2.0));
        assert_approx_eq!(position.x, inches(6.0) - feet(2.0));
        assert_approx_eq!(position.y, feet(9.0));

        let velocity = state.velocity();
        assert_approx_eq!(velocity.x, feet_per_second(-1.0));
        assert_approx_eq!(velocity.y, feet_per_second(4.0));

        let release_point = state.release_point();
        assert_approx_eq!(release_point.x, inches(6.0) - feet(3.0));
        assert_approx_eq!(release_point.y, feet(13.0));
    }

    #[test]
    fn moving_stone_properties() {
        let state = Moving {
            t0: seconds(1.5),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(-1.0), y: feet(30.0) },
                v0: Vector2 { x: feet_per_second(-0.01), y: feet_per_second(3.0) },
                a: Vector2 { x: feet_per_second_squared(0.003), y: feet_per_second_squared(-0.05) },
            },
            rotation: Rotation::CounterClockwise,
        };

        let t = seconds(2.0);
        let velocity = state.velocity(t);
        assert_approx_eq!(velocity.x, feet_per_second(-0.0085));
        assert_approx_eq!(velocity.y, feet_per_second(2.975));
        let position = state.position(t);
        assert_approx_eq!(position.x, feet(-1.004625));
        assert_approx_eq!(position.y, feet(31.49375));

        let t = seconds(3.5);
        let velocity = state.velocity(t);
        assert_approx_eq!(velocity.x, feet_per_second(-0.004));
        assert_approx_eq!(velocity.y, feet_per_second(2.9));
        let position = state.position(t);
        assert_approx_eq!(position.x, feet(-1.014));
        assert_approx_eq!(position.y, feet(35.9));

        let when_stop = state.when_stop(feet_per_second_squared(0.05));
        assert_approx_eq!(when_stop, seconds(61.50033));
    }
}
