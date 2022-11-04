use crate::game::sheet::stones::Stones;
use crate::game::team::Team;
use crate::game::{Dirty, Sheet};
use crate::unit::{seconds, Time};
use anyhow::{bail, Result};
use std::time;

pub mod delivery;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Phase {
    Thinking,
    Delivering { started_at: time::Instant, process: delivery::Process },
    Finished { snapshot: Stones, delivery_time: Time },
}

#[derive(Debug)]
pub struct Current {
    pub playing_team: Team,
    pub phase: Phase,
}

impl Current {
    pub fn new(playing_team: Team) -> Self {
        Self { playing_team, phase: Phase::Thinking }
    }

    pub fn delivery_time(&self) -> Time {
        match &self.phase {
            Phase::Thinking => Time::default(),
            Phase::Delivering { process, .. } => process.current_time,
            Phase::Finished { delivery_time, .. } => *delivery_time,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished { .. })
    }

    pub fn update(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        now: time::Instant,
        speed_factor: f32,
    ) {
        let (finished, delivery_time) = match &mut self.phase {
            Phase::Delivering { started_at, process } => {
                let real_time = seconds((now - *started_at).as_secs_f32());
                let game_time = real_time * speed_factor;
                let until = Some(game_time);
                let finished = delivery::process::Update { dirty, sheet, process, until }.run();
                (finished, process.current_time)
            }
            _ => (false, Time::default()),
        };
        if finished {
            self.phase = Phase::Finished { snapshot: sheet.stones.clone(), delivery_time };
            dirty.phase = true;
        }
    }

    pub fn start_delivery(&mut self, delivery: delivery::Start, now: time::Instant) -> Result<()> {
        let new_phase = match &mut self.phase {
            Phase::Thinking => {
                let mut resolved = delivery.resolve(self.playing_team);
                resolved.add_delivered_stone();
                resolved.dirty.phase = true;
                Phase::Delivering { started_at: now, process: delivery::Process::new() }
            }
            _ => bail!("Starting delivery not on thinking phase"),
        };
        self.phase = new_phase;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub playing_team: Team,
    pub snapshot: Stones,
}

impl TryFrom<Current> for Finished {
    type Error = anyhow::Error;

    fn try_from(current: Current) -> anyhow::Result<Self> {
        match current.phase {
            Phase::Finished { snapshot, .. } => {
                Ok(Self { playing_team: current.playing_team, snapshot })
            }
            _ => bail!("Trying to get Finished Turn from unfinished Current Turn"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet;
    use crate::game::sheet::stone::Stone;
    use crate::unit::feet;
    use crate::vector::Vector2;

    #[test]
    fn progressing_turn() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let mut dirty = Dirty::new();
        let mut turn = Current::new(Team::B);
        assert!(matches!(&turn.phase, Phase::Thinking));
        assert!(!turn.is_finished());

        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        let start_time = time::Instant::now();
        turn.start_delivery(delivery, start_time).expect("Starting delivery failed");
        assert!(
            matches!(&turn.phase, Phase::Delivering {started_at, ..} if started_at == &start_time)
        );
        assert!(!turn.is_finished());
        assert_eq!(turn.delivery_time(), seconds(0.0));
        assert_eq!(sheet.stones.len(), 1);
        assert!(sheet.stones[0].is_moving());
        dirty.check_and_clear(&Dirty { stone_count: 1, phase: true, ..Dirty::default() });

        let first_update = start_time + time::Duration::from_secs(2);
        turn.update(&mut dirty, &mut sheet, first_update, 5.0);
        assert!(
            matches!(&turn.phase, Phase::Delivering {started_at, ..} if started_at == &start_time)
        );
        assert!(!turn.is_finished());
        assert_eq!(turn.delivery_time(), seconds(10.0));
        assert!(sheet.stones[0].is_moving());
        dirty.check_and_clear(&Dirty { stones: 1, ..Dirty::default() });

        let finishing_update = start_time + time::Duration::from_secs(7);
        turn.update(&mut dirty, &mut sheet, finishing_update, 5.0);
        let Phase::Finished {snapshot, delivery_time} = &turn.phase else {
            panic!("Wrong stage")
        };
        assert!(turn.is_finished());
        assert_eq!(snapshot, &sheet.stones);
        assert_eq!(turn.delivery_time(), *delivery_time);
        assert!(!sheet.stones[0].is_moving());
        dirty.check_and_clear(&Dirty { stones: 1, phase: true, ..Dirty::new() });
    }

    #[test]
    fn updating_and_starting_in_wrong_stage() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let mut dirty = Dirty::new();
        let mut turn = Current::new(Team::B);

        turn.update(&mut dirty, &mut sheet, time::Instant::now(), 5.0);
        dirty.check_and_clear(&Dirty::new());

        turn.phase = Phase::Delivering {
            started_at: time::Instant::now(),
            process: delivery::Process::new(),
        };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        assert!(turn.start_delivery(delivery, time::Instant::now()).is_err());
        dirty.check_and_clear(&Dirty::new());

        turn.phase =
            Phase::Finished { snapshot: Stones::default(), delivery_time: Time::default() };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        assert!(turn.start_delivery(delivery, time::Instant::now()).is_err());
        dirty.check_and_clear(&Dirty::new());
        turn.update(&mut dirty, &mut sheet, time::Instant::now(), 5.0);
        dirty.check_and_clear(&Dirty::new());
    }

    #[test]
    fn finished_turn_conversion() {
        let stones = Stones::from_iter(Some(Stone::new_stationary(
            Team::B,
            Vector2 { x: feet(1.0), y: feet(133.0) },
        )));
        let unfinished = Current { playing_team: Team::B, phase: Phase::Thinking };
        let finished = Current {
            playing_team: Team::A,
            phase: Phase::Finished { delivery_time: seconds(20.0), snapshot: stones.clone() },
        };
        assert!(TryInto::<Finished>::try_into(unfinished).is_err());
        let converted: Finished =
            finished.try_into().expect("Finished turn should convert successfully");
        assert_eq!(converted.playing_team, Team::A);
        assert_eq!(converted.snapshot, stones);
    }
}
