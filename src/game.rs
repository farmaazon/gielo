pub mod dirty;
pub mod end;
pub mod sheet;
pub mod simulation;
pub mod stone;
pub mod team;
pub mod turn;

pub use crate::game::dirty::Dirty;
use crate::{
    game::{
        simulation::Simulation,
        team::{PerTeam, Team},
    },
    unit::Time,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
pub use sheet::Sheet;
use std::{cmp::Ordering, time};

pub type Score = PerTeam<u8>;

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Rules {
    pub free_guard_rule_stones: usize,
    pub no_tick_rule_stones: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self { free_guard_rule_stones: 5, no_tick_rule_stones: 5 }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Parameters {
    pub speed_factor: f32,
    pub ends: u8,
    pub rules: Rules,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { speed_factor: 10.0, ends: 8, rules: Rules::default() }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game {
    pub params: Parameters,
    pub simulation: Simulation,
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
        simulation_params: simulation::Parameters,
        first_hammer: Team,
    ) -> Self {
        let sheet = Sheet::new(sheet_params);
        let simulation = Simulation::new(simulation_params, &sheet_params);
        Self {
            params,
            teams,
            sheet,
            simulation,
            finished_ends: vec![],
            score: Score::default(),
            phase: Phase::End(end::Current::new(first_hammer)),
        }
    }

    pub fn new_with_default_params(teams: PerTeam<team::Info>, first_hammer: Team) -> Self {
        Self::new(
            teams,
            Parameters::default(),
            sheet::Parameters::default(),
            simulation::Parameters::default(),
            first_hammer,
        )
    }

    pub fn first_hammer(&self) -> Team {
        self.finished_ends
            .first()
            .map_or_else(|| self.current_end().map_or(Team::A, |end| end.hammer), |end| end.hammer)
    }

    pub fn current_end(&self) -> Option<&end::Current> {
        match &self.phase {
            Phase::End(end) => Some(end),
            _ => None,
        }
    }

    pub fn current_end_number(&self) -> Option<u8> {
        match &self.phase {
            Phase::End(_) => Some((self.finished_ends.len() + 1) as u8),
            _ => None,
        }
    }

    pub fn current_turn(&self) -> Option<&turn::Current> {
        self.current_end().and_then(|end| end.current_turn())
    }

    pub fn current_turn_number(&self) -> Option<u8> {
        self.current_end().and_then(|end| end.current_turn_number())
    }

    pub fn playing_team(&self) -> Option<Team> {
        self.current_turn().map(|turn| turn.playing_team())
    }

    pub fn delivering_player(&self) -> Option<team::player::Id> {
        self.current_turn().map(|turn| turn.delivering_player)
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

    pub fn delivered_stone(&self) -> Option<stone::Id> {
        match &self.phase {
            Phase::End(end) => end.delivered_stone(),
            _ => None,
        }
    }

    pub fn delivery_time(&self) -> Time {
        match &self.phase {
            Phase::End(end) => end.current_turn().map_or_else(Time::default, |t| t.delivery_time),
            _ => Time::default(),
        }
    }

    pub fn update(&mut self, dirty: &mut Dirty, now: time::Instant) {
        if let Phase::End(end) = &mut self.phase {
            end.update(dirty, &mut self.sheet, &self.simulation, &self.params, now)
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
                let delivery = turn::delivery::Start {
                    call,
                    sheet: &mut self.sheet,
                    dirty,
                    teams: &self.teams,
                };
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
                    dirty.preview = true;
                    Some(Phase::End(end::Current::new(new_hammer)))
                } else {
                    Some(Phase::GameConcluded)
                }
            }
            Phase::End(end) => {
                end.proceed(dirty, &mut self.sheet, &self.params)?;
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

    pub fn replace_stones(&mut self, dirty: &mut Dirty) -> Result<()> {
        match &mut self.phase {
            Phase::End(end) => end.replace_stones(dirty, &mut self.sheet, &self.params),
            _ => bail!("Replacing stones at wrong game phase"),
        }
    }

    pub fn expected_path(&self, call: turn::delivery::Call) -> Option<Vec<stone::Position>> {
        self.current_turn()
            .map(|turn| turn.expected_path(&self.sheet, &self.simulation, &self.teams, call))
    }

    #[cfg(test)]
    pub(crate) fn new_with_ends_finished(
        teams: PerTeam<team::Info>,
        params: Parameters,
        sheet_params: sheet::Parameters,
        simulation_params: simulation::Parameters,
        hammer: Team,
        finished_end_scores: Vec<Score>,
    ) -> Self {
        let mut game = Self::new(teams, params, sheet_params, simulation_params, hammer);
        game.finished_ends = finished_end_scores
            .iter()
            .map(|&score| end::Finished { score, ..end::Finished::new_with_all_stones_out(hammer) })
            .collect();
        game.score = finished_end_scores.into_iter().sum();
        game
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::turn::delivery;

    pub struct PhaseTestSetup {
        pub parameters: Parameters,
        pub sheet: Sheet,
        pub simulation: Simulation,
        pub teams: PerTeam<team::Info>,
        pub dirty: Dirty,
        pub time: time::Instant,
    }

    impl Default for PhaseTestSetup {
        fn default() -> Self {
            let sheet_params = sheet::Parameters::default();
            let simulation = Simulation::new(simulation::Parameters::default(), &sheet_params);
            Self {
                parameters: Parameters::default(),
                sheet: Sheet::new(sheet_params),
                simulation,
                teams: PerTeam::default(),
                dirty: Dirty::default(),
                time: time::Instant::now(),
            }
        }
    }

    impl PhaseTestSetup {
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[test]
    fn state_of_new_game() {
        let game = Game::new_with_default_params(PerTeam::default(), Team::B);
        let current_turn = game.current_turn().unwrap();
        assert_eq!(current_turn.played_stone, stone::QUEUE_BY_HAMMER.b[0]);
        assert!(matches!(current_turn.phase, turn::Phase::Thinking));
        let current_end = game.current_end().unwrap();
        assert_eq!(current_end.finished_turns.len(), 0);
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
        assert!(game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(!game.is_end_finished());
    }

    #[test]
    fn last_stone_in_end() {
        let mut game = Game::new_with_default_params(PerTeam::default(), Team::B);
        let stone = *stone::QUEUE_BY_HAMMER.b.last().unwrap();
        let Phase::End(end) = &mut game.phase else {
            panic!("Wrong phase at game start");
        };
        *end = end::Current::new_with_turns_finished(&game.sheet, Team::B, stone::COUNT - 1);
        assert!(game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(!game.is_end_finished());
        assert_eq!(game.delivered_stone(), None);

        let delivery_time = time::Instant::now();
        let mut dirty = Dirty::new();
        game.start_delivery(
            &mut dirty,
            delivery::Call::tee_draw(&sheet::Parameters::default()),
            delivery_time,
        )
        .expect("Error while starting delivery");
        assert!(!game.is_thinking());
        assert!(game.is_delivering());
        assert!(!game.is_finished());
        assert!(!game.is_end_finished());
        assert_eq!(game.delivered_stone(), Some(stone));
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(stone),
            phase: true,
            ..Dirty::default()
        });

        let when_stopped = delivery_time + time::Duration::from_secs(4);
        game.update(&mut dirty, when_stopped);
        assert!(!game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(game.is_end_finished());
        assert_eq!(game.delivered_stone(), None);
        assert_eq!(game.finished_ends.len(), 0);
        assert_eq!(game.score, Score { a: 0, b: 0 });
        dirty.check_and_clear(&Dirty {
            phase: true,
            stones: stone::Flag::stone(stone),
            ..Dirty::default()
        });

        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert!(game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(!game.is_end_finished());
        assert_eq!(game.delivered_stone(), None);
        assert_eq!(game.finished_ends.len(), 1);
        assert_eq!(game.finished_ends[0].score, Score { a: 0, b: 1 });
        assert_eq!(game.score, Score { a: 0, b: 1 });
        assert_eq!(game.current_end().unwrap().hammer, Team::A);
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(stone),
            finished_ends_count: 1,
            phase: true,
            score: true,
            preview: true,
        });
    }

    #[test]
    fn proceeding_after_blank() {
        let mut game = Game::new_with_default_params(PerTeam::default(), Team::B);
        let Phase::End(end) = &mut game.phase else {
            panic!("Wrong phase at game start");
        };
        *end = end::Current::new_with_turns_finished(&game.sheet, Team::B, stone::COUNT);
        assert!(game.is_end_finished());

        let mut dirty = Dirty::new();
        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert_eq!(game.current_end().unwrap().hammer, Team::B);
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag(0),
            finished_ends_count: 1,
            phase: true,
            score: true,
            preview: true,
        });
    }

    #[test]
    fn last_stone_in_game() {
        let mut game = Game::new_with_ends_finished(
            PerTeam::default(),
            Parameters::default(),
            sheet::Parameters::default(),
            simulation::Parameters::default(),
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
        let Phase::End(end) = &mut game.phase else {
            panic!("Wrong phase at game start");
        };
        *end = end::Current::new_with_turns_finished(&game.sheet, Team::A, stone::COUNT - 1);
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
        assert!(!game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(game.is_end_finished());
        assert_eq!(game.finished_ends.len(), 7);
        assert_eq!(game.score, Score { a: 4, b: 7 });
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(*stone::QUEUE_BY_HAMMER.a.last().unwrap()),
            phase: true,
            ..Dirty::default()
        });

        game.proceed(&mut dirty).expect("Proceeding finished end failed");
        assert!(matches!(&game.phase, Phase::GameConcluded));
        assert!(game.current_turn().is_none());
        assert!(game.current_end().is_none());
        assert!(!game.is_thinking());
        assert!(!game.is_delivering());
        assert!(game.is_finished());
        assert!(!game.is_end_finished());
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
            PerTeam::default(),
            Parameters::default(),
            sheet::Parameters::default(),
            simulation::Parameters::default(),
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
        let Phase::End(end) = &mut game.phase else {
            panic!("Wrong phase at game start");
        };
        *end = end::Current::new_with_turns_finished(&game.sheet, Team::A, stone::COUNT - 1);
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
        assert!(game.is_thinking());
        assert!(!game.is_delivering());
        assert!(!game.is_finished());
        assert!(!game.is_end_finished());
        assert_eq!(game.finished_ends.len(), 8);
        assert_eq!(game.score, Score { a: 7, b: 7 });
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(*stone::QUEUE_BY_HAMMER.a.last().unwrap()),
            finished_ends_count: 1,
            phase: true,
            score: true,
            preview: true,
        });
    }
}
