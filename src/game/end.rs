use crate::game::sheet::{stone, stones};
use crate::game::team::{Team, TEAMS_COUNT};
use crate::game::{turn, Dirty, Score, Sheet};
use anyhow::{bail, Result};
use std::time;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Phase {
    PlayingStones { current_turn: turn::Current },
    Finished { score: Score },
}

impl TryFrom<Phase> for turn::Current {
    type Error = anyhow::Error;
    fn try_from(phase: Phase) -> anyhow::Result<Self> {
        match phase {
            Phase::PlayingStones { current_turn } => Ok(current_turn),
            Phase::Finished { .. } => bail!("Getting current turn info from finished end"),
        }
    }
}

#[derive(Debug)]
pub struct Current {
    pub hammer: Team,
    pub finished_turns: Vec<turn::Finished>,
    pub phase: Phase,
}

impl Current {
    pub fn new(hammer: Team) -> Self {
        Self {
            hammer,
            finished_turns: vec![],
            phase: Phase::PlayingStones { current_turn: turn::Current::new(hammer.opponent()) },
        }
    }

    pub fn delivered_stone(&self) -> Option<stone::Id> {
        match self.phase {
            Phase::PlayingStones {
                current_turn: turn::Current { phase: turn::Phase::Delivering { .. }, .. },
            } => Some(self.finished_turns.len()),
            _ => None,
        }
    }

    pub fn current_turn(&self) -> Option<&turn::Current> {
        match &self.phase {
            Phase::PlayingStones { current_turn } => Some(current_turn),
            _ => None,
        }
    }

    pub fn stones_left(&self, team: Team) -> usize {
        let stone_id = self.finished_turns.len();
        let hammer_team_stones_delivered = stone_id / TEAMS_COUNT;
        let stones_delivered = if team == self.hammer || stone_id % 2 == 0 {
            hammer_team_stones_delivered
        } else {
            hammer_team_stones_delivered + 1
        };
        stones::PER_TEAM - stones_delivered
    }

    pub fn update(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        now: time::Instant,
        speed_factor: f32,
    ) {
        let finished = match &mut self.phase {
            Phase::PlayingStones { current_turn } => {
                current_turn.update(dirty, sheet, now, speed_factor);
                current_turn.is_finished()
            }
            _ => false,
        };
        if finished {
            self.proceed(dirty, sheet)
                .expect("If update resulted in finished state we should be able to proceed");
        }
    }

    pub fn proceed(&mut self, dirty: &mut Dirty, sheet: &Sheet) -> Result<()> {
        let new_phase = match &self.phase {
            Phase::PlayingStones { current_turn } if current_turn.is_finished() => {
                let turn_index = self.finished_turns.len();
                Self::phase_after_turn(sheet, current_turn, turn_index)
            }
            _ => bail!("Proceeding at wrong end phase"),
        };
        let previous_phase = std::mem::replace(&mut self.phase, new_phase);
        let turn: turn::Current = previous_phase.try_into().unwrap();
        self.finished_turns.push(turn.try_into().unwrap());
        dirty.phase = true;
        Ok(())
    }

    fn phase_after_turn(sheet: &Sheet, turn: &turn::Current, turn_index: usize) -> Phase {
        if turn_index + 1 >= stones::COUNT {
            Phase::Finished { score: sheet.count_score() }
        } else {
            Phase::PlayingStones { current_turn: turn::Current::new(turn.playing_team.opponent()) }
        }
    }

