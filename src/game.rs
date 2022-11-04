pub mod dirty;
pub mod end;
pub mod sheet;
pub mod team;
pub mod turn;

pub use crate::game::dirty::Dirty;
use crate::game::sheet::stone::Stone;
use crate::game::team::{PerTeam, Team};
use crate::unit::Time;
use anyhow::{bail, Result};
pub use sheet::Sheet;
use std::cmp::Ordering;
use std::time;

pub type Score = PerTeam<u8>;

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    pub speed_factor: f32,
    pub ends: u8,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { speed_factor: 10.0, ends: 8 }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Phase {
    End(end::Current),
    GameConcluded,
}

impl TryFrom<Phase> for end::Current {
    type Error = anyhow::Error;

    fn try_from(phase: Phase) -> Result<Self> {
        match phase {
            Phase::End(end) => Ok(end),
            Phase::GameConcluded => bail!("Tried to get current end from finished game"),
        }
    }
}

#[derive(Debug)]
pub struct Game {
    pub params: Parameters,
    pub teams: PerTeam<team::Info>,
    pub sheet: Sheet,
    pub finished_ends: Vec<end::Finished>,
    pub score: Score,
    pub phase: Phase,
}

impl Game {
    pub fn new(
        teams: PerTeam<team::Info>,
        params: Parameters,
        sheet_params: sheet::Parameters,
        first_hammer: Team,
    ) -> Self {
        Self {
            params,
            teams,
            sheet: Sheet::new(sheet_params),
            finished_ends: vec![],
            score: Score::default(),
            phase: Phase::End(end::Current::new(first_hammer)),
        }
    }

    pub fn current_end(&self) -> Option<&end::Current> {
        match &self.phase {
            Phase::End(end) => Some(end),
            _ => None,
        }
    }

    pub fn current_turn(&self) -> Option<&turn::Current> {
        self.current_end().and_then(|end| end.current_turn())
    }

    pub fn is_finished(&self) -> bool {
        matches!(&self.phase, Phase::GameConcluded)
    }

    pub fn is_end_finished(&self) -> bool {
        matches!(self.current_end(), Some(end::Current { phase: end::Phase::Finished { .. }, .. }))
    }

    pub fn is_thinking(&self) -> bool {
        matches!(self.current_turn(), Some(turn::Current { phase: turn::Phase::Thinking, .. }))
    }

    pub fn is_delivering(&self) -> bool {
        matches!(
            self.current_turn(),
            Some(turn::Current { phase: turn::Phase::Delivering { .. }, .. })
        )
    }

    pub fn delivered_stone(&self) -> Option<&Stone> {
        match &self.phase {
            Phase::End(end) => Some(&self.sheet.stones[end.delivered_stone()?]),
            _ => None,
        }
    }

    pub fn delivery_time(&self) -> Time {
        match &self.phase {
            Phase::End(end) => end.current_turn().map_or_else(Time::default, |t| t.delivery_time()),
            _ => Time::default(),
        }
    }

    pub fn update(&mut self, dirty: &mut Dirty, now: time::Instant) {
        if let Phase::End(end) = &mut self.phase {
            end.update(dirty, &mut self.sheet, now, self.params.speed_factor)
        }
    }

    pub fn start_delivery(
        &mut self,
        dirty: &mut Dirty,
        call: turn::delivery::Call,
        now: time::Instant,
    ) -> Result<()> {
        match &mut self.phase {
            Phase::End(end) => {
                let delivery = turn::delivery::Start { call, sheet: &mut self.sheet, dirty };
                end.start_delivery(delivery, now)
            }
            _ => bail!("Starting delivery at wrong game phase"),
        }
    }

