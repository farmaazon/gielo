use crate::game;
use crate::game::simulation::delivery::event::Event;
use crate::game::simulation::stone::MovingStone;
use crate::game::simulation::{stone, Simulation};
use crate::game::stone::Rotation;
use crate::game::turn::delivery::ResolvedStart;
use crate::game::{Dirty, Sheet};
use crate::unit::Time;
use crate::vector::Vector2;
use decorum::NotNan;
use derive_more::{Deref, DerefMut};
use std::cmp;
use uom::si::time::second;
use uom::ConstZero;

mod event {
    use crate::game::simulation::stone;
    use crate::unit::Time;

    #[derive(Copy, Clone, Debug)]
    pub enum Kind {
        Release,
        StoneOut,
        StoneStopped,
        Collision { with: stone::Id },
        NextTimeQuantum,
    }

    #[derive(Clone, Debug)]
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

#[derive(Debug)]
pub struct Process {
    pub stones: [MovingStone; game::stone::COUNT],
    pub delivered_stone: stone::Id,
    pub rotation: Rotation,
    pub current_time: Time,
    next_event_cached: Option<Event>,
}

impl Process {
    pub fn new(
        ResolvedStart { stone, angle, weight, hack, rotation, sheet, dirty }: ResolvedStart,
    ) -> Self {
        let delivered_initial_pos = sheet.parameters.geometry.hack_pos(hack);
        sheet.stones.put_stone(dirty, stone, delivered_initial_pos);
        let mut stones = sheet.stones.positions().map(MovingStone::new_stationary);
        let delivering_dist = sheet.parameters.geometry.delivery_dist();
        let measure_dist = sheet.parameters.geometry.measure_dist();
        let release_time = weight * delivering_dist / measure_dist;
        let v = measure_dist / weight;
        stones[stone].motion.v0 = Vector2 { x: v * angle.sin(), y: v * angle.cos() };
        Self {
            stones,
            delivered_stone: stone,
            rotation,
            current_time: Time::ZERO,
            next_event_cached: Some(Event {
                time: release_time,
                stone,
                kind: event::Kind::Release,
            }),
        }
    }
}

#[derive(Debug, Deref, DerefMut)]
pub struct Update<'a, 'b, 'c, 'd> {
    #[deref]
    #[deref_mut]
    pub process: &'a mut Process,
    pub sheet: &'b mut Sheet,
    pub simulation: &'d Simulation,
    pub dirty: &'c mut Dirty,
    pub until: Option<Time>,
}

impl<'a, 'b, 'c, 'd> Update<'a, 'b, 'c, 'd> {
    pub fn next_event(&mut self) -> Option<Event> {
        let not_yet_cached_event = self
            .process
            .next_event_cached
            .as_ref()
            .zip(self.until)
            .map_or(false, |(event, until)| event.time > until);
        if not_yet_cached_event {
            None
        } else if let Some(event) = self.process.next_event_cached.take() {
            Some(event)
        } else {
            let next_event = self.compute_next_event();
            let to_cache = next_event
                .as_ref()
                .zip(self.until)
                .map_or(false, |(event, until)| event.time > until);
            if to_cache {
                self.process.next_event_cached = next_event;
                None
            } else {
                next_event
            }
        }
    }

    fn compute_next_event(&self) -> Option<Event> {
        let key = |event: &Event| NotNan::from(event.time.get::<second>());
        let events =
            self.sheet.stones.in_play().iter_ids().flat_map(|stone| self.stone_events(stone));
        events.filter(|event| event.time >= self.process.current_time).min_by_key(key)
    }

    fn stone_events(&self, id: stone::Id) -> impl Iterator<Item = Event> + '_ {
        let next_event = stone::NextStoneEvent {
            stone: &self.process.stones[id],
            sheet_params: &self.sheet.parameters,
        };
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
            game::stone::Flag::range((id + 1)..game::stone::COUNT) & self.sheet.stones.in_play();
        let collisions = next_stones.iter_ids().flat_map(move |with| {
            let rhs = &self.process.stones[with];
            Event::from_times(event::Kind::Collision { with }, id, next_event.when_collision(rhs))
        });
        next_quantum.into_iter().chain(outside_x).chain(outside_y).chain(stopped).chain(collisions)
    }