    pub fn start_delivery(
        &mut self,
        delivery: turn::delivery::Start,
        now: time::Instant,
    ) -> Result<()> {
        match &mut self.phase {
            Phase::PlayingStones { current_turn } => current_turn.start_delivery(delivery, now),
            _ => bail!("Starting delivery on wrong end phase"),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_turns_finished(hammer: Team, finished_turns: usize) -> Self {
        let next_stone_played_by = if finished_turns % 2 == 1 { hammer } else { hammer.opponent() };
        Self {
            hammer,
            finished_turns: make_finished_turns(hammer, finished_turns),
            phase: Phase::PlayingStones { current_turn: turn::Current::new(next_stone_played_by) },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub hammer: Team,
    pub score: Score,
    pub turns: Vec<turn::Finished>,
}

impl TryFrom<Current> for Finished {
    type Error = anyhow::Error;

    fn try_from(current: Current) -> anyhow::Result<Self> {
        match current.phase {
            Phase::Finished { score } => {
                Ok(Self { hammer: current.hammer, turns: current.finished_turns, score })
            }
            _ => bail!("Trying to get Finished End from unfinished Current End"),
        }
    }
}

#[cfg(test)]
impl Finished {
    pub(crate) fn new_with_all_stones_out(hammer: Team) -> Self {
        Finished {
            hammer,
            score: Score { a: 0, b: 0 },
            turns: make_finished_turns(hammer, stones::COUNT),
        }
    }
}

#[cfg(test)]
fn make_finished_turns(hammer: Team, count: usize) -> Vec<turn::Finished> {
    use crate::game::sheet::stone::Stone;
    use crate::game::sheet::stones::Stones;
    let mut stones = Stones::new();
    [hammer.opponent(), hammer]
        .into_iter()
        .cycle()
        .take(count)
        .map(|team| {
            stones.push(&mut Dirty::new(), Stone::new(team, stone::State::Out));
            turn::Finished { playing_team: team, snapshot: stones.clone() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet;
    use crate::game::team::teams;
    use crate::game::turn::delivery;
    use std::time::Duration;

    impl Current {}

    #[test]
    fn stones_left() {
        for hammer in teams() {
            let first_team = hammer.opponent();
            let mut end = Current::new(hammer);
            for expected_stones_left in (1..=stones::PER_TEAM).rev() {
                assert_eq!(end.stones_left(Team::A), expected_stones_left);
                assert_eq!(end.stones_left(Team::B), expected_stones_left);
                end.finished_turns.push(turn::Finished {
                    playing_team: first_team,
                    snapshot: Default::default(),
                });
                assert_eq!(end.stones_left(first_team), expected_stones_left - 1);
                assert_eq!(end.stones_left(hammer), expected_stones_left);
                end.finished_turns
                    .push(turn::Finished { playing_team: hammer, snapshot: Default::default() });
            }
        }
    }

    #[test]
    fn play_two_turns() {
        let mut dirty = Dirty::new();
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let mut end = Current::new(Team::A);

        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Thinking, playing_team: Team::B })
        ));
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 0);

        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        let delivery_start = time::Instant::now();
        end.start_delivery(delivery, delivery_start).expect("Error while starting delivery");
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Delivering { .. }, playing_team: Team::B })
        ));
        assert_eq!(end.delivered_stone(), Some(0));
        assert_eq!(end.finished_turns.len(), 0);

        let still_delivering = delivery_start + time::Duration::from_secs(2);
        end.update(&mut dirty, &mut sheet, still_delivering, 5.0);
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Delivering { .. }, playing_team: Team::B })
        ));
        assert_eq!(end.delivered_stone(), Some(0));
        assert_eq!(end.finished_turns.len(), 0);

        let finished_delivering = delivery_start + time::Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, finished_delivering, 5.0);
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Thinking, playing_team: Team::A })
        ));
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 1);
        assert_eq!(end.finished_turns[0].playing_team, Team::B);
        assert_eq!(end.finished_turns[0].snapshot, sheet.stones);

        let second_delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        let second_delivery_start = finished_delivering + time::Duration::from_secs(40);
        end.start_delivery(second_delivery, second_delivery_start)
            .expect("Error while starting second delivery");
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Delivering { .. }, playing_team: Team::A })
        ));
        assert_eq!(end.delivered_stone(), Some(1));
        assert_eq!(end.finished_turns.len(), 1);

        let finished_delivering = second_delivery_start + time::Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, finished_delivering, 5.0);
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(
            end.current_turn(),
            Some(turn::Current { phase: turn::Phase::Thinking, playing_team: Team::B })
        ));
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 2);
        assert_eq!(end.finished_turns[1].playing_team, Team::A);
        assert_eq!(end.finished_turns[1].snapshot, sheet.stones);
    }

    #[test]
    fn finishing_end() {
        let hammer = Team::A;
        let mut end = Current::new_with_turns_finished(hammer, stones::COUNT - 1);
        let mut sheet = Sheet {
            stones: end.finished_turns.last().unwrap().snapshot.clone(),
            parameters: sheet::Parameters::default(),
        };
        let mut dirty = Dirty::new();
        let start_time = time::Instant::now();
        end.start_delivery(delivery::Start::tee_draw(&mut dirty, &mut sheet), start_time).unwrap();
        let end_time = start_time + Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, end_time, 5.0);
        assert!(matches!(&end.phase, Phase::Finished { score: Score { a: 1, b: 0 } }));
        assert!(matches!(end.current_turn(), None));
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), stones::COUNT);
    }

    #[test]
    fn updating_and_starting_in_wrong_stage() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let mut dirty = Dirty::new();
        let mut end = Current::new(Team::B);

        end.update(&mut dirty, &mut sheet, time::Instant::now(), 5.0);
        dirty.check_and_clear(&Dirty::new());

        end.phase = Phase::Finished { score: Default::default() };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet);
        assert!(end.start_delivery(delivery, time::Instant::now()).is_err());
        dirty.check_and_clear(&Dirty::new());
        end.update(&mut dirty, &mut sheet, time::Instant::now(), 5.0);
        dirty.check_and_clear(&Dirty::new());
    }

    #[test]
    fn finished_turn_conversion() {
        let unfinished = Current::new(Team::A);
        let finished = Current {
            phase: Phase::Finished { score: Score { a: 2, b: 0 } },
            ..Current::new_with_turns_finished(Team::A, stones::COUNT)
        };
        assert!(TryInto::<Finished>::try_into(unfinished).is_err());
        let converted: Finished =
            finished.try_into().expect("Finished turn should convert successfully");
        assert_eq!(converted.hammer, Team::A);
        assert_eq!(converted.turns.len(), stones::COUNT);
        assert_eq!(converted.score, Score { a: 2, b: 0 });
    }
}
