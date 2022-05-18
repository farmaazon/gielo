use decorum::NotNan;
use lazy_static::lazy_static;
use std::time;
use uom::si::time::second;

use event::Event;

use crate::game::sheet::{Hack, Sheet};
use crate::game::stone::Rotation;
use crate::game::{sheet, stone, Stone, Team};
use crate::unit::{milliseconds, seconds, Angle, Time};
use crate::vector::Vector2;

lazy_static! {
    static ref TIME_QUANTUM_DURATION: Time = milliseconds(250.0);
}

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    pub angle: Angle,
    pub weight: Time,
    pub team: Team,
    pub hack: Hack,
    pub rotation: Rotation,
}

mod event {
    use crate::game::stone;
    use crate::unit::Time;

    #[derive(Copy, Clone, Debug)]
    pub enum Kind {
        StoneOut(stone::Id),
        StoneNextStage(stone::Id),
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
pub struct Delivery {
    pub current_time: Time,
    pub next_time_quantum: Time,
    next_event_cached: Option<Event>,
}

impl Delivery {
    pub fn new(
        sheet: &mut Sheet,
        Parameters { angle, weight, team, hack, rotation }: Parameters,
    ) -> Self {
        let delivering_dist = *sheet::delivery_end::HOG_LINE_Y - *sheet::delivery_end::HACK_LINE_Y;
        let measure_dist = *sheet::delivery_end::HOG_LINE_Y - *sheet::delivery_end::TEE_LINE_Y;
        let release_time = weight * delivering_dist / measure_dist;
        let delivering_off =
            Vector2 { x: delivering_dist * angle.sin(), y: delivering_dist * angle.cos() };
        let delivered_stone = Stone {
            team,
            state: stone::State::BeingDelivered(stone::state::BeingDelivered {
                release_time,
                starting_point: sheet::hack_pos(hack),
                delivering_off,
                rotation,
            }),
        };
        sheet.stones.push(delivered_stone);
        Self {
            current_time: Time::default(),
            next_time_quantum: *TIME_QUANTUM_DURATION,
            next_event_cached: None,
        }
    }

    pub fn next_event(&mut self, sheet: &Sheet, until: Option<Time>) -> Option<Event> {
        if let Some(event) = self.next_event_cached.take() {
            Some(event)
        } else {
            let next_event = self.compute_next_event(sheet);
            let to_cache =
                next_event.as_ref().zip(until).map_or(false, |(event, until)| event.time > until);
            if to_cache {
                self.next_event_cached = next_event;
                None
            } else {
                next_event
            }
        }
    }

    fn compute_next_event(&self, sheet: &Sheet) -> Option<Event> {
        let key = |event: &Event| NotNan::from(event.time.get::<second>());
        let stone_events = sheet
            .stones
            .iter()
            .enumerate()
            .flat_map(|stone| self.stone_events(&sheet.parameters, stone));
        let base_event =
            stone_events.filter(|event| event.time >= self.current_time).min_by_key(key);
        // Consider next quantum only if there are other potential events. Otherwise we will run infinitely.
        base_event.and_then(|event| {
            let next_quantum =
                Event { kind: event::Kind::NextTimeQuantum, time: self.next_time_quantum };
            [next_quantum, event].into_iter().min_by_key(key)
        })
    }

    fn stone_events(
        &self,
        sheet_params: &sheet::Parameters,
        (id, stone): (stone::Id, &Stone),
    ) -> impl Iterator<Item = Event> {
        let outside_x =
            Event::from_times(event::Kind::StoneOut(id), stone.when_outside_x(sheet_params));
        let outside_y =
            Event::from_times(event::Kind::StoneOut(id), stone.when_outside_y(sheet_params));
        let next_stage =
            Event::from_times(event::Kind::StoneNextStage(id), stone.when_next_stage(sheet_params));
        outside_x.chain(outside_y).chain(next_stage)
    }

