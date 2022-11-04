use crate::game::sheet::stone;
use crate::game::sheet::stone::Stone;
use crate::game::turn::delivery::process::event::Event;
use crate::game::{Dirty, Sheet};
use crate::unit::{milliseconds, Time};
use decorum::NotNan;
use derive_more::{Deref, DerefMut};
use lazy_static::lazy_static;
use uom::si::time::second;

lazy_static! {
    static ref TIME_QUANTUM_DURATION: Time = milliseconds(250.0);
}

mod event {
    use crate::game::sheet::stone;
    use crate::unit::Time;

    #[derive(Copy, Clone, Debug)]
    pub enum Kind {
        StoneOut(stone::Id),
        StoneNextStage(stone::Id),
        Collision(stone::Id, stone::Id),
        NextTimeQuantum,
    }

    #[derive(Clone, Debug)]
    pub struct Event {
        pub time: Time,
        pub kind: Kind,
    }

    impl Event {
        pub fn from_times(
            kind: Kind,
            times: impl IntoIterator<Item = Time>,
        ) -> impl Iterator<Item = Event> {
            times.into_iter().map(move |time| Self { kind, time })
        }
    }
}

#[derive(Debug)]
pub struct Process {
    pub current_time: Time,
    pub next_time_quantum: Time,
    next_event_cached: Option<Event>,
}

impl Process {
    pub fn new() -> Self {
        Self {
            current_time: Time::default(),
            next_time_quantum: *TIME_QUANTUM_DURATION,
            next_event_cached: None,
        }
    }
}

impl Default for Process {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deref, DerefMut)]
pub struct Update<'a, 'b, 'c> {
    #[deref]
    #[deref_mut]
    pub process: &'a mut Process,
    pub sheet: &'b mut Sheet,
    pub dirty: &'c mut Dirty,
    pub until: Option<Time>,
}

