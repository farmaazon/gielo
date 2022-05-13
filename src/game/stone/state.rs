use crate::game::stone::{Acceleration, Curl, Position, Velocity};
use crate::unit;
use crate::unit::Time;
use crate::vector::{EuclideanNorm, Vector2};

#[derive(Clone, Debug)]
pub struct BeingDelivered {
    pub release_time: Time,
    pub starting_point: Position,
    pub delivering_off: Vector2<unit::Length>,
    pub curl: Curl,
}

impl BeingDelivered {
    pub fn position(&self, t: Time) -> Position {
        self.starting_point + self.delivering_off * t / self.release_time
    }

    pub fn velocity(&self) -> Velocity {
        self.delivering_off / self.release_time
    }

    pub fn release_point(&self) -> Position {
        self.starting_point + self.delivering_off
    }
}

#[derive(Clone, Debug)]
pub struct Moving {
    pub t0: Time,
    pub t0_pos: Position,
    pub t0_v: Velocity,
    pub acc: Acceleration,
    pub curl: Curl,
}

impl Moving {
    pub fn position(&self, t: Time) -> Position {
        let t = t - self.t0;
        self.t0_pos + self.t0_v * t + self.acc * t * t / 2.0
    }

    pub fn velocity(&self, t: Time) -> Velocity {
        let t = t - self.t0;
        self.t0_v + self.acc * t
    }

    pub fn when_stop(&self, friction: unit::Acceleration) -> Time {
        let v = self.t0_v.norm();
        self.t0 + (v / friction)
    }
}

#[derive(Clone, Debug)]
pub struct Stationary {
    pos: Position,
    dirty: bool,
}

impl Stationary {
    pub fn new(pos: Position) -> Self {
        Self { pos, dirty: true }
    }

    pub fn position(&self) -> Position {
        self.pos
    }

    pub fn read_position(&mut self) -> Option<Position> {
        if self.dirty {
            self.dirty = false;
            Some(self.pos)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub enum State {
    BeingDelivered(BeingDelivered),
    Moving(Moving),
    Stationary(Stationary),
    Out,
}

impl Default for State {
    fn default() -> Self {
        Self::Stationary(Stationary::new(Position::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{
        assert_approx_eq, feet, feet_per_second, feet_per_second_squared, inches, seconds,
    };

    #[test]
    fn delivered_stone_properties() {
        let state = BeingDelivered {
            release_time: seconds(3.0),
            starting_point: Vector2 {
                x: inches(6.0),
                y: feet(1.0),
            },
            delivering_off: Vector2 {
                x: -feet(3.0),
                y: feet(12.0),
            },
            curl: Curl::None,
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
            t0_pos: Vector2 {
                x: feet(-1.0),
                y: feet(30.0),
            },
            t0_v: Vector2 {
                x: feet_per_second(-0.01),
                y: feet_per_second(3.0),
            },
            acc: Vector2 {
                x: feet_per_second_squared(0.003),
                y: feet_per_second_squared(-0.05),
            },
            curl: Curl::CounterClockwise,
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
        assert_approx_eq!(when_stop, seconds(61.50033333240741255140));
    }

    #[test]
    fn reading_stationary_position() {
        let mut state = Stationary::new(Vector2 {
            x: feet(0.0),
            y: feet(100.0),
        });
        assert!(state.read_position().is_some());
        assert!(state.read_position().is_none());
    }
}
