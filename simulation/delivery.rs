use crate::{Simulation, stone};
use decorum::NotNan;
use derive_more::{Deref, DerefMut};
use gielo_sheet as sheet;
use gielo_sheet::stone::Rotation;
use gielo_unit::{Angle, BaseType, Time, Velocity, float_eq, vector::Vector2};
use itertools::Itertools;
use sheet::stone::Stones;
use std::cmp;
use uom::{ConstZero, si::time::second};

pub use crate::delivery::event::Event;
use crate::stone::MovingStone;

const EVENT_LIMIT_IN_SINGLE_RUN: usize = 1000;

pub mod event {
    use crate::stone;
    use gielo_unit::Time;

    #[derive(Copy, Clone, Debug)]
    pub enum Kind {
        Release,
        StoneOut,
        StoneStopped,
        Collision { with: stone::Id },
        NextTimeQuantum,
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Event {
        pub time: Time,
        pub kind: Kind,
        pub stone: stone::Id,
    }

    impl Event {
        pub fn from_times(
            kind: Kind,
            stone: stone::Id,
            times: impl IntoIterator<Item = Time>,
        ) -> impl Iterator<Item = Event> {
            times.into_iter().map(move |time| Self { kind, stone, time })
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StartingConditions {
    pub stone: stone::Id,
    pub angle: Angle,
    pub velocity: Velocity,
    pub hack: sheet::Hack,
    pub rotation: Rotation,
}

#[derive(Clone, Debug)]
pub struct Process {
    pub stones: [MovingStone; sheet::stone::COUNT],
    pub delivered_stone: stone::Id,
    pub rotation: Rotation,
    pub current_time: Time,
    pub collision_happened: bool,
    next_event_cached: Option<Event>,
}

impl Process {
    pub fn new(
        StartingConditions { stone, angle, velocity, hack, rotation }: StartingConditions,
        stones: &mut Stones,
        sheet_params: &sheet::Parameters,
        dirty: &mut sheet::stone::Flag,
    ) -> Self {
        let delivered_initial_pos = sheet_params.geometry.hack_pos(hack);
        stones.put_stone(dirty, stone, delivered_initial_pos);
        let mut stones = stones.positions().map(MovingStone::new_stationary);
        let delivering_dist = sheet_params.geometry.delivery_dist();
        let release_time = delivering_dist / velocity;
        stones[stone].motion.v0 = Vector2 { x: velocity * angle.sin(), y: velocity * angle.cos() };
        stones[stone].t1 = release_time;
        Self {
            stones,
            delivered_stone: stone,
            rotation,
            current_time: Time::ZERO,
            collision_happened: false,
            next_event_cached: Some(Event {
                time: release_time,
                stone,
                kind: event::Kind::Release,
            }),
        }
    }
}

pub enum UpdateResult<'a> {
    StoppedAtEvent(&'a Event),
    StoppedAtTime(Time),
    Finished,
}

#[derive(Debug, Deref, DerefMut)]
pub struct Update<'a, 'b, 'c, 'd, 'e> {
    #[deref]
    #[deref_mut]
    pub process: &'a mut Process,
    pub sheet: &'b mut Stones,
    pub sheet_params: &'c sheet::Parameters,
    pub simulation: &'d Simulation,
    pub dirty: &'e mut sheet::stone::Flag,
}

impl<'a, 'b, 'c, 'd, 'e> Update<'a, 'b, 'c, 'd, 'e> {
    pub fn next_event(&mut self, mut until_predicate: impl FnMut(&Event) -> bool) -> Option<Event> {
        let not_yet_cached_event =
            self.process.next_event_cached.as_ref().is_some_and(&mut until_predicate);
        if not_yet_cached_event {
            None
        } else if let Some(event) = self.process.next_event_cached.take() {
            Some(event)
        } else {
            let next_event = self.compute_next_event();
            let to_cache = next_event.as_ref().is_some_and(until_predicate);
            if to_cache {
                self.process.next_event_cached = next_event;
                None
            } else {
                next_event
            }
        }
    }

    fn compute_next_event(&self) -> Option<Event> {
        let key = |event: &Event| NotNan::<BaseType>::assert(event.time.get::<second>());
        let events = self.sheet.in_play().iter_ids().flat_map(|stone| self.stone_events(stone));
        events
            .inspect(|e| log::debug!("Considering event {e:?}"))
            .filter(|event| event.time >= self.process.current_time)
            .min_by_key(key)
    }

