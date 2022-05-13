use crate::game::sheet::{Hack, Sheet};
use crate::game::stone::Curl;
use crate::game::{sheet, stone, Stone, Team};
use crate::unit::{Angle, Time};
use crate::vector::Vector2;
use decorum::NotNan;
use lazy_static::lazy_static;
use std::iter::once;
use uom::si::time::{millisecond, second};

lazy_static! {
    static ref TIME_QUANTUM_DURATION: Time = Time::new::<millisecond>(200.0);
}

pub struct Params {
    pub angle: Angle,
    pub weight: Time,
    pub team: Team,
    pub hack: Hack,
    pub curl: Curl,
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

        pub fn in_time_bounds(&self, current: Time, until: Option<Time>) -> bool {
            if let Some(until) = until {
                (current..=until).contains(&self.time)
            } else {
                (current..).contains(&self.time)
            }
        }
    }
}
use event::Event;

pub struct Delivery {
    pub current_time: Time,
    pub next_time_quantum: Time,
}

impl Delivery {
    pub fn start(
        sheet: &mut Sheet,
        Params {
            angle,
            weight,
            team,
            hack,
            curl,
        }: Params,
    ) -> Self {
        let delivering_dist = *sheet::delivery_end::HOG_LINE_Y - *sheet::delivery_end::HACK_LINE_Y;
        let measure_dist = *sheet::delivery_end::HOG_LINE_Y - *sheet::delivery_end::TEE_LINE_Y;
        let release_time = weight * delivering_dist / measure_dist;
        let delivering_off = Vector2 {
            x: delivering_dist * angle.cos(),
            y: delivering_dist * angle.sin(),
        };
        let delivered_stone = Stone {
            team,
            state: stone::State::BeingDelivered(stone::state::BeingDelivered {
                release_time,
                starting_point: sheet.hack_pos(hack),
                delivering_off,
                curl,
            }),
        };
        sheet.stones.push(delivered_stone);
        Self {
            current_time: Time::default(),
            next_time_quantum: *TIME_QUANTUM_DURATION,
        }
    }

    pub fn next_event(&self, sheet: &Sheet, until: Option<Time>) -> Option<Event> {
        let next_quantum = once(Event {
            kind: event::Kind::NextTimeQuantum,
            time: self.next_time_quantum,
        });
        let stone_events = sheet
            .stones
            .iter()
            .enumerate()
            .flat_map(|stone| self.stone_events(sheet, stone));
        let all_events = next_quantum.chain(stone_events);
        all_events
            .filter(|event| event.in_time_bounds(self.current_time, until))
            .min_by_key(|event| NotNan::from(event.time.get::<second>()))
    }

    fn stone_events(
        &self,
        sheet: &Sheet,
        (id, stone): (stone::Id, &Stone),
    ) -> impl Iterator<Item = Event> {
        let outside_x = Event::from_times(event::Kind::StoneOut(id), stone.when_outside_x(sheet));
        let outside_y = Event::from_times(event::Kind::StoneOut(id), stone.when_outside_y(sheet));
        let next_stage = Event::from_times(
            event::Kind::StoneNextStage(id),
            stone.when_next_stage(&sheet.parameters),
        );
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

    pub fn run(&mut self, sheet: &mut Sheet, until: Option<Time>) {
        while let Some(event) = self.next_event(sheet, until) {
            self.apply_event(sheet, event)
        }
    }
}
