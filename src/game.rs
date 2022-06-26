pub mod delivery;
pub mod score;
pub mod sheet;
pub mod shot;
pub mod stage;
pub mod stone;
pub mod stones;
pub mod team;

use crate::game::sheet::Hack;
pub use crate::game::stage::Stage;
use crate::game::team::{PerTeam, Team};
use crate::unit::{seconds, Time};
use anyhow::{bail, Result};
pub use delivery::Delivery;
pub use score::Score;
pub use sheet::Sheet;
use std::time;
pub use stone::Stone;

pub const MAX_ENDS: usize = 11;

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
pub struct Game {
    pub params: Parameters,
    pub teams: PerTeam<team::Info>,
    pub first_hammer: Team,
    pub score: score::Table,
    pub sheet: Sheet,
    pub stage: Stage,
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
            first_hammer,
            score: score::Table::new(),
            sheet: Sheet::new(sheet_params),
            stage: Stage::Thinking(stage::End::first_end(first_hammer)),
        }
    }

    pub fn current_end_stage(&self) -> Option<&stage::End> {
        match &self.stage {
            Stage::Thinking(end) => Some(end),
            Stage::Delivering { end, .. } => Some(end),
            Stage::EndConcluded(end, _) => Some(end),
            Stage::GameConcluded(_) => None,
        }
    }

    pub fn delivered_stone(&self) -> Option<&Stone> {
        match &self.stage {
            Stage::Delivering { end, .. } => Some(&self.sheet.stones[end.stone]),
            _ => None,
        }
    }

    pub fn delivery_time(&self) -> Time {
        match &self.stage {
            Stage::Delivering { delivery, .. } => delivery.current_time,
            _ => Time::default(),
        }
    }

    pub fn update(&mut self, now: time::Instant) {
        let new_stage = match &mut self.stage {
            Stage::Delivering { end, started_at, delivery } => {
                let real_time = seconds((now - *started_at).as_secs_f32());
                let game_time = real_time * self.params.speed_factor;
                let finished = delivery.run(&mut self.sheet, Some(game_time));
                if finished {
                    Some(Self::stone_delivered(*end, &self.sheet, &mut self.score))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(stage) = new_stage {
            self.stage = stage
        }
    }

    fn stone_delivered(end: stage::End, sheet: &Sheet, score: &mut score::Table) -> Stage {
        let next_stone = end.stone + 1;
        if next_stone < stones::COUNT {
            Stage::Thinking(stage::End {
                stone: next_stone,
                playing_team: end.playing_team.opponent(),
                ..end
            })
        } else {
            let end_score = sheet.count_score();
            score.push_end(end_score);
            Stage::EndConcluded(end, end_score)
        }
    }

    pub fn start_delivery(&mut self, now: time::Instant, call: shot::Call) -> Result<()> {
        self.stage = match &mut self.stage {
            Stage::Thinking(end) => {
                let hack = Hack::Left;
                let delivery_params = delivery::Parameters {
                    angle: call.angle(hack, &self.sheet.parameters),
                    weight: call.weight,
                    team: end.playing_team,
                    hack,
                    rotation: call.rotation,
                };
                Stage::Delivering {
                    end: *end,
                    started_at: now,
                    delivery: Delivery::new(&mut self.sheet, delivery_params),
                }
            }
            stage => bail!("Starting delivery at wrong stage {:?}", stage),
        };
        Ok(())
    }

    pub fn finish_end(&mut self) -> Result<()> {
        self.stage = match &mut self.stage {
            Stage::EndConcluded(end, end_score) => {
                let score = self.score.full();
                let tied = score.a == score.b;
                if tied || end.no < self.params.ends {
                    self.sheet.stones.clear();
                    Stage::Thinking(end.next_end(*end_score))
                } else {
                    Stage::GameConcluded(score)
                }
            }
            stage => bail!("Going to next end at wrong stage {:?}", stage),
        };
        Ok(())
    }
}

pub type Shared = std::rc::Rc<std::cell::RefCell<Game>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::stone::Rotation;
    use crate::game::Stage::Thinking;
    use crate::unit::feet;
    use crate::vector::Vector2;
    use slint::Color;
    use std::time::{Duration, Instant};

    struct Fixture {
        teams: PerTeam<team::Info>,
        params: Parameters,
        sheet_params: sheet::Parameters,
    }

    fn tee_draw(sheet: &sheet::Parameters) -> shot::Call {
        shot::Call {
            weight: seconds(3.0),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }

    impl Fixture {
        fn make_empty_sheet(&self, played_stones: usize, starting_team: Team) -> Sheet {
            Sheet {
                parameters: self.sheet_params,
                stones: (0..played_stones)
                    .map(|i| Stone {
                        team: if i % 2 == 0 { starting_team } else { starting_team.opponent() },
                        state: stone::State::Out { dirty: true },
                    })
                    .collect(),
            }
        }

        fn make_game_with_empty_sheet(
            &self,
            stage: Stage,
            score: score::Table,
            played_stones: usize,
            starting_team: Team,
        ) -> Game {
            Game {
                params: self.params,
                teams: self.teams.clone(),
                first_hammer: Team::B,
                score,
                sheet: self.make_empty_sheet(played_stones, starting_team),
                stage,
            }
        }

        fn make_game_at_thinking_stage(&self, end: stage::End, score: score::Table) -> Game {
            self.make_game_with_empty_sheet(Thinking(end), score, end.stone, end.hammer.opponent())
        }
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                teams: PerTeam {
                    a: team::Info { name: "A".into(), color: Color::from_rgb_u8(255, 0, 0) },
                    b: team::Info { name: "B".into(), color: Color::from_rgb_u8(255, 255, 0) },
                },
                params: Parameters { speed_factor: 2.0, ends: 8 },
                sheet_params: sheet::Parameters::default(),
            }
        }
    }

    #[test]
    fn first_stone() {
        let Fixture { teams, params, sheet_params } = Fixture::default();
        let mut game = Game::new(teams, params, sheet_params, Team::B);
        assert!(matches!(
            game.stage,
            Stage::Thinking(stage::End { no: 1, hammer: Team::B, stone: 0, playing_team: Team::A })
        ));

        let delivery_time = Instant::now();
        game.start_delivery(delivery_time, tee_draw(&game.sheet.parameters))
            .expect("Error while starting delivery");
        assert!(matches!(
            &game.stage,
            Stage::Delivering {
                end: stage::End { no: 1, hammer: Team::B, stone: 0, playing_team: Team::A },
                started_at,
                delivery
            } if *started_at == delivery_time && delivery.current_time == seconds(0.0)
        ));
        let stone = game.delivered_stone().expect("No stone is delivered during delivery");
        assert!(matches!(&stone.state, stone::State::BeingDelivered(_)));

        let in_the_middle_time = delivery_time + Duration::from_secs_f32(6.0);
        game.update(in_the_middle_time);
        assert!(matches!(
            &game.stage,
            Stage::Delivering {
                end: stage::End { no: 1, hammer: Team::B, stone: 0, playing_team: Team::A },
                started_at,
                delivery
            } if *started_at == delivery_time && delivery.current_time == seconds(12.0)
        ));
        let stone = game.delivered_stone().expect("No stone is delivered during delivery");
        assert!(matches!(&stone.state, stone::State::Moving(_)));

        let when_stopped = delivery_time + Duration::from_secs_f32(20.0);
        game.update(when_stopped);
        assert!(matches!(
            game.stage,
            Stage::Thinking(stage::End { no: 1, hammer: Team::B, stone: 1, playing_team: Team::B })
        ));
        let stone = game.sheet.stones.last().expect("No stones after finished delivery");
        assert!(matches!(&stone.state, stone::State::Stationary(_)));
    }

    #[test]
    fn last_stone_of_the_end() {
        let test = Fixture::default();
        let end =
            stage::End { no: 1, hammer: Team::B, stone: stones::COUNT - 1, playing_team: Team::B };
        let mut game = test.make_game_at_thinking_stage(end, score::Table::default());

        let delivery_time = Instant::now();
        game.start_delivery(delivery_time, tee_draw(&game.sheet.parameters))
            .expect("Error while starting delivery");
        game.update(delivery_time + Duration::from_secs_f32(20.0));
        assert!(matches!(
            game.stage,
            Stage::EndConcluded(finished_end, score) if finished_end == end && score == (0, 1).into()
        ));

        game.finish_end().expect("Error while finishing end");
        let next_end = end.next_end((0, 1).into());
        assert!(matches!(
            game.stage,
            Stage::Thinking(end) if end == next_end
        ));
        assert_eq!(game.score.ends(), &[(0, 1).into()]);
    }

    #[test]
    fn last_stone_of_the_game() {
        let test = Fixture::default();
        let end = stage::End {
            no: test.params.ends,
            hammer: Team::A,
            stone: stones::COUNT - 1,
            playing_team: Team::A,
        };
        let score =
            score::Table::from_end_scores([(0, 1), (0, 0), (2, 0), (0, 1), (0, 0), (2, 0), (0, 2)]);
        let mut game = test.make_game_at_thinking_stage(end, score);
        let delivery_time = Instant::now();
        game.start_delivery(delivery_time, tee_draw(&game.sheet.parameters))
            .expect("Error while starting delivery");
        game.update(delivery_time + Duration::from_secs_f32(20.0));
        assert!(matches!(
            game.stage,
            Stage::EndConcluded(finished_end, score) if finished_end == end && score == (1, 0).into()
        ));
        assert_eq!(game.score.full(), (5, 4).into());

        game.finish_end().expect("Error while finishing end");
        assert!(matches!(
            game.stage,
            Stage::GameConcluded(end_score) if end_score == (5, 4).into()
        ));
    }

    #[test]
    fn extra_end() {
        let test = Fixture::default();
        let score = score::Table::from_end_scores([
            (0, 1),
            (0, 0),
            (2, 0),
            (0, 1),
            (0, 0),
            (2, 0),
            (0, 0),
            (0, 2),
        ]);

        let end = stage::End {
            no: test.params.ends,
            hammer: Team::A,
            stone: stones::COUNT,
            playing_team: Team::A,
        };
        let next_end = end.next_end((0, 2).into());
        let stage = Stage::EndConcluded(end, (0, 2).into());
        let mut game = test.make_game_with_empty_sheet(stage, score, stones::COUNT, Team::A);
        game.finish_end().expect("Error while finishing end");
        assert!(matches!(game.stage, Stage::Thinking(end) if end == next_end));
        assert_eq!(game.score.full(), (4, 4).into());
    }

    #[test]
    fn actions_in_wrong_stage() {
        let test = Fixture::default();
        let end =
            stage::End { no: test.params.ends, hammer: Team::A, stone: 3, playing_team: Team::B };
        let mut game = test.make_game_at_thinking_stage(end, score::Table::default());
        assert!(game.finish_end().is_err());
        assert!(matches!(game.stage, Stage::Thinking(end_after) if end_after == end));

        game.start_delivery(Instant::now(), tee_draw(&game.sheet.parameters))
            .expect("Error while starting delivery");
        assert!(game.start_delivery(Instant::now(), tee_draw(&game.sheet.parameters)).is_err());
        assert!(game.finish_end().is_err());
        assert!(matches!(game.stage, Stage::Delivering{ end: end_after, ..} if end_after == end));

        let end_score = (0, 1).into();
        let score = score::Table::from_end_scores([end_score]);
        let mut game = test.make_game_with_empty_sheet(
            Stage::EndConcluded(end, end_score),
            score,
            stones::COUNT,
            Team::A,
        );
        assert!(game.start_delivery(Instant::now(), tee_draw(&game.sheet.parameters)).is_err());
    }
}
