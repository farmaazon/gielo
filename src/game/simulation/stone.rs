use crate::{
    game::{
        sheet,
        simulation::{motion, Simulation},
        stone::{Acceleration, Position, Rotation, Velocity},
    },
    unit::{float_eq, seconds, Time},
    vector::{EuclideanNorm, Vector2},
};
use decorum::NotNan;
use uom::{si::time::second, ConstZero};

pub use crate::game::stone::Id;
use crate::unit;

#[derive(Copy, Clone, Debug, Default)]
pub struct MovingStone {
    pub t0: Time,
    pub t1: Time,
    pub motion: motion::UniformlyAccelerated,
    pub rotation: Rotation,
}

impl MovingStone {
    pub fn new_stationary(position: Position) -> Self {
        Self {
            t0: Time::ZERO,
            t1: seconds(unit::BaseType::INFINITY),
            motion: motion::UniformlyAccelerated {
                s0: position,
                v0: Velocity::ZERO,
                a: Acceleration::ZERO,
            },
            rotation: Rotation::None,
        }
    }

    pub fn new_delivered(s0: Position, v0: Velocity) -> Self {
        Self {
            t0: Time::ZERO,
            t1: seconds(unit::BaseType::INFINITY),
            motion: motion::UniformlyAccelerated { s0, v0, a: Acceleration::ZERO },
            rotation: Rotation::None,
        }
    }

    pub fn is_moving(&self) -> bool {
        self.motion.v0 != Velocity::ZERO
    }

    pub fn position(&self, t: Time) -> Position {
        self.motion.position(t - self.t0)
    }

    pub fn velocity(&self, t: Time) -> Velocity {
        self.motion.velocity(t - self.t0)
    }

    pub fn motion_at_t(&self, t: Time) -> motion::UniformlyAccelerated {
        motion::UniformlyAccelerated {
            s0: self.position(t),
            v0: self.velocity(t),
            a: self.motion.a,
        }
    }

    fn jump_t0_to(&mut self, t: Time) {
        self.motion = self.motion_at_t(t);
        self.t0 = t;
    }
}

#[derive(Debug)]
pub struct NextStoneEvent<'a, 'b> {
    pub stone: &'a MovingStone,
    pub sheet_params: &'b sheet::Parameters,
}

impl<'a, 'b> NextStoneEvent<'a, 'b> {
    pub fn when_stop(&self) -> Option<Time> {
        self.stone.is_moving().then(|| {
            let v = self.stone.motion.v0.norm();
            self.stone.t0 + (v / self.sheet_params.friction)
        })
    }

    pub fn when_outside_x(&self) -> Option<Time> {
        let geometry = &self.sheet_params.geometry;
        [(geometry.left_bound(), -1.0), (geometry.right_bound(), 1.0)]
            .into_iter()
            .filter_map(|(bound, x_sign)| {
                let out_x = bound - x_sign * self.sheet_params.stone_radius;
                self.stone.motion.when_at_x(out_x).map(|t| t + self.stone.t0)
            })
            .min_by_key(|t| NotNan::from_inner(t.get::<second>()))
    }

    pub fn when_outside_y(&self) -> Option<Time> {
        let sheet_params = &self.sheet_params;
        let out_y = sheet_params.geometry.playing_end.back_line_y + sheet_params.stone_radius;
        self.stone.motion.when_at_y(out_y).map(|t| t + self.stone.t0)
    }

    pub fn when_collision(&self, rhs: &MovingStone) -> Option<Time> {
        let t0 = self.stone.t0.max(rhs.t0);
        let motion = self.stone.motion_at_t(t0) - rhs.motion_at_t(t0);
        motion.when_hits_circle(self.sheet_params.stone_radius * 2.0).map(|t| t + t0)
    }
}

#[derive(Debug)]
pub struct Update<'a, 'b, 'c> {
    pub stone: &'a mut MovingStone,
    pub simulation: &'b Simulation,
    pub sheet_params: &'c sheet::Parameters,
}

impl<'a, 'b, 'c> Update<'a, 'b, 'c> {
    pub fn release(&mut self, t: Time, rotation: Rotation) {
        self.stone.jump_t0_to(t);
        self.stone.rotation = rotation;
        self.recompute_acc_and_t1();
    }

    pub fn set_next_time_quantum(&mut self) {
        self.stone.jump_t0_to(self.stone.t1);
        self.recompute_acc_and_t1();
    }