    pub fn apply_event(&mut self, sheet: &mut Sheet, Event { time, kind }: Event) {
        self.current_time = time;
        match kind {
            event::Kind::StoneOut(stone) => {
                sheet.stones[stone].state = stone::State::Out;
            }
            event::Kind::StoneNextStage(stone) => {
                sheet.stones[stone].next_stage(&sheet.parameters);
            }
            event::Kind::NextTimeQuantum => {
                for stone in sheet.stones.iter_mut() {
                    stone.next_time_quantum(time, &sheet.parameters);
                }
                self.next_time_quantum = time + *TIME_QUANTUM_DURATION;
            }
        }
    }

    /// Returns true when finished.
    pub fn run(&mut self, sheet: &mut Sheet, until: Option<Time>) -> bool {
        while let Some(event) = self.next_event(sheet, until) {
            self.apply_event(sheet, event);
        }
        self.next_event_cached.is_none()
    }
}

#[derive(Debug)]
pub struct LiveDelivery {
    started_at: time::Instant,
    delivery: Delivery,
}

impl LiveDelivery {
    pub fn start(sheet: &mut Sheet, params: Parameters) -> Self {
        Self { started_at: time::Instant::now(), delivery: Delivery::new(sheet, params) }
    }

    pub fn update(&mut self, sheet: &mut Sheet, speed_factor: f32) -> bool {
        let real_time = time::Instant::now() - self.started_at;
        let game_time = seconds(real_time.as_secs_f32()) * speed_factor;
        self.delivery.run(sheet, Some(game_time))
    }
}

#[cfg(test)]
mod tests {
    use crate::unit::{approx_eq, assert_approx_eq, feet, radians, seconds};

    use super::*;

    #[test]
    fn inaccurate_tee_shot() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let params = Parameters {
            angle: radians(6.0 / 132.0),
            weight: seconds(3.0),
            team: Team::A,
            hack: Hack::Left,
            rotation: Rotation::Clockwise,
        };
        let mut delivery = Delivery::new(&mut sheet, params);
        assert!(!delivery.run(&mut sheet, Some(seconds(2.0))));
        assert!(matches!(sheet.stones[0].state, stone::State::BeingDelivered { .. }));

        let mut check_moving_stage = |time: Time, exp_t0: Time, exp_pos: stone::Position| {
            assert!(!delivery.run(&mut sheet, Some(time)));
            assert!(
                matches!(sheet.stones[0].state, stone::State::Moving (stone::state::Moving {t0, ..}) if approx_eq!(t0, exp_t0))
            );
            let position = sheet.stones[0].position(time).unwrap();
            assert_approx_eq!(position.x, exp_pos.x, epsilon = 2.0);
            assert_approx_eq!(position.y, exp_pos.y, epsilon = 2.0);
        };

        check_moving_stage(seconds(5.1), seconds(5.0), Vector2 { x: feet(2.0), y: feet(40.0) });
        check_moving_stage(seconds(12.5), seconds(12.5), Vector2 { x: feet(3.5), y: feet(85.0) });
        check_moving_stage(seconds(20.0), seconds(20.0), Vector2 { x: feet(3.0), y: feet(115.0) });
        check_moving_stage(seconds(30.2), seconds(30.0), Vector2 { x: feet(1.0), y: feet(130.0) });

        assert!(delivery.run(&mut sheet, None));
        assert!(matches!(sheet.stones[0].state, stone::State::Stationary(_)));
        let position = sheet.stones[0].position(seconds(0.0)).unwrap();
        assert_approx_eq!(position.x, feet(0.0), epsilon = 2.0);
        assert_approx_eq!(position.y, sheet::playing_end::TEE_LINE_Y, epsilon = 2.0);
    }

    #[test]
    fn too_strong() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let params = Parameters {
            angle: radians(6.0 / 132.0),
            weight: seconds(2.8),
            team: Team::A,
            hack: Hack::Left,
            rotation: Rotation::Clockwise,
        };
        let mut delivery = Delivery::new(&mut sheet, params);
        assert!(delivery.run(&mut sheet, Some(seconds(60.0))));
        assert!(matches!(sheet.stones[0].state, stone::State::Out));
        assert_eq!(sheet.stones[0].position(seconds(0.0)), None);
    }
}
