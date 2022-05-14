use crate::game::{sheet, Team};
use crate::unit;
use crate::unit::Time;
use crate::vector::{EuclideanNorm, Vector2};
use decorum::NotNan;
use uom::si::time::second;
use uom::ConstZero;

pub mod state;
pub use state::State;

pub type Id = usize;
pub type Velocity = Vector2<unit::Velocity>;
pub type Acceleration = Vector2<unit::Acceleration>;
pub type Position = Vector2<unit::Length>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Curl {
    None,
    Clockwise,
    CounterClockwise,
}

#[derive(Clone, Debug)]
pub struct Stone {
    pub team: Team,
    pub state: State,
}

impl Stone {
    pub fn position(&self, t: Time) -> Option<Position> {
        match &self.state {
            State::BeingDelivered(state) => Some(state.position(t)),
            State::Moving(state) => Some(state.position(t)),
            State::Stationary(state) => Some(state.position()),
            State::Out => None,
        }
    }

    pub fn when_next_stage(&self, sheet: &sheet::Parameters) -> Option<Time> {
        match &self.state {
            State::BeingDelivered(state) => Some(state.release_time),
            State::Moving(state) => Some(state.when_stop(sheet.friction)),
            _ => None,
        }
    }

    pub fn when_outside_x(&self, sheet: &sheet::Parameters) -> Option<Time> {
        [(sheet.left_bound(), -1.0), (sheet.right_bound(), 1.0)]
            .into_iter()
            .filter_map(|(bound, x_sign)| {
                let out_x = bound - x_sign * sheet.stone_radius;
                match &self.state {
                    State::BeingDelivered(state) => state.motion_x().when_at_position(out_x),
                    State::Moving(state) => {
                        state.motion_x().when_at_position(out_x).map(|t| t + state.t0)
                    }
                    _ => None,
                }
            })
            .min_by_key(|t| NotNan::from_inner(t.get::<second>()))
    }

    pub fn when_outside_y(&self, sheet: &sheet::Parameters) -> Option<Time> {
        match &self.state {
            State::Moving(state) => {
                let out_y = *sheet::playing_end::BACK_LINE_Y + sheet.stone_radius;
                state.motion_y().when_at_position(out_y).map(|t| t + state.t0)
            }
            _ => None,
        }
    }

    pub fn next_stage(&mut self, sheet: &sheet::Parameters) {
        self.state = match std::mem::take(&mut self.state) {
            State::BeingDelivered(state) => {
                let t0_v = state.velocity();
                State::Moving(state::Moving {
                    t0: state.release_time,
                    t0_pos: state.release_point(),
                    t0_v,
                    acc: compute_acc(sheet, t0_v, state.curl),
                    curl: state.curl,
                })
            }
            State::Moving(state) => {
                let pos = state.position(state.when_stop(sheet.friction));
                State::Stationary(state::Stationary::new(pos))
            }
            other => other,
        };
    }

    pub fn next_time_quantum(&mut self, next_time_quantum: Time, sheet: &sheet::Parameters) {
        if let State::Moving(state) = &mut self.state {
            let new_t0_v = state.velocity(next_time_quantum);
            let new_state = state::Moving {
                t0: next_time_quantum,
                t0_pos: state.position(next_time_quantum),
                t0_v: new_t0_v,
                acc: compute_acc(sheet, new_t0_v, state.curl),
                curl: state.curl,
            };
            *state = new_state
        }
    }
}