    pub fn stop(&mut self, t: Time) {
        self.stone.jump_t0_to(t);
        self.stone.motion.v0 = Velocity::ZERO;
        self.stone.motion.a = Acceleration::ZERO;
        self.stone.t1 = seconds(unit::BaseType::INFINITY);
    }

    pub fn remove(&mut self) {
        *self.stone =
            MovingStone { t1: seconds(unit::BaseType::INFINITY), ..MovingStone::default() }
    }

    pub fn collision(&mut self, rhs: &mut MovingStone, t: Time) {
        self.stone.jump_t0_to(t);
        rhs.jump_t0_to(t);
        let lhs_pos = self.stone.motion.s0;
        let rhs_pos = rhs.motion.s0;
        let lhs_v = self.stone.motion.v0;
        let rhs_v = rhs.motion.v0;
        let (mut new_lhs_v, mut new_rhs_v) =
            motion::velocity_after_collision(lhs_pos, lhs_v, rhs_pos, rhs_v);
        if lhs_v == Velocity::ZERO {
            new_lhs_v = motion::decrease_energy(new_lhs_v, self.sheet_params.static_friction);
        }
        self.stone.motion.v0 = new_lhs_v;
        self.stone.rotation = Rotation::None;
        self.recompute_acc_and_t1();
        if rhs_v == Velocity::ZERO {
            new_rhs_v = motion::decrease_energy(new_rhs_v, self.sheet_params.static_friction);
        }
        rhs.motion.v0 = new_rhs_v;
        rhs.rotation = Rotation::None;
        let mut lhs_update =
            Update { stone: rhs, simulation: self.simulation, sheet_params: self.sheet_params };
        lhs_update.recompute_acc_and_t1();
    }

