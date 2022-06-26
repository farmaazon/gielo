use decorum::NotNan;
use uom::si::time::second;
use uom::ConstZero;

pub use state::State;

use crate::game::{sheet, Team};
use crate::unit;
use crate::unit::Time;
use crate::vector::{EuclideanNorm, Vector2};

pub mod motion;
pub mod state;

pub type Id = usize;
pub type Velocity = Vector2<unit::Velocity>;
pub type Acceleration = Vector2<unit::Acceleration>;
pub type Position = Vector2<unit::Length>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Rotation {
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
    pub fn new_stationary(team: Team, position: Position) -> Self {
        Self { team, state: State::Stationary(state::Stationary::new(position)) }
    }

    pub fn position(&self, t: Time) -> Option<Position> {
        match &self.state {
            State::BeingDelivered(state) => Some(state.position(t)),
            State::Moving(state) => Some(state.position(t)),
            State::Stationary(state) => Some(state.position()),
            State::Out { .. } => None,
        }
    }

    pub fn is_position_dirty(&self) -> bool {
        match &self.state {
            State::BeingDelivered(_) => true,
            State::Moving(_) => true,
            State::Stationary(state) => state.is_dirty(),
            State::Out { dirty } => *dirty,
        }
    }

    pub fn read_position(&mut self, t: Time) -> (bool, Option<Position>) {
        match &mut self.state {
            State::BeingDelivered(state) => (true, Some(state.position(t))),
            State::Moving(state) => (true, Some(state.position(t))),
            State::Stationary(state) => {
                let (was_dirty, pos) = state.read_position();
                (was_dirty, Some(pos))
            }
            State::Out { dirty } => (std::mem::take(dirty), None),
        }
    }

    pub fn velocity(&self, t: Time) -> Option<Velocity> {
        match &self.state {
            State::BeingDelivered(state) => Some(state.velocity()),
            State::Moving(state) => Some(state.velocity(t)),
            State::Stationary(_) => Some(Velocity::default()),
            State::Out { .. } => None,
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
        [(sheet.geometry.left_bound(), -1.0), (sheet.geometry.right_bound(), 1.0)]
            .into_iter()
            .filter_map(|(bound, x_sign)| {
                let out_x = bound - x_sign * sheet.stone_radius;
                match &self.state {
                    State::BeingDelivered(state) => state.motion().when_at_x(out_x),
                    State::Moving(state) => state.motion.when_at_x(out_x).map(|t| t + state.t0),
                    _ => None,
                }
            })
            .min_by_key(|t| NotNan::from_inner(t.get::<second>()))
    }

    pub fn when_outside_y(&self, sheet: &sheet::Parameters) -> Option<Time> {
        match &self.state {
            State::Moving(state) => {
                let out_y = sheet.geometry.playing_end.back_line_y + sheet.stone_radius;
                state.motion.when_at_y(out_y).map(|t| t + state.t0)
            }
            _ => None,
        }
    }

    pub fn when_collision(&self, rhs: &Stone, sheet: &sheet::Parameters) -> Option<Time> {
        match (&self.state, &rhs.state) {
            (State::Moving(lhs), State::Moving(rhs)) => {
                let t0 = lhs.t0.max(rhs.t0);
                let motion = lhs.motion_at_t(t0) - rhs.motion_at_t(t0);
                motion.when_hits_circle(sheet.stone_radius * 2.0).map(|t| t + t0)
            }
            (State::Moving(moving), State::Stationary(stationary))
            | (State::Stationary(stationary), State::Moving(moving)) => {
                let mut motion = moving.motion;
                motion.s0 -= stationary.position();
                motion.when_hits_circle(sheet.stone_radius * 2.0).map(|t| t + moving.t0)
            }
            _ => None,
        }
    }

    pub fn next_stage(&mut self, sheet: &sheet::Parameters) {
        self.state = match std::mem::take(&mut self.state) {
            State::BeingDelivered(state) => {
                let v0 = state.velocity();
                State::Moving(state::Moving {
                    t0: state.release_time,
                    motion: motion::UniformlyAccelerated {
                        s0: state.release_point(),
                        v0,
                        a: compute_acc(sheet, v0, state.rotation),
                    },
                    rotation: state.rotation,
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
            let new_v0 = state.velocity(next_time_quantum);
            let new_state = state::Moving {
                t0: next_time_quantum,
                motion: motion::UniformlyAccelerated {
                    s0: state.position(next_time_quantum),
                    v0: new_v0,
                    a: compute_acc(sheet, new_v0, state.rotation),
                },
                rotation: state.rotation,
            };
            *state = new_state
        }
    }

    pub fn states_after_collision(
        &self,
        rhs: &Stone,
        sheet: &sheet::Parameters,
        t: Time,
    ) -> Option<(State, State)> {
        let lhs_pos = self.position(t)?;
        let rhs_pos = rhs.position(t)?;
        let lhs_v = self.velocity(t).unwrap_or_default();
        let rhs_v = rhs.velocity(t).unwrap_or_default();
        let (mut new_lhs_v, mut new_rhs_v) =
            dbg!(motion::velocity_after_collision(lhs_pos, lhs_v, rhs_pos, rhs_v));
        if matches!(self.state, State::Stationary(_)) {
            new_lhs_v = motion::decrease_energy(new_lhs_v, sheet.static_friction);
        }
        if matches!(rhs.state, State::Stationary(_)) {
            new_rhs_v = motion::decrease_energy(new_rhs_v, sheet.static_friction);
        }
        let make_state = |pos: Position, new_v: Velocity| {
            if new_v.x > unit::Velocity::ZERO || new_v.y > unit::Velocity::ZERO {
                State::Moving(state::Moving {
                    t0: t,
                    motion: motion::UniformlyAccelerated {
                        s0: pos,
                        v0: new_v,
                        a: compute_acc(sheet, new_v, Rotation::None),
                    },
                    rotation: Rotation::None,
                })
            } else {
                State::Stationary(state::Stationary::new(pos))
            }
        };
        Some((make_state(lhs_pos, new_lhs_v), make_state(rhs_pos, new_rhs_v)))
    }
}

fn compute_acc(sheet: &sheet::Parameters, v0: Velocity, rotation: Rotation) -> Acceleration {
    let v = v0.norm();
    let a_friction = -v0 / v * sheet.friction;
    let a_rotation = match rotation {
        Rotation::None => Vector2 { x: unit::Velocity::ZERO, y: unit::Velocity::ZERO },
        Rotation::Clockwise => Vector2 { x: -v0.y, y: v0.x },
        Rotation::CounterClockwise => Vector2 { x: v0.y, y: -v0.x },
    } * sheet.rotation_acc
        / v;
    a_friction + a_rotation
}

#[cfg(test)]
mod tests {
    use crate::game::sheet::Hack;
    use crate::unit::{
        assert_approx_eq, feet, feet_per_second, feet_per_second_squared,
        feet_squared_per_second_squared, inches, seconds,
    };

    use super::*;

    #[test]
    fn delivered_stone_out_x() {
        let sheet = sheet::Parameters::default();
        let state = state::BeingDelivered {
            release_time: seconds(3.0),
            starting_point: sheet.geometry.hack_pos(Hack::Left),
            delivering_off: Vector2 {
                x: sheet.geometry.right_bound() - sheet.geometry.hack_x_offset - sheet.stone_radius,
                y: feet(6.0),
            } * 2.0,
            rotation: Rotation::Clockwise,
        };
        let stone = Stone { team: Team::A, state: State::BeingDelivered(state.clone()) };
        assert_approx_eq!(stone.when_outside_x(&sheet).unwrap(), seconds(1.5));
        let stone = Stone {
            team: Team::B,
            state: State::BeingDelivered(state::BeingDelivered {
                starting_point: sheet.geometry.hack_pos(Hack::Right),
                delivering_off: Vector2 {
                    x: sheet.geometry.left_bound()
                        + sheet.geometry.hack_x_offset
                        + sheet.stone_radius,
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
            motion: motion::UniformlyAccelerated {
                s0: Vector2 {
                    x: sheet.geometry.right_bound() - sheet.stone_radius - feet(1.0),
                    y: feet(100.0),
                },
                v0: Vector2 { x: feet_per_second(1.0), y: feet_per_second(3.0) },
                a: Vector2 { x: feet_per_second_squared(0.001), y: feet_per_second_squared(0.03) },
            },
            rotation: Rotation::CounterClockwise,
        };
        let stone = Stone { team: Team::A, state: State::Moving(state) };
        assert_approx_eq!(stone.when_outside_x(&sheet).unwrap(), seconds(11.0), epsilon = 0.1);
    }

    #[test]
    fn moving_stone_out_y() {
        let sheet = sheet::Parameters::default();
        let state = state::Moving {
            t0: seconds(20.0),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 {
                    x: feet(3.0),
                    y: sheet.geometry.playing_end.back_line_y + sheet.stone_radius - feet(1.0),
                },
                v0: Vector2 { x: feet_per_second(-0.1), y: feet_per_second(1.0) },
                a: Vector2 { x: feet_per_second_squared(0.001), y: feet_per_second_squared(0.03) },
            },

            rotation: Rotation::CounterClockwise,
        };
        let stone = Stone { team: Team::A, state: State::Moving(state) };
        assert_approx_eq!(stone.when_outside_y(&sheet).unwrap(), seconds(21.0), epsilon = 0.1);
    }

    #[test]
    fn releasing() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(0.2),
            rotation_acc: feet_per_second_squared(0.03),
            stone_radius: inches(6.0),
            ..sheet::Parameters::default()
        };
        // CounterClockwise
        let mut state = state::BeingDelivered {
            release_time: seconds(3.0),
            starting_point: Vector2 { x: inches(6.0), y: feet(6.0) },
            delivering_off: Vector2 { x: feet(-1.0), y: feet(30.0) },
            rotation: Rotation::CounterClockwise,
        };
        let mut stone = Stone { team: Team::A, state: State::BeingDelivered(state.clone()) };
        stone.next_stage(&sheet);
        let new_state = if let State::Moving(state) = stone.state.clone() {
            state
        } else {
            panic!("Wrong state after release");
        };
        assert_approx_eq!(new_state.t0, seconds(3.0));
        assert_approx_eq!(new_state.motion.s0.x, feet(-1.0) + inches(6.0));
        assert_approx_eq!(new_state.motion.s0.y, feet(36.0));
        assert_approx_eq!(new_state.motion.v0.x, feet_per_second(-1.0 / 3.0));
        assert_approx_eq!(new_state.motion.v0.y, feet_per_second(10.0));
        assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(1.1 / 901.0_f32.sqrt()));
        assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-5.97 / 901.0_f32.sqrt()));
        assert_eq!(new_state.rotation, Rotation::CounterClockwise);

        // Clockwise
        state.rotation = Rotation::Clockwise;
        let mut stone = Stone { team: Team::A, state: State::BeingDelivered(state) };
        stone.next_stage(&sheet);
        let new_state_cw = if let State::Moving(state) = stone.state.clone() {
            state
        } else {
            panic!("Wrong state after release");
        };
        assert_approx_eq!(new_state_cw.t0, new_state.t0);
        assert_approx_eq!(new_state_cw.motion.s0.x, new_state.motion.s0.x);
        assert_approx_eq!(new_state_cw.motion.s0.y, new_state.motion.s0.y);
        assert_approx_eq!(new_state_cw.motion.v0.x, new_state.motion.v0.x);
        assert_approx_eq!(new_state_cw.motion.v0.y, new_state.motion.v0.y);
        assert_approx_eq!(
            new_state_cw.motion.a.x,
            feet_per_second_squared(-0.7 / 901.0_f32.sqrt())
        );
        assert_approx_eq!(
            new_state_cw.motion.a.y,
            feet_per_second_squared(-6.03 / 901.0_f32.sqrt())
        );
        assert_eq!(new_state_cw.rotation, Rotation::Clockwise);
    }

    #[test]
    fn moving_stone_next_quantum() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(0.2),
            rotation_acc: feet_per_second_squared(0.03),
            stone_radius: inches(6.0),
            ..sheet::Parameters::default()
        };
        let state = state::Moving {
            t0: seconds(2.0),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(-3.0), y: feet_per_second(4.0) },
                a: Vector2 {
                    x: feet_per_second_squared(0.096),
                    y: feet_per_second_squared(-0.178),
                },
            },
            rotation: Rotation::Clockwise,
        };
        let mut stone = Stone { team: Team::A, state: State::Moving(state) };
        stone.next_time_quantum(seconds(4.0), &sheet);
        let new_state = if let State::Moving(state) = stone.state {
            state
        } else {
            panic!("Wrong state after next time quantum")
        };
        assert_approx_eq!(new_state.t0, seconds(4.0));
        assert_approx_eq!(new_state.motion.s0.x, feet(-2.808));
        assert_approx_eq!(new_state.motion.s0.y, feet(107.644));
        assert_approx_eq!(new_state.motion.v0.x, feet_per_second(-2.808));
        assert_approx_eq!(new_state.motion.v0.y, feet_per_second(3.644));
        let v = new_state.motion.v0.norm().value;
        assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(0.45228 / v));
        assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-0.81304 / v));
        assert_eq!(new_state.rotation, Rotation::Clockwise)
    }

    #[test]
    fn stopping_stone() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(1.0),
            rotation_acc: feet_per_second_squared(0.1),
            ..sheet::Parameters::default()
        };
        let state = state::Moving {
            t0: seconds(2.0),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                a: Vector2 { x: feet_per_second_squared(0.52), y: feet_per_second_squared(-0.86) },
            },
            rotation: Rotation::Clockwise,
        };
        let mut stone = Stone { team: Team::A, state: State::Moving(state) };
        stone.next_stage(&sheet);
        let new_state = if let State::Stationary(state) = stone.state {
            state
        } else {
            panic!("Wrong state after next stage")
        };
        assert_approx_eq!(new_state.position().x, feet(2.915));
        assert_approx_eq!(new_state.position().y, feet(100.0925));
    }

    #[test]
    fn collision_with_stationary() {
        let sheet = sheet::Parameters {
            stone_radius: feet(0.5),
            friction: feet_per_second_squared(0.5),
            static_friction: feet_squared_per_second_squared(55.0 / 512.0),
            ..sheet::Parameters::default()
        };
        let moving = Stone {
            team: Team::A,
            state: State::Moving(state::Moving {
                t0: seconds(2.0),
                motion: motion::UniformlyAccelerated {
                    s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                    v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                    a: Acceleration::default(),
                },
                rotation: Rotation::None,
            }),
        };
        let stationary = Stone {
            team: Team::B,
            state: State::Stationary(state::Stationary::new(Vector2 {
                x: feet(1.8),
                y: feet(101.6),
            })),
        };
        let collision_time = moving.when_collision(&stationary, &sheet).expect("Stone will miss");
        assert_approx_eq!(collision_time, seconds(4.0), ulps = 10);

        let (new_moving_state, new_stationary_state) = moving
            .states_after_collision(&stationary, &sheet, collision_time)
            .expect("Stones missed.");
        if let State::Stationary(new_state) = new_moving_state {
            assert_approx_eq!(new_state.position().x, feet(2.4), ulps = 10);
            assert_approx_eq!(new_state.position().y, feet(100.8), ulps = 10);
        } else {
            panic!("Wrong new state of moving stone");
        }
        if let State::Moving(new_state) = new_stationary_state {
            assert_approx_eq!(new_state.t0, collision_time);
            assert_eq!(new_state.motion.s0.x, feet(1.8));
            assert_eq!(new_state.motion.s0.y, feet(101.6));
            assert_approx_eq!(new_state.motion.v0.x, feet_per_second(-0.1125), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.v0.y, feet_per_second(0.15), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(0.3), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-0.4), epsilon = 1e-6);
            assert_eq!(new_state.rotation, Rotation::None);
        } else {
            panic!("Wrong new state of stationary stone");
        }
    }

    // #[test]
    // fn collision_with_stationary2() {
    //     let sheet = sheet::Parameters::default();
    //     let moving = Stone {
    //         team: Team::A,
    //         state: State::Moving(state::Moving {
    //             t0: seconds(25.5),
    //             motion: motion::UniformlyAccelerated {
    //                 s0: Vector2 { x: feet(1.2736267), y: feet(131.15353) },
    //                 v0: Vector2 { x: feet_per_second(0.32272235), y: feet_per_second(0.51202404) },
    //                 a: Vector2 {
    //                     x: feet_per_second_squared(-0.14046976),
    //                     y: feet_per_second_squared(-0.22286618),
    //                 },
    //             },
    //             rotation: Rotation::None,
    //         }),
    //     };
    //     let stationary = Stone {
    //         team: Team::B,
    //         state: State::Stationary(state::Stationary::new(Vector2 {
    //             x: feet(2.0472093),
    //             y: feet(130.66595),
    //         })),
    //     };
    //     let collision_time = moving.when_collision(&stationary, &sheet).expect("Stone will miss");
    //     assert_approx_eq!(collision_time, seconds(25.5));
    //
    //     let (new_moving_state, new_stationary_state) = dbg!(moving
    //         .states_after_collision(&stationary, &sheet, collision_time)
    //         .expect("Stones missed."));
    //     if let State::Stationary(new_state) = new_moving_state {
    //         assert_approx_eq!(new_state.position().x, feet(2.4), ulps = 10);
    //         assert_approx_eq!(new_state.position().y, feet(100.8), ulps = 10);
    //     } else {
    //         panic!("Wrong new state of moving stone");
    //     }
    //     if let State::Moving(new_state) = new_stationary_state {
    //         assert_approx_eq!(new_state.t0, collision_time);
    //         assert_eq!(new_state.motion.s0.x, feet(1.8));
    //         assert_eq!(new_state.motion.s0.y, feet(101.6));
    //         assert_approx_eq!(new_state.motion.v0.x, feet_per_second(-0.1125), epsilon = 1e-6);
    //         assert_approx_eq!(new_state.motion.v0.y, feet_per_second(0.15), epsilon = 1e-6);
    //         assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(0.3), epsilon = 1e-6);
    //         assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-0.4), epsilon = 1e-6);
    //         assert_eq!(new_state.rotation, Rotation::None);
    //     } else {
    //         panic!("Wrong new state of stationary stone");
    //     }
    // }

    #[test]
    fn collision_with_moving() {
        let sheet = sheet::Parameters {
            stone_radius: feet(0.5),
            friction: feet_per_second_squared(0.5),
            static_friction: feet_squared_per_second_squared(55.0 / 512.0),
            ..sheet::Parameters::default()
        };
        let left_stone = Stone {
            team: Team::A,
            state: State::Moving(state::Moving {
                t0: seconds(2.0),
                motion: motion::UniformlyAccelerated {
                    s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                    v0: Vector2 { x: feet_per_second(0.3), y: feet_per_second(0.4) },
                    a: Acceleration::default(),
                },
                rotation: Rotation::None,
            }),
        };
        let right_stone = Stone {
            team: Team::B,
            state: State::Moving(state::Moving {
                t0: seconds(2.0),
                motion: motion::UniformlyAccelerated {
                    s0: Vector2 { x: feet(5.8), y: feet(100.0) },
                    v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                    a: Acceleration::default(),
                },
                rotation: Rotation::None,
            }),
        };
        let collision_time =
            left_stone.when_collision(&right_stone, &sheet).expect("Stone will miss");
        let another_time =
            right_stone.when_collision(&left_stone, &sheet).expect("Stone will miss");
        assert_approx_eq!(collision_time, seconds(5.0), epsilon = 1e-6);
        assert_approx_eq!(another_time, seconds(5.0), epsilon = 1e-6);

        let (new_left_state, new_right_state) = left_stone
            .states_after_collision(&right_stone, &sheet, collision_time)
            .expect("Stones missed.");
        if let State::Moving(new_state) = new_left_state {
            assert_approx_eq!(new_state.t0, collision_time);
            assert_eq!(new_state.motion.s0.x, feet(3.9));
            assert_eq!(new_state.motion.s0.y, feet(101.2));
            assert_approx_eq!(new_state.motion.v0.x, feet_per_second(-0.3), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.v0.y, feet_per_second(0.4), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(0.3), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-0.4), epsilon = 1e-6);
        } else {
            panic!("Wrong new state of moving stone");
        }
        if let State::Moving(new_state) = new_right_state {
            assert_approx_eq!(new_state.t0, collision_time);
            assert_eq!(new_state.motion.s0.x, feet(4.9));
            assert_eq!(new_state.motion.s0.y, feet(101.2));
            assert_approx_eq!(new_state.motion.v0.x, feet_per_second(0.3), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.v0.y, feet_per_second(0.4), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.x, feet_per_second_squared(-0.3), epsilon = 1e-6);
            assert_approx_eq!(new_state.motion.a.y, feet_per_second_squared(-0.4), epsilon = 1e-6);
            assert_eq!(new_state.rotation, Rotation::None);
        } else {
            panic!("Wrong new state of stationary stone");
        }
    }
}