    pub fn apply_event(&mut self, Event { time, stone: stone_id, kind }: Event) {
        self.process.current_time = time;
        let (stones_before, rest) = self.process.stones.split_at_mut(stone_id);
        let (stone, stones_after) = rest.split_first_mut().unwrap();
        let mut stone_update = stone::Update {
            stone,
            simulation: self.simulation,
            sheet_params: &self.sheet.parameters,
        };
        match kind {
            event::Kind::Release => stone_update.release(time, self.process.rotation),
            event::Kind::StoneOut => {
                stone_update.remove();
                self.sheet.stones.remove_stone(self.dirty, stone_id);
            }
            event::Kind::StoneStopped => {
                stone_update.stop(time);
                self.sheet.stones.set_position(self.dirty, stone_id, stone_update.stone.motion.s0);
            }
            event::Kind::Collision { with } => {
                let with_stone = match stone_id.cmp(&with) {
                    cmp::Ordering::Less => &mut stones_after[with - stone_id - 1],
                    cmp::Ordering::Greater => &mut stones_before[with],
                    cmp::Ordering::Equal => panic!("Stone collided with itself"),
                };
                stone_update.collision(with_stone, time);
            }
            event::Kind::NextTimeQuantum => stone_update.set_next_time_quantum(),
        }
    }