    pub fn proceed(&mut self, dirty: &mut Dirty) -> Result<()> {
        let new_phase = match &mut self.phase {
            Phase::End(end::Current { phase: end::Phase::Finished { score }, hammer, .. }) => {
                self.score += *score;
                dirty.score = true;
                let tied = self.score.a == self.score.b;
                if tied || self.finished_ends.len() + 1 < self.params.ends as usize {
                    self.sheet.stones.clear(dirty);
                    let new_hammer = match score.a.cmp(&score.b) {
                        Ordering::Less => Team::A,
                        Ordering::Equal => *hammer,
                        Ordering::Greater => Team::B,
                    };
                    Some(Phase::End(end::Current::new(new_hammer)))
                } else {
                    Some(Phase::GameConcluded)
                }
            }
            Phase::End(end) => {
                end.proceed(dirty, &self.sheet)?;
                None
            }
            _ => bail!("Proceeding at wrong game phase"),
        };
        if let Some(new_phase) = new_phase {
            let previous_phase = std::mem::replace(&mut self.phase, new_phase);
            let end: end::Current = previous_phase.try_into().unwrap();
            self.finished_ends.push(end.try_into().unwrap());
            dirty.phase = true;
            dirty.finished_ends_count += 1;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn new_with_ends_finished(
        teams: PerTeam<team::Info>,
        params: Parameters,
        sheet_params: sheet::Parameters,
        hammer: Team,
        finished_end_scores: Vec<Score>,
    ) -> Self {
        let mut game = Self::new(teams, params, sheet_params, hammer);
        game.finished_ends = finished_end_scores
            .iter()
            .map(|&score| end::Finished { score, ..end::Finished::new_with_all_stones_out(hammer) })
            .collect();
        game.score = finished_end_scores.into_iter().sum();
        game
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet::stones;
    use crate::game::turn::delivery;
    use slint::Color;

    fn mock_teams() -> PerTeam<team::Info> {
        PerTeam {
            a: team::Info { name: "A".into(), color: Color::from_rgb_u8(255, 0, 0) },
            b: team::Info { name: "B".into(), color: Color::from_rgb_u8(255, 255, 0) },
        }
    }

    #[test]
    fn state_of_new_game() {
        let game =
            Game::new(mock_teams(), Parameters::default(), sheet::Parameters::default(), Team::B);
        assert!(matches!(
            game.current_turn(),
            Some(turn::Current { playing_team: Team::A, phase: turn::Phase::Thinking })
        ));
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, .. }) if finished_turns.is_empty()
        ));
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
    }

    #[test]
    fn last_stone_in_end() {
        let mut game =
            Game::new(mock_teams(), Parameters::default(), sheet::Parameters::default(), Team::B);
        let Phase::End(end) = &mut game.phase else {panic!("Wrong phase at game start"); };
        *end = end::Current::new_with_turns_finished(Team::B, stones::COUNT - 1);
        game.sheet.stones = end.finished_turns.last().unwrap().snapshot.clone();
        assert!(matches!(
            game.current_turn(),
            Some(turn::Current { playing_team: Team::B, phase: turn::Phase::Thinking })
        ));

        let delivery_time = time::Instant::now();
        let mut dirty = Dirty::new();
        game.start_delivery(
            &mut dirty,
            delivery::Call::tee_draw(&sheet::Parameters::default()),
            delivery_time,
        )
        .expect("Error while starting delivery");
        assert!(matches!(
            game.current_turn(),
            Some(turn::Current { playing_team: Team::B, phase: turn::Phase::Delivering { .. } })
        ));
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, .. }) if finished_turns.len() == stones::COUNT - 1
        ));
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
        dirty.check_and_clear(&Dirty { stone_count: 1, phase: true, ..Dirty::default() });

        let when_stopped = delivery_time + time::Duration::from_secs(4);
        game.update(&mut dirty, when_stopped);
        assert!(game.current_turn().is_none());
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, .. }) if finished_turns.len() == stones::COUNT
        ));
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
        dirty.check_and_clear(&Dirty {
            phase: true,
            stones: 1 << (stones::COUNT - 1),
            ..Dirty::default()
        });

        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert!(matches!(
            game.current_turn(),
            Some(turn::Current { playing_team: Team::B, phase: turn::Phase::Thinking })
        ));
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, ..}) if finished_turns.is_empty()
        ));
        assert_eq!(game.finished_ends.len(), 1);
        assert_eq!(game.finished_ends[0].score, Score { a: 0, b: 1 });
        assert_eq!(game.score, Score { a: 0, b: 1 });
        dirty.check_and_clear(&Dirty {
            stone_count: -(stones::COUNT as isize),
            finished_ends_count: 1,
            phase: true,
            score: true,
            ..Dirty::default()
        });
    }

    #[test]
    fn last_stone_in_game() {
        let mut game = Game::new_with_ends_finished(
            mock_teams(),
            Parameters::default(),
            sheet::Parameters::default(),
            Team::B,
            vec![
                Score { a: 0, b: 0 },
                Score { a: 0, b: 1 },
                Score { a: 0, b: 2 },
                Score { a: 3, b: 0 },
                Score { a: 0, b: 3 },
                Score { a: 1, b: 0 },
                Score { a: 0, b: 1 },
            ],
        );
        let Phase::End(end) = &mut game.phase else {panic!("Wrong phase at game start"); };
        *end = end::Current::new_with_turns_finished(Team::A, stones::COUNT - 1);
        game.sheet.stones = end.finished_turns.last().unwrap().snapshot.clone();

        let delivery_time = time::Instant::now();
        let when_stopped = delivery_time + time::Duration::from_secs(4);
        let mut dirty = Dirty::new();
        game.start_delivery(
            &mut dirty,
            delivery::Call::tee_draw(&sheet::Parameters::default()),
            delivery_time,
        )
        .expect("Error while starting delivery");
        game.update(&mut dirty, when_stopped);
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, .. }) if finished_turns.len() == stones::COUNT
        ));
        assert_eq!(game.finished_ends.len(), 7);
        assert_eq!(game.score, Score { a: 4, b: 7 });
        dirty.check_and_clear(&Dirty {
            stone_count: 1,
            stones: 1 << (stones::COUNT - 1),
            phase: true,
            ..Dirty::default()
        });

        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert!(matches!(&game.phase, Phase::GameConcluded));
        assert!(game.current_turn().is_none());
        assert!(game.current_end().is_none());
        assert_eq!(game.finished_ends.len(), 8);
        assert_eq!(game.score, Score { a: 5, b: 7 });
        dirty.check_and_clear(&Dirty {
            finished_ends_count: 1,
            phase: true,
            score: true,
            ..Dirty::default()
        });
    }

    #[test]
    fn extra_end() {
        let mut game = Game::new_with_ends_finished(
            mock_teams(),
            Parameters::default(),
            sheet::Parameters::default(),
            Team::B,
            vec![
                Score { a: 1, b: 0 },
                Score { a: 0, b: 1 },
                Score { a: 0, b: 2 },
                Score { a: 3, b: 0 },
                Score { a: 0, b: 3 },
                Score { a: 2, b: 0 },
                Score { a: 0, b: 1 },
            ],
        );
        let Phase::End(end) = &mut game.phase else {panic!("Wrong phase at game start"); };
        *end = end::Current::new_with_turns_finished(Team::A, stones::COUNT - 1);
        game.sheet.stones = end.finished_turns.last().unwrap().snapshot.clone();

        let delivery_time = time::Instant::now();
        let when_stopped = delivery_time + time::Duration::from_secs(4);

        game.start_delivery(
            &mut Dirty::new(),
            delivery::Call::tee_draw(&sheet::Parameters::default()),
            delivery_time,
        )
        .expect("Error while starting delivery");
        game.update(&mut Dirty::new(), when_stopped);

        let mut dirty = Dirty::new();
        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert!(matches!(&game.phase, Phase::End(_)));
        assert!(matches!(
            game.current_turn(),
            Some(turn::Current { playing_team: Team::A, phase: turn::Phase::Thinking })
        ));
        assert!(matches!(
            game.current_end(),
            Some(end::Current { finished_turns, ..}) if finished_turns.is_empty()
        ));
        assert_eq!(game.finished_ends.len(), 8);
        assert_eq!(game.score, Score { a: 7, b: 7 });
        dirty.check_and_clear(&Dirty {
            stone_count: -(stones::COUNT as isize),
            finished_ends_count: 1,
            phase: true,
            score: true,
            ..Dirty::default()
        });
    }
}