    fn recompute_acc_and_t1(&mut self) {
        let v0 = self.stone.motion.v0;
        let v = v0.norm();
        if float_eq!(v, unit::Length::ZERO) {
            self.stone.motion.a = Acceleration::ZERO;
            self.stone.t1 = seconds(unit::BaseType::INFINITY)
        } else {
            let a_friction = (-v0 / v) * self.sheet_params.friction;
            let a_rotation = match self.stone.rotation {
                Rotation::None => Vector2::ZERO,
                Rotation::Clockwise => Vector2 { x: -v0.y, y: v0.x },
                Rotation::CounterClockwise => Vector2 { x: v0.y, y: -v0.x },
            } * self.sheet_params.rotation_acc
                / v;
            self.stone.motion.a = a_friction + a_rotation;
            debug_assert!(!self.stone.motion.a.x.is_nan());
            debug_assert!(!self.stone.motion.a.y.is_nan());
            self.stone.t1 = match self.stone.rotation {
                Rotation::None => seconds(unit::BaseType::INFINITY),
                _ => {
                    let adaptive_quantum =
                        self.simulation.time_quantum_factor * self.stone.motion.v0.norm();
                    let time_quantum = adaptive_quantum
                        .min(self.simulation.parameters.max_time_quantum)
                        .max(self.simulation.parameters.min_time_quantum);
                    self.stone.t0 + time_quantum
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::simulation,
        unit::{
            assert_float_eq, feet, feet_per_second, feet_per_second_squared,
            feet_squared_per_second_squared, inches, milliseconds,
        },
    };

    #[test]
    fn moving_stone_properties() {
        let stone = MovingStone {
            t0: seconds(1.5),
            t1: seconds(unit::BaseType::INFINITY),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(-1.0), y: feet(30.0) },
                v0: Vector2 { x: feet_per_second(-0.01), y: feet_per_second(3.0) },
                a: Vector2 { x: feet_per_second_squared(0.003), y: feet_per_second_squared(-0.05) },
            },
            rotation: Rotation::CounterClockwise,
        };

        let t = seconds(2.0);
        let velocity = stone.velocity(t);
        assert_float_eq!(velocity.x, feet_per_second(-0.0085));
        assert_float_eq!(velocity.y, feet_per_second(2.975));
        let position = stone.position(t);
        assert_float_eq!(position.x, feet(-1.004625));
        assert_float_eq!(position.y, feet(31.49375));

        let t = seconds(3.5);
        let velocity = stone.velocity(t);
        assert_float_eq!(velocity.x, feet_per_second(-0.004));
        assert_float_eq!(velocity.y, feet_per_second(2.9));
        let position = stone.position(t);
        assert_float_eq!(position.x, feet(-1.014));
        assert_float_eq!(position.y, feet(35.9));
    }

    #[test]
    fn moving_stone_out_x() {
        let sheet = sheet::Parameters::default();
        let stone = MovingStone {
            t0: seconds(10.0),
            t1: seconds(unit::BaseType::INFINITY),
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
        let next_event = NextStoneEvent { stone: &stone, sheet_params: &sheet };
        assert_float_eq!(next_event.when_outside_x().unwrap(), seconds(11.0), abs <= 0.1);
    }

    #[test]
    fn moving_stone_out_y() {
        let sheet = sheet::Parameters::default();
        let stone = MovingStone {
            t0: seconds(20.0),
            t1: seconds(unit::BaseType::INFINITY),
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
        let next_event = NextStoneEvent { stone: &stone, sheet_params: &sheet };
        assert_float_eq!(next_event.when_outside_y().unwrap(), seconds(21.0), abs <= 0.1);
    }

    #[test]
    fn moving_stone_next_quantum() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(0.2),
            rotation_acc: feet_per_second_squared(0.03),
            stone_radius: inches(6.0),
            ..sheet::Parameters::default()
        };
        let simulation = Simulation::new_mock(
            simulation::Parameters {
                min_time_quantum: milliseconds(100.0),
                max_time_quantum: seconds(10.0),
            },
            seconds(1.0) / feet_per_second(3.0),
        );
        let mut stone = MovingStone {
            t0: seconds(2.0),
            t1: seconds(4.0),
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
        let mut update =
            Update { stone: &mut stone, simulation: &simulation, sheet_params: &sheet };
        update.set_next_time_quantum();

        assert_float_eq!(stone.t0, seconds(4.0));
        assert_float_eq!(stone.motion.s0.x, feet(-2.808));
        assert_float_eq!(stone.motion.s0.y, feet(107.644));
        assert_float_eq!(stone.motion.v0.x, feet_per_second(-2.808));
        assert_float_eq!(stone.motion.v0.y, feet_per_second(3.644));
        let v = stone.motion.v0.norm().value;
        assert_float_eq!(stone.motion.a.x, feet_per_second_squared(0.45228 / v));
        assert_float_eq!(stone.motion.a.y, feet_per_second_squared(-0.81304 / v));
        assert_float_eq!(stone.t1, seconds(5.533463762568621));
        assert_eq!(stone.rotation, Rotation::Clockwise);
    }

    #[test]
    fn stopping_stone() {
        let sheet = sheet::Parameters {
            friction: feet_per_second_squared(1.0),
            rotation_acc: feet_per_second_squared(0.1),
            ..sheet::Parameters::default()
        };
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet);
        let mut stone = MovingStone {
            t0: seconds(2.0),
            t1: seconds(unit::BaseType::INFINITY),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                a: Vector2 { x: feet_per_second_squared(0.52), y: feet_per_second_squared(-0.86) },
            },
            rotation: Rotation::Clockwise,
        };
        let when_stopped = NextStoneEvent { stone: &stone, sheet_params: &sheet }
            .when_stop()
            .expect("Stone won't stop");
        assert_float_eq!(when_stopped, seconds(2.5));

        let mut update =
            Update { stone: &mut stone, simulation: &simulation, sheet_params: &sheet };
        update.stop(when_stopped);
        assert_float_eq!(stone.motion.s0.x, feet(2.915));
        assert_float_eq!(stone.motion.s0.y, feet(100.0925));
        assert_float_eq!(stone.t0, seconds(2.5));
        assert_eq!(stone.motion.v0, Velocity::ZERO);
        assert_eq!(stone.motion.a, Acceleration::ZERO);
    }