    fn stone_events(&self, id: stone::Id) -> impl Iterator<Item = Event> + '_ {
        let next_event = stone::NextStoneEvent::new(&self.process.stones[id], self.sheet_params);
        let t1 = next_event.stone.t1;
        let next_quantum = t1.is_finite().then_some(Event {
            kind: event::Kind::NextTimeQuantum,
            stone: id,
            time: t1,
        });
        let outside_x = Event::from_times(event::Kind::StoneOut, id, next_event.when_outside_x());
        let outside_y = Event::from_times(event::Kind::StoneOut, id, next_event.when_outside_y());
        let stopped = Event::from_times(event::Kind::StoneStopped, id, next_event.when_stop());
        let next_stones =
            sheet::stone::Flag::range((id + 1)..sheet::stone::COUNT) & self.sheet.in_play();
        let collisions = next_stones.iter_ids().flat_map(move |with| {
            let rhs = &self.process.stones[with];
            Event::from_times(event::Kind::Collision { with }, id, next_event.when_collision(rhs))
        });
        stopped.chain(outside_x).chain(outside_y).chain(collisions).chain(next_quantum)
    }

    pub fn apply_event(&mut self, event: Event) {
        log::debug!("Applying event {event:?}");
        let Event { time, stone: stone_id, kind } = event;
        self.process.current_time = time;
        let (stones_before, rest) = self.process.stones.split_at_mut(stone_id);
        let (stone, stones_after) = rest.split_first_mut().unwrap();
        let mut stone_update =
            stone::Update { stone, simulation: self.simulation, sheet_params: self.sheet_params };
        match kind {
            event::Kind::Release => stone_update.release(time, self.process.rotation),
            event::Kind::StoneOut => {
                stone_update.remove();
                self.sheet.remove_stone(self.dirty, stone_id);
            }
            event::Kind::StoneStopped => {
                stone_update.stop(time);
                let stone_back =
                    stone_update.stone.motion.s0.y - stone_update.sheet_params.stone_radius;
                let hog_line = stone_update.sheet_params.geometry.playing_end.hog_line_y;
                let before_hog = stone_back < hog_line || float_eq!(stone_back, hog_line);
                if before_hog && !self.process.collision_happened {
                    self.sheet.remove_stone(self.dirty, stone_id);
                } else {
                    self.sheet.set_position(self.dirty, stone_id, stone_update.stone.motion.s0);
                }
            }
            event::Kind::Collision { with } => {
                let with_stone = match stone_id.cmp(&with) {
                    cmp::Ordering::Less => &mut stones_after[with - stone_id - 1],
                    cmp::Ordering::Greater => &mut stones_before[with],
                    cmp::Ordering::Equal => panic!("Stone collided with itself"),
                };
                stone_update.collision(with_stone, time);
                self.process.collision_happened = true;
            }
            event::Kind::NextTimeQuantum => stone_update.set_next_time_quantum(),
        }
        log::debug!(
            "Situation after applying event: \n{:?}",
            self.sheet.iter_in_play().map(|(index, _)| (index, self.stones[index])).format(",")
        );
    }

    /// Returns true when finished.
    pub fn run(&mut self, time: Option<Time>) -> bool {
        let mut limit = std::iter::repeat_n((), EVENT_LIMIT_IN_SINGLE_RUN);
        while let Some(event) = self.next_event(|event| time.is_some_and(|t| event.time > t)) {
            if limit.next().is_none() {
                panic!("Event limit exceeded!");
            }
            self.apply_event(event);
        }
        if let Some(t) = time {
            self.process.current_time = t;
        }
        self.update_positions_in_sheet();
        self.process.next_event_cached.is_none()
    }

    pub fn trace_until_event(&mut self, mut callback: impl FnMut(&Self)) {
        while let Some(event) = self.next_event(|event| {
            !matches!(event.kind, event::Kind::Release | event::Kind::NextTimeQuantum)
        }) {
            self.apply_event(event);
            self.update_positions_in_sheet();
            callback(self);
        }
        if let Some(event) = &self.next_event_cached {
            self.current_time = event.time;
            self.update_positions_in_sheet();
            callback(self);
        }
    }