impl<'a, 'b, 'c> Update<'a, 'b, 'c> {
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
        let stone_events =
            self.sheet.stones.iter().enumerate().flat_map(|stone| self.stone_events(stone));
        let stone_pairs = self.sheet.stones.iter().enumerate().flat_map(|(id, stone)| {
            self.sheet.stones.iter().enumerate().take(id).map(move |rstone| ((id, stone), rstone))
        });
        let collisions = stone_pairs.flat_map(|((lid, lstone), (rid, rstone))| {
            Event::from_times(
                event::Kind::Collision(lid, rid),
                lstone.when_collision(rstone, &self.sheet.parameters),
            )
        });
        let all_events = stone_events.chain(collisions);
        let base_event =
            all_events.filter(|event| event.time >= self.process.current_time).min_by_key(key);
        // Consider next quantum only if there are other potential events. Otherwise we will run infinitely.
        base_event.and_then(|event| {
            let next_quantum =
                Event { kind: event::Kind::NextTimeQuantum, time: self.process.next_time_quantum };
            [next_quantum, event].into_iter().min_by_key(key)
        })
    }

    fn stone_events(&self, (id, stone): (stone::Id, &Stone)) -> impl Iterator<Item = Event> {
        let outside_x = Event::from_times(
            event::Kind::StoneOut(id),
            stone.when_outside_x(&self.sheet.parameters),
        );
        let outside_y = Event::from_times(
            event::Kind::StoneOut(id),
            stone.when_outside_y(&self.sheet.parameters),
        );
        let next_stage = Event::from_times(
            event::Kind::StoneNextStage(id),
            stone.when_next_stage(&self.sheet.parameters),
        );
        outside_x.chain(outside_y).chain(next_stage)
    }

    pub fn apply_event(&mut self, Event { time, kind }: Event) {
        self.process.current_time = time;
        match kind {
            event::Kind::StoneOut(stone) => {
                self.sheet.stones[stone].set_state(&mut self.dirty.stone(stone), stone::State::Out);
            }
            event::Kind::StoneNextStage(stone) => {
                self.sheet.stones[stone]
                    .next_stage(&mut self.dirty.stone(stone), &self.sheet.parameters);
            }
            event::Kind::Collision(lid, rid) => {
                let lstone = &self.sheet.stones[lid];
                let rstone = &self.sheet.stones[rid];
                if let Some((lstate, rstate)) =
                    lstone.states_after_collision(rstone, &self.sheet.parameters, time)
                {
                    self.sheet.stones[lid].set_state(&mut self.dirty.stone(lid), lstate);
                    self.sheet.stones[rid].set_state(&mut self.dirty.stone(rid), rstate);
                }
            }
            event::Kind::NextTimeQuantum => {
                for (index, stone) in self.sheet.stones.iter_mut().enumerate() {
                    stone.next_time_quantum(
                        &mut self.dirty.stone(index),
                        time,
                        &self.sheet.parameters,
                    );
                }
                self.process.next_time_quantum = time + *TIME_QUANTUM_DURATION;
            }
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
            self.sheet.stones.iter().enumerate().filter(|(_, stone)| stone.is_moving());
        for (id, _) in moving_stones {
            self.dirty.stone(id).set()
        }
        self.process.next_event_cached.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::game::sheet;
    use crate::game::sheet::stone::Rotation;
    use crate::game::sheet::Hack;
    use crate::game::team::Team;
    use crate::game::turn::delivery;
    use crate::unit::{
        approx_eq, assert_approx_eq, feet, feet_squared_per_second_squared, radians, seconds, Angle,
    };
    use crate::vector::Vector2;

    struct DeliveryTest {
        sheet: Sheet,
        dirty: Dirty,
        process: Process,
    }

    impl DeliveryTest {
        fn set_up(
            angle: Angle,
            weight: Time,
            rotation: Rotation,
            stones: impl IntoIterator<Item = Stone>,
        ) -> Self {
            let mut sheet = Sheet::new(sheet::Parameters::default());
            for stone in stones {
                sheet.stones.push(&mut Dirty::new(), stone);
            }
            let mut dirty = Dirty::new();
            let mut start = delivery::ResolvedStart {
                angle,
                weight,
                team: Team::A,
                hack: Hack::Left,
                rotation,
                sheet: &mut sheet,
                dirty: &mut dirty,
            };
            start.add_delivered_stone();
            start.dirty.check_and_clear(&Dirty { stone_count: 1, ..Dirty::default() });

            Self { sheet, dirty, process: Process::new() }
        }

        fn run_update(&mut self, until: Option<Time>, expected_result: bool) {
            let mut update = Update {
                process: &mut self.process,
                sheet: &mut self.sheet,
                dirty: &mut self.dirty,
                until,
            };
            assert_eq!(update.run(), expected_result);
        }
    }

    #[test]
    fn inaccurate_tee_shot() {
        let mut test =
            DeliveryTest::set_up(radians(6.0 / 132.0), seconds(3.0), Rotation::Clockwise, []);
        test.run_update(Some(seconds(2.0)), false);
        assert!(matches!(test.sheet.stones[0].state(), stone::State::BeingDelivered { .. }));
        test.dirty.check_and_clear(&Dirty { stones: 1, ..Dirty::default() });

        let mut check_moving_stage = |time: Time, exp_t0: Time, exp_pos: stone::Position| {
            test.run_update(Some(time), false);
            assert!(matches!(
                test.sheet.stones[0].state(),
                stone::State::Moving (stone::state::Moving {t0, ..}) if approx_eq!(t0, exp_t0)
            ));
            let position = test.sheet.stones[0].position(time).unwrap();
            assert_approx_eq!(position.x, exp_pos.x, epsilon = 2.0);
            assert_approx_eq!(position.y, exp_pos.y, epsilon = 2.0);
            test.dirty.check_and_clear(&Dirty { stones: 1, ..Dirty::default() });
        };

        check_moving_stage(seconds(5.1), seconds(5.0), Vector2 { x: feet(2.0), y: feet(40.0) });
        check_moving_stage(seconds(12.5), seconds(12.5), Vector2 { x: feet(3.5), y: feet(85.0) });
        check_moving_stage(seconds(20.0), seconds(20.0), Vector2 { x: feet(3.0), y: feet(115.0) });
        check_moving_stage(seconds(30.2), seconds(30.0), Vector2 { x: feet(1.0), y: feet(130.0) });

        test.run_update(None, true);
        assert!(matches!(test.sheet.stones[0].state(), stone::State::Stationary(_)));
        let position = test.sheet.stones[0].position(seconds(0.0)).unwrap();
        assert_approx_eq!(position.x, feet(0.0), epsilon = 2.0);
        assert_approx_eq!(
            position.y,
            test.sheet.parameters.geometry.playing_end.tee_line_y,
            epsilon = 2.0
        );
        test.dirty.check_and_clear(&Dirty { stones: 1, ..Dirty::default() });
    }

    #[test]
    fn too_strong() {
        let mut test =
            DeliveryTest::set_up(radians(6.0 / 132.0), seconds(2.8), Rotation::Clockwise, []);
        test.run_update(Some(seconds(60.0)), true);
        assert!(matches!(test.sheet.stones[0].state(), stone::State::Out { .. }));
        assert_eq!(test.sheet.stones[0].position(seconds(0.0)), None);
        test.dirty.check_and_clear(&Dirty { stones: 1, ..Dirty::default() });
    }

    #[test]
    fn clear_stone() {
        let tee = sheet::Geometry::default().tee();
        let mut test = DeliveryTest::set_up(
            radians(-2.0 / 132.0),
            seconds(2.3),
            Rotation::CounterClockwise,
            [Stone::new_stationary(Team::B, tee)],
        );

        test.run_update(Some(seconds(15.0)), false);
        assert!(
            matches!(test.sheet.stones[0].state(), stone::State::Stationary(position) if *position == tee)
        );
        assert!(matches!(test.sheet.stones[1].state(), stone::State::Moving(_)));
        test.dirty.check_and_clear(&Dirty { stone_count: 0, stones: 2, ..Dirty::default() });

        test.run_update(Some(seconds(16.0)), false);
        assert!(
            matches!(test.sheet.stones[0].state(), stone::State::Moving(state) if state.position(seconds(16.0)) != tee)
        );
        assert!(matches!(test.sheet.stones[1].state(), stone::State::Moving(_)));
        test.dirty.check_and_clear(&Dirty { stones: 3, ..Dirty::default() });

        test.run_update(Some(seconds(20.0)), true);
        assert!(matches!(test.sheet.stones[0].state(), stone::State::Out { .. }));
        assert!(matches!(test.sheet.stones[1].state(), stone::State::Out { .. }));
        test.dirty.check_and_clear(&Dirty { stones: 3, ..Dirty::default() });
    }

    #[test]
    fn take_out_through_freezed_stone() {
        let sheet_params = sheet::Parameters {
            static_friction: feet_squared_per_second_squared(0.0),
            ..sheet::Parameters::default()
        };
        let tee = sheet_params.geometry.tee();
        let stones = [
            Stone::new_stationary(Team::A, tee),
            Stone::new_stationary(
                Team::B,
                tee + Vector2 { x: feet(0.0), y: sheet_params.stone_radius * 2.0 },
            ),
        ];
        let mut test =
            DeliveryTest::set_up(radians(3.0 / 132.0), seconds(2.7), Rotation::Clockwise, stones);
        test.sheet.parameters = sheet_params;

        test.run_update(None, true);
        assert!(
            matches!(&test.sheet.stones[0].state(), stone::State::Stationary(position) if approx_eq!(position.y, test.sheet.parameters.geometry.playing_end.tee_line_y, epsilon = 0.5))
        );
        assert!(matches!(test.sheet.stones[1].state(), stone::State::Out { .. }));
        assert!(matches!(test.sheet.stones[2].state(), stone::State::Stationary { .. }));
    }
}