    #[test]
    fn collision_with_stationary() {
        let sheet = sheet::Parameters {
            stone_radius: feet(0.5),
            friction: feet_per_second_squared(0.5),
            static_friction: feet_squared_per_second_squared(55.0 / 512.0),
            ..sheet::Parameters::default()
        };
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet);
        let mut moving = MovingStone {
            t0: seconds(2.0),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                a: Acceleration::default(),
            },
            rotation: Rotation::None,
            t1: seconds(4.5),
        };
        let mut stationary = MovingStone::new_stationary(Vector2 { x: feet(1.8), y: feet(101.6) });

        let next_event = NextStoneEvent { stone: &moving, sheet_params: &sheet };
        let collision_time = next_event.when_collision(&stationary).expect("Stone will miss");
        assert_float_eq!(collision_time, seconds(4.0));

        let mut update =
            Update { stone: &mut moving, simulation: &simulation, sheet_params: &sheet };
        update.collision(&mut stationary, collision_time);

        assert_float_eq!(moving.t0, collision_time);
        assert_float_eq!(moving.motion.s0.x, feet(2.4));
        assert_float_eq!(moving.motion.s0.y, feet(100.8));
        assert_float_eq!(moving.motion.v0.x, feet_per_second(0.0));
        assert_float_eq!(moving.motion.v0.y, feet_per_second(0.0));
        assert_float_eq!(moving.motion.a.x, feet_per_second(0.0));
        assert_float_eq!(moving.motion.a.y, feet_per_second(0.0));

        assert_float_eq!(stationary.t0, collision_time);
        assert_eq!(stationary.motion.s0.x, feet(1.8));
        assert_eq!(stationary.motion.s0.y, feet(101.6));
        assert_float_eq!(stationary.motion.v0.x, feet_per_second(-0.1125));
        assert_float_eq!(stationary.motion.v0.y, feet_per_second(0.15));
        assert_float_eq!(stationary.motion.a.x, feet_per_second_squared(0.3));
        assert_float_eq!(stationary.motion.a.y, feet_per_second_squared(-0.4));
        assert_eq!(stationary.rotation, Rotation::None);
    }

    #[test]
    fn collision_with_moving() {
        let sheet = sheet::Parameters {
            stone_radius: feet(0.5),
            friction: feet_per_second_squared(0.5),
            static_friction: feet_squared_per_second_squared(55.0 / 512.0),
            ..sheet::Parameters::default()
        };
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet);
        let mut left_stone = MovingStone {
            t0: seconds(2.0),
            t1: seconds(5.5),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(3.0), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(0.3), y: feet_per_second(0.4) },
                a: Acceleration::default(),
            },
            rotation: Rotation::None,
        };
        let mut right_stone = MovingStone {
            t0: seconds(2.0),
            t1: seconds(5.5),
            motion: motion::UniformlyAccelerated {
                s0: Vector2 { x: feet(5.8), y: feet(100.0) },
                v0: Vector2 { x: feet_per_second(-0.3), y: feet_per_second(0.4) },
                a: Acceleration::default(),
            },
            rotation: Rotation::None,
        };
        let left_next_event = NextStoneEvent { stone: &left_stone, sheet_params: &sheet };
        let right_next_event = NextStoneEvent { stone: &right_stone, sheet_params: &sheet };
        let collision_time = left_next_event.when_collision(&right_stone).expect("Stone will miss");
        let another_time = right_next_event.when_collision(&left_stone).expect("Stone will miss");
        assert_float_eq!(collision_time, seconds(5.0));
        assert_float_eq!(another_time, seconds(5.0));

        let mut update =
            Update { stone: &mut left_stone, simulation: &simulation, sheet_params: &sheet };
        update.collision(&mut right_stone, collision_time);

        assert_float_eq!(left_stone.t0, collision_time);
        assert_float_eq!(left_stone.motion.s0.x, feet(3.9));
        assert_float_eq!(left_stone.motion.s0.y, feet(101.2));
        assert_float_eq!(left_stone.motion.v0.x, feet_per_second(-0.3));
        assert_float_eq!(left_stone.motion.v0.y, feet_per_second(0.4));
        assert_float_eq!(left_stone.motion.a.x, feet_per_second_squared(0.3));
        assert_float_eq!(left_stone.motion.a.y, feet_per_second_squared(-0.4));
        assert_eq!(left_stone.rotation, Rotation::None);

        assert_float_eq!(right_stone.t0, collision_time);
        assert_float_eq!(right_stone.motion.s0.x, feet(4.9));
        assert_float_eq!(right_stone.motion.s0.y, feet(101.2));
        assert_float_eq!(right_stone.motion.v0.x, feet_per_second(0.3));
        assert_float_eq!(right_stone.motion.v0.y, feet_per_second(0.4));
        assert_float_eq!(right_stone.motion.a.x, feet_per_second_squared(-0.3));
        assert_float_eq!(right_stone.motion.a.y, feet_per_second_squared(-0.4));
        assert_eq!(right_stone.rotation, Rotation::None);
    }
}