fn compute_acc(sheet: &sheet::Parameters, t0_v: Velocity, curl: Curl) -> Acceleration {
    let v = t0_v.norm();
    let a_friction = -t0_v / v * sheet.friction;
    let a_curl = match curl {
        Curl::None => Vector2 { x: unit::Velocity::ZERO, y: unit::Velocity::ZERO },
        Curl::Clockwise => Vector2 { x: -t0_v.y, y: t0_v.x },
        Curl::CounterClockwise => Vector2 { x: t0_v.y, y: -t0_v.x },
    } * sheet.curl_factor
        / v;
    a_friction + a_curl
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet::Hack;
    use crate::unit::{
        assert_approx_eq, feet, feet_per_second, feet_per_second_squared, inches, seconds,
    };

    #[test]
    fn delivered_stone_out_x() {
        let sheet = sheet::Parameters::default();
        let state = state::BeingDelivered {
            release_time: seconds(3.0),
            starting_point: sheet::hack_pos(Hack::Left),
            delivering_off: Vector2 {
                x: sheet.right_bound() - *sheet::HACK_X_OFFSET - sheet.stone_radius,
                y: feet(6.0),
            } * 2.0,
            curl: Curl::Clockwise,
        };
        let stone = Stone { team: Team::First, state: State::BeingDelivered(state.clone()) };
        assert_approx_eq!(stone.when_outside_x(&sheet).unwrap(), seconds(1.5));
        let stone = Stone {
            team: Team::Second,
            state: State::BeingDelivered(state::BeingDelivered {
                delivering_off: Vector2 {
                    x: sheet.left_bound() - *sheet::HACK_X_OFFSET + sheet.stone_radius,
                    y: feet(6.0),
                } * 3.0,
                ..state
            }),
        };
        assert_approx_eq!(stone.when_outside_x(&sheet).unwrap(), seconds(1.0));
    }

    #[test]
    fn moving_stone_out_x() {
        let sheet = sheet::Parameters::default();
        let state = state::Moving {
            t0: seconds(10.0),
            t0_pos: Vector2 {
                x: sheet.right_bound() - sheet.stone_radius - feet(1.0),
                y: feet(100.0),
            },
            t0_v: Vector2 { x: feet_per_second(1.0), y: feet_per_second(3.0) },
            acc: Vector2 { x: feet_per_second_squared(0.001), y: feet_per_second_squared(0.03) },
            curl: Curl::CounterClockwise,
        };
        let stone = Stone { team: Team::First, state: State::Moving(state) };
        assert_approx_eq!(stone.when_outside_x(&sheet).unwrap(), seconds(11.0), epsilon = 0.1);
    }

    #[test]
    fn moving_stone_out_y() {
        let sheet = sheet::Parameters::default();
        let state = state::Moving {
            t0: seconds(20.0),
            t0_pos: Vector2 {
                x: feet(3.0),
                y: *sheet::playing_end::BACK_LINE_Y + sheet.stone_radius - feet(1.0),
            },
            t0_v: Vector2 { x: feet_per_second(-0.1), y: feet_per_second(1.0) },
            acc: Vector2 { x: feet_per_second_squared(0.001), y: feet_per_second_squared(0.03) },
            curl: Curl::CounterClockwise,
        };
        let stone = Stone { team: Team::First, state: State::Moving(state) };
        assert_approx_eq!(stone.when_outside_y(&sheet).unwrap(), seconds(21.0), epsilon = 0.1);
    }

    #[test]
    fn releasing() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(0.2),
            curl_factor: feet_per_second_squared(0.03),
            stone_radius: inches(6.0),
            width: feet(15.0),
        };
        // CounterClockwise
        let mut state = state::BeingDelivered {
            release_time: seconds(3.0),
            starting_point: Vector2 { x: inches(6.0), y: feet(6.0) },
            delivering_off: Vector2 { x: feet(-1.0), y: feet(30.0) },
            curl: Curl::CounterClockwise,
        };
        let mut stone = Stone { team: Team::First, state: State::BeingDelivered(state.clone()) };
        stone.next_stage(&sheet);
        let new_state = if let State::Moving(state) = stone.state.clone() {
            state
        } else {
            panic!("Wrong state after release");
        };
        assert_approx_eq!(new_state.t0, seconds(3.0));
        assert_approx_eq!(new_state.t0_pos.x, feet(-1.0) + inches(6.0));
        assert_approx_eq!(new_state.t0_pos.y, feet(36.0));
        assert_approx_eq!(new_state.t0_v.x, feet_per_second(-1.0 / 3.0));
        assert_approx_eq!(new_state.t0_v.y, feet_per_second(10.0));
        assert_approx_eq!(new_state.acc.x, feet_per_second_squared(1.1 / 901.0_f32.sqrt()));
        assert_approx_eq!(new_state.acc.y, feet_per_second_squared(-5.97 / 901.0_f32.sqrt()));
        assert_eq!(new_state.curl, Curl::CounterClockwise);

        // Clockwise
        state.curl = Curl::Clockwise;
        let mut stone = Stone { team: Team::First, state: State::BeingDelivered(state) };
        stone.next_stage(&sheet);
        let new_state_cw = if let State::Moving(state) = stone.state.clone() {
            state
        } else {
            panic!("Wrong state after release");
        };
        assert_approx_eq!(new_state_cw.t0, new_state.t0);
        assert_approx_eq!(new_state_cw.t0_pos.x, new_state.t0_pos.x);
        assert_approx_eq!(new_state_cw.t0_pos.y, new_state.t0_pos.y);
        assert_approx_eq!(new_state_cw.t0_v.x, new_state.t0_v.x);
        assert_approx_eq!(new_state_cw.t0_v.y, new_state.t0_v.y);
        assert_approx_eq!(new_state_cw.acc.x, feet_per_second_squared(-0.7 / 901.0_f32.sqrt()));
        assert_approx_eq!(new_state_cw.acc.y, feet_per_second_squared(-6.03 / 901.0_f32.sqrt()));
        assert_eq!(new_state_cw.curl, Curl::Clockwise);
    }

    #[test]
    fn moving_stone_next_quantum() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(0.2),
            curl_factor: feet_per_second_squared(0.03),
            stone_radius: inches(6.0),
            width: feet(15.0),
        };
        let state = state::Moving {
            t0: seconds(2.0),
            t0_pos: Vector2 { x: feet(3.0), y: feet(100.0) },
            t0_v: Vector2 { x: feet_per_second(-3.0), y: feet_per_second(4.0) },
            acc: Vector2 { x: feet_per_second_squared(0.096), y: feet_per_second_squared(-0.178) },
            curl: Curl::Clockwise,
        };
        let mut stone = Stone { team: Team::First, state: State::Moving(state) };
        stone.next_time_quantum(seconds(4.0), &sheet);
        let new_state = if let State::Moving(state) = stone.state {
            state
        } else {
            panic!("Wrong state after next time quantum")
        };
        assert_approx_eq!(new_state.t0, seconds(4.0));
        assert_approx_eq!(new_state.t0_pos.x, feet(-2.808));
        assert_approx_eq!(new_state.t0_pos.y, feet(107.644));
        assert_approx_eq!(new_state.t0_v.x, feet_per_second(-2.808));
        assert_approx_eq!(new_state.t0_v.y, feet_per_second(3.644));
        let v = new_state.t0_v.norm().value;
        assert_approx_eq!(new_state.acc.x, feet_per_second_squared(0.45228 / v));
        assert_approx_eq!(new_state.acc.y, feet_per_second_squared(-0.81304 / v));
        assert_eq!(new_state.curl, Curl::Clockwise)
    }

    #[test]
    fn stopping_stone() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(1.0),
            curl_factor: feet_per_second_squared(0.1),
            ..sheet::Parameters::default()
        };
        let state = state::Moving {
            t0: seconds(2.0),
            t0_pos: Vector2 { x: feet(3.0), y: feet(100.0) },
            t0_v: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
            acc: Vector2 { x: feet_per_second_squared(0.52), y: feet_per_second_squared(-0.86) },
            curl: Curl::Clockwise,
        };
        let mut stone = Stone { team: Team::First, state: State::Moving(state) };
        stone.next_stage(&sheet);
        let new_state = if let State::Stationary(state) = stone.state {
            state
        } else {
            panic!("Wrong state after next stage")
        };
        assert_approx_eq!(new_state.position().x, feet(2.915));
        assert_approx_eq!(new_state.position().y, feet(100.0925));
    }
}