    /// Returns true when finished.
    pub fn run(&mut self) -> bool {
        while let Some(event) = self.next_event() {
            self.apply_event(event);
        }
        if let Some(current_time) = self.until {
            self.process.current_time = current_time;
        }
        let moving_stones =
            self.process.stones.iter().enumerate().filter(|(_, stone)| stone.is_moving());
        for (id, stone) in moving_stones {
            self.sheet.stones.set_position(
                self.dirty,
                id,
                stone.position(self.process.current_time),
            );
        }
        self.process.next_event_cached.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::game::sheet::Hack;
    use crate::game::stone::Flag;
    use crate::game::stone::{Position, Rotation};
    use crate::game::{sheet, simulation};
    use crate::unit::{
        assert_approx_eq, feet, feet_squared_per_second_squared, radians, seconds, Angle,
    };
    use crate::vector::Vector2;

    struct DeliveryTest {
        sheet: Sheet,
        simulation: Simulation,
        dirty: Dirty,
        process: Process,
    }

    impl DeliveryTest {
        fn set_up(
            angle: Angle,
            weight: Time,
            rotation: Rotation,
            delivered_stone: stone::Id,
            other_stones: impl IntoIterator<Item = (stone::Id, Position)>,
        ) -> Self {
            let mut sheet = Sheet::new(sheet::Parameters::default());
            sheet.stones = other_stones.into_iter().collect();
            let simulation = Simulation::new(simulation::Parameters::default(), &sheet.parameters);
            let mut dirty = Dirty::new();
            let start = ResolvedStart {
                stone: delivered_stone,
                angle,
                weight,
                hack: Hack::Left,
                rotation,
                sheet: &mut sheet,
                dirty: &mut dirty,
            };
            let process = Process::new(start);
            dirty.check_and_clear(&Dirty {
                stones: Flag::stone(delivered_stone),
                ..Dirty::default()
            });
            sheet.stones.in_play().contains(delivered_stone);

            Self { sheet, dirty, process, simulation }
        }

        fn run_update(&mut self, until: Option<Time>, expected_result: bool) {
            let mut update = Update {
                process: &mut self.process,
                sheet: &mut self.sheet,
                simulation: &self.simulation,
                dirty: &mut self.dirty,
                until,
            };
            assert_eq!(update.run(), expected_result);
        }
    }

    #[test]
    fn inaccurate_tee_shot() {
        let mut test =
            DeliveryTest::set_up(radians(6.0 / 132.0), seconds(3.0), Rotation::Clockwise, 0, []);
        test.run_update(Some(seconds(2.0)), false);
        test.dirty.check_and_clear(&Dirty { stones: Flag::stone(0), ..Dirty::default() });

        let mut check_moving = |time: Time, exp_pos: Position| {
            test.run_update(Some(time), false);
            let position = test.sheet.stones.positions()[0];
            assert_approx_eq!(position.x, exp_pos.x, epsilon = 2.0);
            assert_approx_eq!(position.y, exp_pos.y, epsilon = 2.0);
            assert_eq!(test.sheet.stones.in_play(), Flag::stone(0));
            test.dirty.check_and_clear(&Dirty { stones: Flag::stone(0), ..Dirty::default() });
        };

        check_moving(seconds(5.1), Vector2 { x: feet(2.0), y: feet(40.0) });
        check_moving(seconds(12.5), Vector2 { x: feet(3.5), y: feet(85.0) });
        check_moving(seconds(20.0), Vector2 { x: feet(3.0), y: feet(115.0) });
        check_moving(seconds(30.2), Vector2 { x: feet(1.0), y: feet(130.0) });

        test.run_update(None, true);
        let position = test.sheet.stones.positions()[0];
        assert_approx_eq!(position.x, feet(0.0), epsilon = 2.0);
        assert_approx_eq!(
            position.y,
            test.sheet.parameters.geometry.playing_end.tee_line_y,
            epsilon = 2.0
        );
        assert_eq!(test.sheet.stones.in_play(), Flag::stone(0));
        test.dirty.check_and_clear(&Dirty { stones: Flag::stone(0), ..Dirty::default() });
    }

    #[test]
    fn too_strong() {
        let mut test =
            DeliveryTest::set_up(radians(6.0 / 132.0), seconds(2.8), Rotation::Clockwise, 4, []);
        test.run_update(Some(seconds(60.0)), true);
        assert_eq!(test.sheet.stones.in_play(), Flag(0));
        test.dirty.check_and_clear(&Dirty { stones: Flag::stone(4), ..Dirty::default() });
    }

    #[test]
    fn clear_stone() {
        let tee = sheet::Geometry::default().tee();
        let delivered = game::stone::TEAM_IDS.a.start;
        let taken_out = game::stone::TEAM_IDS.b.start;
        let mut test = DeliveryTest::set_up(
            radians(-2.0 / 132.0),
            seconds(2.3),
            Rotation::CounterClockwise,
            delivered,
            [(taken_out, tee)],
        );

        test.run_update(Some(seconds(15.0)), false);
        assert!(!test.process.stones[taken_out].is_moving());
        assert_eq!(test.sheet.stones.positions()[taken_out], tee);
        assert!(test.process.stones[delivered].is_moving());
        test.dirty.check_and_clear(&Dirty { stones: Flag::stone(delivered), ..Dirty::default() });

        test.run_update(Some(seconds(16.0)), false);
        assert!(test.process.stones[taken_out].is_moving());
        assert!(test.process.stones[delivered].is_moving());
        test.dirty.check_and_clear(&Dirty {
            stones: Flag::stone(taken_out) | Flag::stone(delivered),
            ..Dirty::default()
        });

        test.run_update(Some(seconds(20.0)), true);
        assert_eq!(test.sheet.stones.in_play(), Flag(0));
        test.dirty.check_and_clear(&Dirty {
            stones: Flag::stone(taken_out) | Flag::stone(delivered),
            ..Dirty::default()
        });
    }

    #[test]
    fn take_out_through_frozen_stone() {
        let sheet_params = sheet::Parameters {
            static_friction: feet_squared_per_second_squared(0.0),
            ..sheet::Parameters::default()
        };
        let tee = sheet_params.geometry.tee();
        let frozen = game::stone::TEAM_IDS.a.start;
        let taken_out = game::stone::TEAM_IDS.b.start;
        let delivered = game::stone::TEAM_IDS.a.start + 1;
        let stones = [
            (frozen, tee),
            (taken_out, tee + Vector2 { x: feet(0.0), y: sheet_params.stone_radius * 2.0 }),
        ];
        let mut test = DeliveryTest::set_up(
            radians(3.0 / 132.0),
            seconds(2.7),
            Rotation::Clockwise,
            delivered,
            stones,
        );
        test.sheet.parameters = sheet_params;

        test.run_update(None, true);
        assert_eq!(test.sheet.stones.in_play(), Flag::stone(frozen) | Flag::stone(delivered));
        assert_approx_eq!(
            test.sheet.stones.positions()[frozen].y,
            test.sheet.parameters.geometry.playing_end.tee_line_y,
            epsilon = 0.5
        );
    }
}
