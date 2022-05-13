use crate::game::{sheet, Sheet, Team};
use crate::unit;
use crate::unit::{seconds, Time};
use crate::vector::{EuclideanNorm, Vector2};
use decorum::NotNan;
use roots::{find_roots_linear, find_roots_quadratic, Roots};
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;
use uom::si::time::second;
use uom::si::velocity::foot_per_second;
use uom::ConstZero;

pub mod state;
pub use state::State;

pub type Id = usize;
pub type Velocity = Vector2<unit::Velocity>;
pub type Acceleration = Vector2<unit::Acceleration>;
pub type Position = Vector2<unit::Length>;

#[derive(Copy, Clone, Debug)]
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

    pub fn when_next_stage(&self, sheet_params: &sheet::Parameters) -> Option<Time> {
        match &self.state {
            State::BeingDelivered(state) => Some(state.release_time),
            State::Moving(state) => Some(state.when_stop(sheet_params.friction)),
            _ => None,
        }
    }

    pub fn when_outside_x(&self, sheet: &Sheet) -> Option<Time> {
        [(sheet.left_bound(), -1.0), (sheet.right_bound(), 1.0)]
            .into_iter()
            .filter_map(|(bound, x_sign)| {
                let out_x = bound - x_sign * sheet.parameters.stone_radius;
                match &self.state {
                    State::BeingDelivered(state) => {
                        let candidates = find_roots_linear(
                            state.velocity().x.get::<foot_per_second>(),
                            (state.starting_point.x - out_x).get::<foot>(),
                        );
                        min_plausible_time_candidate(candidates)
                    }
                    State::Moving(state::Moving {
                        t0,
                        acc,
                        t0_v,
                        t0_pos,
                        ..
                    }) => {
                        let candidates = find_roots_quadratic(
                            acc.x.get::<foot_per_second_squared>() / 2.0,
                            t0_v.x.get::<foot_per_second>(),
                            (t0_pos.x - out_x).get::<foot>(),
                        );
                        min_plausible_time_candidate(candidates).map(|t| t + *t0)
                    }
                    _ => None,
                }
            })
            .min_by_key(|t| NotNan::from_inner(t.get::<second>()))
    }

    pub fn when_outside_y(&self, sheet: &Sheet) -> Option<Time> {
        match &self.state {
            State::Moving(state::Moving {
                t0,
                t0_pos,
                t0_v,
                acc,
                ..
            }) => {
                let out_y = *sheet::playing_end::BACK_LINE_Y + sheet.parameters.stone_radius;
                let candidates = find_roots_quadratic(
                    acc.y.get::<foot_per_second_squared>() / 2.0,
                    t0_v.y.get::<foot_per_second>(),
                    (t0_pos.y - out_y).get::<foot>(),
                );
                min_plausible_time_candidate(candidates).map(|t| t + *t0)
            }
            _ => None,
        }
    }

    pub fn next_stage(&mut self, sheet_params: &sheet::Parameters) {
        self.state = match std::mem::take(&mut self.state) {
            State::BeingDelivered(state) => {
                let t0_v = state.velocity();
                State::Moving(state::Moving {
                    t0: state.release_time,
                    t0_pos: state.release_point(),
                    t0_v,
                    acc: compute_acc(sheet_params, t0_v, state.curl),
                    curl: state.curl,
                })
            }
            State::Moving(state) => {
                let pos = state.position(state.when_stop(sheet_params.friction));
                State::Stationary(state::Stationary::new(pos))
            }
            other => other,
        };
    }

    pub fn next_time_quantum(&mut self, next_time_quantum: Time, sheet_params: &sheet::Parameters) {
        if let State::Moving(state) = &mut self.state {
            let new_t0_v = state.velocity(next_time_quantum);
            let new_state = state::Moving {
                t0: next_time_quantum,
                t0_pos: state.position(next_time_quantum),
                t0_v: new_t0_v,
                acc: compute_acc(sheet_params, new_t0_v, state.curl),
                curl: state.curl,
            };
            *state = new_state
        }
    }
}

fn compute_acc(sheet_params: &sheet::Parameters, t0_v: Velocity, curl: Curl) -> Acceleration {
    let v = t0_v.norm();
    let a_friction = -t0_v / v * sheet_params.friction;
    let a_curl = match curl {
        Curl::None => Vector2 {
            x: unit::Velocity::ZERO,
            y: unit::Velocity::ZERO,
        },
        Curl::Clockwise => Vector2 {
            x: -t0_v.y,
            y: t0_v.x,
        },
        Curl::CounterClockwise => Vector2 {
            x: t0_v.y,
            y: -t0_v.x,
        },
    } * sheet_params.curl_factor
        / v;
    a_friction + a_curl
}

fn min_plausible_time_candidate(roots: Roots<f32>) -> Option<Time> {
    roots
        .as_ref()
        .iter()
        .filter(|&&t| t >= 0.0)
        .copied()
        .next()
        .map(seconds)
}
