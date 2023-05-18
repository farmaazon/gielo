use crate::game::simulation::Simulation;
use crate::game::stone;
use crate::game::team::{Team, TEAMS_COUNT};
use crate::game::{turn, Dirty, Parameters, Score, Sheet};
use anyhow::{anyhow, bail, Result};
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
            phase: Phase::PlayingStones { current_turn: turn::Current::new_first(hammer) },
        }
    }

    pub fn delivered_stone(&self) -> Option<stone::Id> {
        match self.phase {
            Phase::PlayingStones {
                current_turn:
                    turn::Current { phase: turn::Phase::Delivering { .. }, played_stone, .. },
            } => Some(played_stone),
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
        stone::COUNT_PER_TEAM - stones_delivered
    }

    pub fn update(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        simulation: &Simulation,
        parameters: &Parameters,
        now: time::Instant,
    ) {
        let next_turn = match &mut self.phase {
            Phase::PlayingStones { current_turn } => {
                current_turn.update(dirty, sheet, simulation, now, parameters.speed_factor);
                matches!(&current_turn.phase, turn::Phase::Finished { .. })
            }
            _ => false,
        };
        if next_turn {
            self.start_next_turn(dirty, sheet, parameters)
                .expect("If update resulted in finished state we should be able to proceed");
        }
    }

    pub fn replace_stones(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        parameters: &Parameters,
    ) -> Result<()> {
        if let Phase::PlayingStones { current_turn } = &mut self.phase {
            current_turn.replace_stones(dirty, sheet)?
        }
        self.start_next_turn(dirty, sheet, parameters)
    }

    pub fn proceed(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        parameters: &Parameters,
    ) -> Result<()> {
        if let Phase::PlayingStones { current_turn } = &mut self.phase {
            current_turn.proceed(dirty, sheet)?
        }
        self.start_next_turn(dirty, sheet, parameters)
    }

    pub fn start_next_turn(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        parameters: &Parameters,
    ) -> Result<()> {
        let turn_index = self.finished_turns.len();
        let new_phase = if turn_index + 1 >= stone::COUNT {
            Phase::Finished { score: sheet.count_score() }
        } else {
            dirty.preview = true;
            Phase::PlayingStones {
                current_turn: turn::Current::new_by_index(
                    turn_index + 1,
                    self.hammer,
                    sheet,
                    parameters,
                ),
            }
        };
        let previous_phase = std::mem::replace(&mut self.phase, new_phase);
        let turn: turn::Current =
            previous_phase.try_into().map_err(|_| anyhow!("Starting next turn at wrong phase"))?;
        let finished_turn: turn::Finished = turn
            .try_into()
            .map_err(|_| anyhow!("Starting next turn before previous one is finished"))?;
        self.finished_turns.push(finished_turn);
        dirty.phase = true;
        Ok(())
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
    pub(crate) fn new_with_turns_finished(
        sheet: &Sheet,
        hammer: Team,
        finished_turns: usize,
    ) -> Self {
        Self {
            hammer,
            finished_turns: make_finished_turns(hammer, finished_turns),
            phase: if finished_turns < stone::COUNT {
                Phase::PlayingStones {
                    current_turn: turn::Current::new_by_index(
                        finished_turns,
                        hammer,
                        sheet,
                        &Parameters::default(),
                    ),
                }
            } else {
                Phase::Finished { score: Score { a: 0, b: 0 } }
            },
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
            turns: make_finished_turns(hammer, stone::COUNT),
        }
    }
}

#[cfg(test)]
fn make_finished_turns(hammer: Team, count: usize) -> Vec<turn::Finished> {
    use crate::game::stone::Stones;
    use crate::game::team::player;

    stone::QUEUE_BY_HAMMER[hammer]
        .iter()
        .take(count)
        .enumerate()
        .map(|(index, &played_stone)| turn::Finished {
            played_stone,
            delivering_player: player::who_is_delivering(index),
            snapshot: Stones::default(),
            violation: turn::ResolvedViolation::None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game;
    use crate::game::sheet;
    use crate::game::stone::Flag;
    use crate::game::team::teams;
    use crate::game::tests::PhaseTestSetup;
    use crate::game::turn::delivery;
    use crate::unit::{feet, seconds};
    use std::time::Duration;

    #[test]
    fn stones_left() {
        for hammer in teams() {
            let first_team = hammer.opponent();
            let mut end = Current::new(hammer);
            let mut stones = stone::QUEUE_BY_HAMMER[hammer].into_iter();
            for expected_stones_left in (1..=stone::COUNT_PER_TEAM).rev() {
                assert_eq!(end.stones_left(Team::A), expected_stones_left);
                assert_eq!(end.stones_left(Team::B), expected_stones_left);
                end.finished_turns.push(turn::Finished {
                    played_stone: stones.next().unwrap(),
                    delivering_player: 0,
                    violation: turn::ResolvedViolation::None,
                    snapshot: Default::default(),
                });
                assert_eq!(end.stones_left(first_team), expected_stones_left - 1);
                assert_eq!(end.stones_left(hammer), expected_stones_left);
                end.finished_turns.push(turn::Finished {
                    played_stone: stones.next().unwrap(),
                    delivering_player: 0,
                    violation: turn::ResolvedViolation::None,
                    snapshot: Default::default(),
                });
            }
        }
    }

    #[test]
    fn play_two_turns() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let parameters = game::Parameters {
            speed_factor: 5.0,
            ends: 8,
            rules: game::Rules { free_guard_rule_stones: 0, no_tick_rule_stones: 0 },
        };
        let mut end = Current::new(Team::A);

        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        let Some(current_turn) = end.current_turn() else {
            panic!("No current turn")
        };
        assert!(matches!(current_turn.phase, turn::Phase::Thinking));
        assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.a[0]);
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 0);

        let check_still_delivering = |end: &Current| {
            assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
            let Some(current_turn) = end.current_turn() else {
                panic!("No current turn")
            };
            assert!(matches!(current_turn.phase, turn::Phase::Delivering { .. }));
            assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.a[0]);
            assert_eq!(end.delivered_stone(), Some(stone::QUEUE_BY_HAMMER.a[0]));
            assert_eq!(end.finished_turns.len(), 0);
        };

        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        end.start_delivery(delivery, time).expect("Error while starting delivery");
        check_still_delivering(&end);

        let still_delivering = time + time::Duration::from_secs(2);
        end.update(&mut dirty, &mut sheet, &simulation, &parameters, still_delivering);
        check_still_delivering(&end);

        let finished_delivering = time + time::Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, &simulation, &parameters, finished_delivering);
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        let Some(current_turn) = end.current_turn() else {
            panic!("No current turn")
        };
        assert!(matches!(current_turn.phase, turn::Phase::Thinking));
        assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.a[1]);
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 1);
        assert_eq!(end.finished_turns[0].played_stone, stone::QUEUE_BY_HAMMER.a[0]);
        assert_eq!(end.finished_turns[0].snapshot, sheet.stones);

        let second_delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        let second_delivery_start = finished_delivering + time::Duration::from_secs(40);
        end.start_delivery(second_delivery, second_delivery_start)
            .expect("Error while starting second delivery");
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        let Some(current_turn) = end.current_turn() else {
            panic!("No current turn")
        };
        assert!(matches!(current_turn.phase, turn::Phase::Delivering { .. }));
        assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.a[1]);
        assert_eq!(end.delivered_stone(), Some(stone::QUEUE_BY_HAMMER.a[1]));
        assert_eq!(end.finished_turns.len(), 1);

        let finished_delivering = second_delivery_start + time::Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, &simulation, &parameters, finished_delivering);
        assert!(matches!(&end.phase, Phase::PlayingStones { .. }));
        let Some(current_turn) = end.current_turn() else {
            panic!("No current turn")
        };
        assert!(matches!(current_turn.phase, turn::Phase::Thinking));
        assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.a[2]);
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), 2);
        assert_eq!(end.finished_turns[1].played_stone, stone::QUEUE_BY_HAMMER.a[1]);
        assert_eq!(end.finished_turns[1].snapshot, sheet.stones);
    }

    #[test]
    fn restoring_after_violation() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let parameters = game::Parameters {
            speed_factor: 20.0,
            ends: 8,
            rules: game::Rules { free_guard_rule_stones: 5, no_tick_rule_stones: 5 },
        };
        let guard = stone::QUEUE_BY_HAMMER.b[0];
        let delivered = stone::QUEUE_BY_HAMMER.b[1];
        let guard_position = sheet.parameters.geometry.tee()
            - stone::Position {
                x: feet(0.0),
                y: sheet.parameters.geometry.house_radius + feet(4.0),
            };
        sheet.stones.put_stone(&mut Dirty::new(), guard, guard_position);
        let mut end = Current::new_with_turns_finished(&sheet, Team::B, 1);
        end.finished_turns[0].snapshot = sheet.stones.clone();
        let Phase::PlayingStones {current_turn: turn::Current {free_guards, free_center_guards, ..}} = &mut end.phase else {
            panic!("Wrong phase");
        };
        *free_guards = Flag::stone(guard);
        *free_center_guards = Flag::stone(guard);

        let take_out_guard = delivery::Call {
            weight: seconds(2.5),
            mark: guard_position,
            rotation: stone::Rotation::None,
        };
        let delivery = delivery::Start {
            call: take_out_guard,
            dirty: &mut dirty,
            sheet: &mut sheet,
            teams: &teams,
        };
        end.start_delivery(delivery, time).expect("Error while starting delivery");
        end.update(
            &mut dirty,
            &mut sheet,
            &simulation,
            &parameters,
            time + time::Duration::from_secs(10),
        );

        assert_eq!(end.finished_turns.len(), 1);
        assert!(!sheet.stones.in_play().contains(guard));
        let Phase::PlayingStones {current_turn: turn::Current {phase: turn::Phase::Violation { violation, .. }, ..}} = &end.phase else {
            panic!("Expected Rule Violation");
        };
        assert_eq!(*violation, turn::Violation::FreeGuardRule);
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::from_iter([guard, delivered]),
            phase: true,
            ..Dirty::default()
        });

        end.proceed(&mut dirty, &mut sheet, &parameters).expect("Proceeding failed");
        assert_eq!(end.finished_turns.len(), 2);
        assert!(sheet.stones.in_play().contains(guard));
        let Phase::PlayingStones {current_turn: turn::Current { free_guards, free_center_guards, phase: turn::Phase::Thinking, .. }} = &end.phase else {
            panic!("Expected Next Turn");
        };
        assert_eq!(*free_guards, Flag::stone(guard));
        assert_eq!(*free_center_guards, Flag::stone(guard));
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::from_iter([guard, delivered]),
            phase: true,
            preview: true,
            ..Dirty::default()
        });
    }

    #[test]
    fn finishing_end() {
        let PhaseTestSetup { mut parameters, mut sheet, simulation, teams, mut dirty, time } =
            PhaseTestSetup::new();
        let hammer = Team::A;
        let mut end = Current::new_with_turns_finished(&sheet, hammer, stone::COUNT - 1);
        sheet.stones = end.finished_turns.last().unwrap().snapshot.clone();
        parameters.speed_factor = 5.0;
        end.start_delivery(delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams), time)
            .unwrap();
        let end_time = time + Duration::from_secs(7);
        end.update(&mut dirty, &mut sheet, &simulation, &parameters, end_time);
        assert!(matches!(&end.phase, Phase::Finished { score: Score { a: 1, b: 0 } }));
        assert!(matches!(end.current_turn(), None));
        assert_eq!(end.delivered_stone(), None);
        assert_eq!(end.finished_turns.len(), stone::COUNT);
    }

    #[test]
    fn updating_and_starting_in_wrong_stage() {
        let PhaseTestSetup { parameters, mut sheet, simulation, teams, mut dirty, time } =
            PhaseTestSetup::new();
        let mut end = Current::new(Team::B);

        end.update(&mut dirty, &mut sheet, &simulation, &parameters, time);
        dirty.check_and_clear(&Dirty::new());

        end.phase = Phase::Finished { score: Default::default() };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        assert!(end.start_delivery(delivery, time).is_err());
        dirty.check_and_clear(&Dirty::new());
        end.update(&mut dirty, &mut sheet, &simulation, &parameters, time);
        dirty.check_and_clear(&Dirty::new());
    }

    #[test]
    fn finished_turn_conversion() {
        let unfinished = Current::new(Team::A);
        let sheet = Sheet::new(sheet::Parameters::default());
        let finished = Current {
            phase: Phase::Finished { score: Score { a: 2, b: 0 } },
            ..Current::new_with_turns_finished(&sheet, Team::A, stone::COUNT)
        };
        assert!(TryInto::<Finished>::try_into(unfinished).is_err());
        let converted: Finished =
            finished.try_into().expect("Finished turn should convert successfully");
        assert_eq!(converted.hammer, Team::A);
        assert_eq!(converted.turns.len(), stone::COUNT);
        assert_eq!(converted.score, Score { a: 2, b: 0 });
    }
}