    fn update_positions_in_sheet(&mut self) {
        let moving_stones =
            self.process.stones.iter().enumerate().filter(|(_, stone)| stone.is_moving());
        for (id, stone) in moving_stones {
            self.sheet.set_position(self.dirty, id, stone.position(self.process.current_time));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation;
    use gielo_sheet::{
        Hack,
        stone::{Flag, Position},
    };
    use gielo_unit::{
        Length, assert_float_eq, base_type::consts::PI, feet, feet_per_second,
        feet_per_second_squared, feet_squared_per_second_squared, inches, radians, seconds,
        vector::EuclideanNorm,
    };

    struct DeliveryTest {
        stones: Stones,
        sheet_params: sheet::Parameters,
        simulation: Simulation,
        dirty: Flag,
        process: Process,
    }

    impl DeliveryTest {
        fn set_up(
            angle: Angle,
            velocity: Velocity,
            rotation: Rotation,
            delivered_stone: stone::Id,
            other_stones: impl IntoIterator<Item = (stone::Id, Position)>,
        ) -> Self {
            let sheet_params = sheet::Parameters {
                geometry: sheet::parameters::Geometry::default(),
                friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
                rotation_acc: feet_per_second_squared(245.0 / 8649.0),
                stone_radius: inches(18.0 / PI),
                static_friction: feet_squared_per_second_squared(0.25),
            };
            let mut stones = other_stones.into_iter().collect();
            let simulation = Simulation::new(simulation::Parameters::default(), &sheet_params);
            let mut dirty = Flag(0);
            let start = StartingConditions {
                stone: delivered_stone,
                angle,
                velocity,
                hack: Hack::Left,
                rotation,
            };
            let process = Process::new(start, &mut stones, &sheet_params, &mut dirty);
            dirty.assert_and_clear(Flag::stone(delivered_stone));
            stones.in_play().contains(delivered_stone);

            Self { stones, sheet_params, dirty, process, simulation }
        }

        fn run_update(&mut self, until: Time, expected_result: bool) {
            let mut update = Update {
                process: &mut self.process,
                sheet: &mut self.stones,
                sheet_params: &self.sheet_params,
                simulation: &self.simulation,
                dirty: &mut self.dirty,
            };
            assert_eq!(update.run(Some(until)), expected_result);
        }
    }

    #[test]
    fn inaccurate_tee_shot() {
        let mut test = DeliveryTest::set_up(
            radians(6.0 / 132.0),
            feet_per_second(7.0),
            Rotation::Clockwise,
            0,
            [],
        );
        test.run_update(seconds(2.0), false);
        test.dirty.assert_and_clear(Flag::stone(0));

        let mut check_moving = |time: Time, exp_pos: Position| {
            test.run_update(time, false);
            let position = test.stones.positions()[0];
            assert_float_eq!(position.x, exp_pos.x, abs <= 2.0);
            assert_float_eq!(position.y, exp_pos.y, abs <= 2.0);
            assert_eq!(test.stones.in_play(), Flag::stone(0));
            test.dirty.assert_and_clear(Flag::stone(0));
        };

        check_moving(seconds(5.1), Vector2 { x: feet(2.0), y: feet(40.0) });
        check_moving(seconds(12.5), Vector2 { x: feet(3.5), y: feet(85.0) });
        check_moving(seconds(20.0), Vector2 { x: feet(3.0), y: feet(115.0) });
        check_moving(seconds(30.2), Vector2 { x: feet(1.0), y: feet(130.0) });

        test.run_update(seconds(100.0), true);
        let position = test.stones.positions()[0];
        assert_float_eq!(position.x, feet(0.0), abs <= 2.0);
        assert_float_eq!(position.y, test.sheet_params.geometry.playing_end.tee_line_y, abs <= 2.0);
        assert_eq!(test.stones.in_play(), Flag::stone(0));
        test.dirty.assert_and_clear(Flag::stone(0));
    }

    #[test]
    fn too_strong() {
        let mut test = DeliveryTest::set_up(
            radians(6.0 / 132.0),
            feet_per_second(7.5),
            Rotation::Clockwise,
            4,
            [],
        );
        test.run_update(seconds(60.0), true);
        assert_eq!(test.stones.in_play(), Flag(0));
        test.dirty.assert_and_clear(Flag::stone(4));
    }

    #[test]
    fn too_weak() {
        let mut test = DeliveryTest::set_up(
            radians(6.0 / 132.0),
            feet_per_second(6.15),
            Rotation::Clockwise,
            4,
            [],
        );
        test.run_update(seconds(60.0), true);
        assert_eq!(test.stones.in_play(), Flag(0));
        test.dirty.assert_and_clear(Flag::stone(4));
    }

    #[test]
    fn clear_stone() {
        let tee = sheet::parameters::Geometry::default().tee();
        let delivered = 0;
        let taken_out = 8;
        let mut test = DeliveryTest::set_up(
            radians(-2.0 / 132.0),
            feet_per_second(9.13),
            Rotation::CounterClockwise,
            delivered,
            [(taken_out, tee)],
        );

        test.run_update(seconds(15.0), false);
        assert!(!test.process.stones[taken_out].is_moving());
        assert_eq!(test.stones.positions()[taken_out], tee);
        assert!(test.process.stones[delivered].is_moving());
        test.dirty.assert_and_clear(Flag::stone(delivered));

        test.run_update(seconds(16.0), false);
        assert!(test.process.stones[taken_out].is_moving());
        assert!(test.process.stones[delivered].is_moving());
        test.dirty.assert_and_clear(Flag::stone(taken_out) | Flag::stone(delivered));

        test.run_update(seconds(20.0), true);
        assert_eq!(test.stones.in_play(), Flag(0));
        test.dirty.assert_and_clear(Flag::stone(taken_out) | Flag::stone(delivered));
    }

    #[test]
    fn take_out_through_frozen_stone() {
        let sheet_params = sheet::Parameters {
            static_friction: feet_squared_per_second_squared(0.0),
            geometry: sheet::parameters::Geometry::default(),
            friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
            rotation_acc: feet_per_second_squared(245.0 / 8649.0),
            stone_radius: inches(18.0 / PI),
        };
        let tee = sheet_params.geometry.tee();
        let frozen = 0;
        let taken_out = 8;
        let delivered = 1;
        let stones = [
            (frozen, tee),
            (taken_out, tee + Vector2 { x: feet(0.0), y: sheet_params.stone_radius * 2.0 }),
        ];
        let mut test = DeliveryTest::set_up(
            radians(3.0 / 132.0),
            feet_per_second(7.7777),
            Rotation::Clockwise,
            delivered,
            stones,
        );
        test.sheet_params = sheet_params;

        test.run_update(seconds(100.0), true);
        assert_eq!(test.stones.in_play(), Flag::stone(frozen) | Flag::stone(delivered));
        assert_float_eq!(
            test.stones.positions()[frozen].y,
            test.sheet_params.geometry.playing_end.tee_line_y,
            abs <= 0.5
        );
    }

    #[test]
    fn take_out_guard_just_behind_hog_line() {
        let sheet_params = sheet::Parameters::default();
        let guard = 0;
        let delivered = 8;
        let stones = [(
            guard,
            Position {
                x: sheet_params.geometry.center_line_x,
                y: sheet_params.geometry.playing_end.hog_line_y
                    + sheet_params.stone_radius
                    + inches(0.1),
            },
        )];
        let mut test = DeliveryTest::set_up(
            radians(1.5 / 132.0),
            feet_per_second(7.7),
            Rotation::Clockwise,
            delivered,
            stones,
        );

        test.run_update(seconds(100.0), true);
        assert_eq!(test.stones.in_play(), Flag::stone(delivered));
        assert!(
            test.stones.positions()[delivered].y < sheet_params.geometry.playing_end.hog_line_y
        );
    }

    #[test]
    fn collision_case() {
        let first_stone = 8;
        let second_stone = 0;
        let delivered_stone = 9;
        let mut test = DeliveryTest::set_up(
            radians(-0.003968233139033363),
            feet_per_second(7.0),
            Rotation::Clockwise,
            delivered_stone,
            [
                (first_stone, Position { x: feet(-5.005604920482656), y: feet(131.7161589974921) }),
                (
                    second_stone,
                    Position { x: feet(-4.71237046298357), y: feet(130.80300085783333) },
                ),
            ],
        );
        test.run_update(seconds(100.0), true);
        let first_stone_result = test.stones.positions()[first_stone];
        let second_stone_result = test.stones.positions()[second_stone];
        let distance = (first_stone_result - second_stone_result).norm();
        let min_distance = test.sheet_params.stone_radius * 2.0;
        assert!(
            distance >= min_distance,
            "Distance {distance:?} is too small (should be at least {min_distance:?})"
        );
    }

    #[test]
    fn ideal_freeze() {
        let sheet_params =
            sheet::Parameters { stone_radius: feet(0.5), ..sheet::Parameters::default() };
        let existing_stone = 0;
        let existing_stone_pos = sheet_params.geometry.tee()
            + Position { x: sheet_params.geometry.hack_x_offset, y: feet(1.0) };
        let delivered_stone = 8;
        let delivered_stone_target = sheet_params.geometry.tee()
            + Position { x: sheet_params.geometry.hack_x_offset, y: Length::ZERO };
        let mut stones = [(existing_stone, existing_stone_pos)].into_iter().collect();
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet_params);
        let mut dirty = Flag(0);
        let start = StartingConditions {
            stone: delivered_stone,
            angle: Angle::ZERO,
            velocity: sheet_params.velocity_for_target_y(delivered_stone_target.y),
            hack: Hack::Left,
            rotation: Rotation::None,
        };
        let mut process = Process::new(start, &mut stones, &sheet_params, &mut dirty);
        dirty.assert_and_clear(Flag::stone(delivered_stone));

        let mut update = Update {
            process: &mut process,
            sheet: &mut stones,
            sheet_params: &sheet_params,
            simulation: &simulation,
            dirty: &mut dirty,
        };
        assert!(update.run(Some(seconds(100.0))));

        assert_float_eq!(stones.positions()[delivered_stone].x, delivered_stone_target.x);
        assert_float_eq!(stones.positions()[delivered_stone].y, delivered_stone_target.y);
        assert_float_eq!(stones.positions()[existing_stone].x, existing_stone_pos.x);
        assert_float_eq!(stones.positions()[existing_stone].y, existing_stone_pos.y);
    }

    #[test]
    fn infinite_loop_case() {
        // 2023-05-29T20:41:30.941Z INFO  [gielo::game::turn] Starting delivery: ResolvedStart { stone: 2, angle: 0.029739065044429545, velocity: 6.660988156007251 ft^1 s^-1, hack: Left, rotation: Clockwise, sheet: Sheet { stones: Stones { positions: [Vector2 { x: 2.1878551923754115 ft^1, y: 131.46574811443395 ft^1 }, Vector2 { x: -1.385046970563802 ft^1, y: 131.63285902920924 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: -0.4094141612577939 ft^1, y: 131.93239985326647 ft^1 }, Vector2 { x: 2.086156542675065 ft^1, y: 129.7895295071387 ft^1 }, Vector2 { x: -1.18616978195276 ft^1, y: 129.57204710483458 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }, Vector2 { x: 0.0 ft^1, y: 0.0 ft^1 }], in_play: Flag(0000011100000011) }, parameters: Parameters { geometry: Geometry { width: 15.583333333333334 ft^1, length: 150.0 ft^1, center_line_x: 0.0 ft^1, hack_x_offset: 0.4999999999999999 ft^1, house_radius: 6.0 ft^1, delivery_end: EndGeometry { hack_line_y: 6.0 ft^1, back_line_y: 12.0 ft^1, tee_line_y: 18.0 ft^1, hog_line_y: 39.0 ft^1 }, playing_end: EndGeometry { hack_line_y: 144.0 ft^1, back_line_y: 138.0 ft^1, tee_line_y: 132.0 ft^1, hog_line_y: 111.0 ft^1 } }, stone_radius: 0.47746482927568595 ft^1, friction: 0.24 ft^1 s^-2, rotation_acc: 0.025 ft^1 s^-2, static_friction: 0.25 ft^2 s^-2 } }, dirty: Dirty { finished_ends_count: 0, stones: Flag(0000000000000000), score: false, phase: false, preview: false } }
        let sheet_params = sheet::Parameters::default();
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet_params);
        let mut stones: Stones = [
            (0, Position { x: feet(2.1878551923754115), y: feet(131.46574811443395) }),
            (1, Position { x: feet(-1.385046970563802), y: feet(131.63285902920924) }),
            (8, Position { x: feet(-0.4094141612577939), y: feet(131.93239985326647) }),
            (9, Position { x: feet(2.086156542675065), y: feet(129.7895295071387) }),
            (10, Position { x: feet(-1.18616978195276), y: feet(129.57204710483458) }),
        ]
        .into_iter()
        .collect();
        let mut dirty = Flag(0);
        let resolved = StartingConditions {
            stone: 2,
            angle: radians(0.029739065044429545),
            velocity: feet_per_second(6.660988156007251),
            hack: Hack::Left,
            rotation: Rotation::Clockwise,
        };
        let mut process = Process::new(resolved, &mut stones, &sheet_params, &mut dirty);
        let mut update = Update {
            process: &mut process,
            sheet: &mut stones,
            sheet_params: &sheet_params,
            simulation: &simulation,
            dirty: &mut dirty,
        };

        assert!(update.run(Some(seconds(100.0))));
        // Should not enter infinite loop.
    }
}
