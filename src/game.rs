pub mod delivery;
pub mod sheet;
pub mod shot;
pub mod stage;
pub mod stone;
pub mod team;

use crate::game::sheet::Hack;
pub use crate::game::stage::Stage;
use crate::game::team::{PerTeam, Team};
use crate::unit::seconds;
use anyhow::{bail, Result};
pub use delivery::Delivery;
use derive_more::{AsRef, Deref, DerefMut};
use local_vec::LocalVec;
pub use sheet::Sheet;
use std::time;
pub use stone::Stone;

const MAX_ENDS: usize = 11;

pub type Score = PerTeam<u8>;

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    speed_factor: f32,
    ends: u8,
}

#[derive(Clone, Debug, Default, AsRef, Deref, DerefMut, Eq, PartialEq)]
pub struct FullScore(pub LocalVec<PerTeam<u8>, MAX_ENDS>);

impl FullScore {
    fn score(&self) -> Score {
        self.iter().fold(PerTeam::<u8>::default(), |sum, score| sum + *score)
    }
}

#[derive(Debug)]
pub struct Game {
    params: Parameters,
    teams: PerTeam<team::Info>,
    first_hammer: Team,
    full_score: FullScore,
    sheet: Sheet,
    stage: Stage,
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
            full_score: FullScore::default(),
            sheet: Sheet::new(sheet_params),
            stage: Stage::Thinking(stage::End::first_end(first_hammer)),
        }
    }

    pub fn teams(&self) -> &PerTeam<team::Info> {
        &self.teams
    }

    pub fn score(&self) -> PerTeam<u8> {
        self.full_score.score()
    }

    pub fn full_score(&self) -> &FullScore {
        &self.full_score
    }

    pub fn first_hammer(&self) -> Team {
        self.first_hammer
    }

    pub fn delivered_stone(&self) -> Option<&Stone> {
        match &self.stage {
            Stage::Delivering { end, .. } => Some(&self.sheet.stones[end.stone]),
            _ => None,
        }
    }

    pub fn update(&mut self, now: time::Instant) {
        let new_stage = match &mut self.stage {
            Stage::Delivering { end, started_at, delivery } => {
                let real_time = seconds((now - *started_at).as_secs_f32());
                let game_time = real_time * self.params.speed_factor;
                let finished = delivery.run(&mut self.sheet, Some(game_time));
                if finished {
                    Some(Self::stone_delivered(*end, &self.sheet, &mut self.full_score))
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

    fn stone_delivered(end: stage::End, sheet: &Sheet, full_score: &mut FullScore) -> Stage {
        let next_stone = end.stone + 1;
        if next_stone < sheet::STONE_COUNT {
            Stage::Thinking(stage::End {
                stone: next_stone,
                playing_team: end.playing_team.opponent(),
                ..end
            })
        } else {
            let score = sheet.count_score();
            full_score.push(score);
            Stage::EndConcluded(end, score)
        }
    }

    pub fn start_delivery(&mut self, now: time::Instant, call: shot::Call) -> Result<()> {
        self.stage = match &mut self.stage {
            Stage::Thinking(end) => {
                let hack = Hack::Left;
                let delivery_params = delivery::Parameters {
                    angle: call.angle(hack),
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
                let score = self.full_score.score();
                let tied = score.a == score.b;
                if tied || end.no < self.params.ends {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::stone::Rotation;
    use crate::unit::feet;
    use crate::vector::Vector2;
    use slint::Color;
    use std::time::{Duration, Instant};

    struct Fixture {
        teams: PerTeam<team::Info>,
        params: Parameters,
        sheet_params: sheet::Parameters,
    }

    fn tee_draw() -> shot::Call {
        shot::Call {
            weight: seconds(3.0),
            mark: *sheet::TEE + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }

    impl Fixture {
        fn make_empty_sheet(&self, played_stones: usize, starting_team: Team) -> Sheet {
            let mut stones = LocalVec::new();
            for i in 0..played_stones {
                stones.push(Stone {
                    team: if i % 2 == 0 { starting_team } else { starting_team.opponent() },
                    state: stone::State::Out,
                });
            }
            Sheet { stones, parameters: self.sheet_params }
        }

        fn make_game_at_thinking_stage(&self, end: stage::End, full_score: FullScore) -> Game {
            Game {
                params: self.params,
                teams: self.teams.clone(),
                first_hammer: Team::B,
                full_score,
                sheet: self.make_empty_sheet(end.stone, end.hammer.opponent()),
                stage: Stage::Thinking(end),
            }
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
        game.start_delivery(delivery_time, tee_draw()).expect("Error while starting delivery");
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
        let end = stage::End {
            no: 1,
            hammer: Team::B,
            stone: sheet::STONE_COUNT - 1,
            playing_team: Team::B,
        };
        let mut game = test.make_game_at_thinking_stage(end, FullScore::default());

        let delivery_time = Instant::now();
        game.start_delivery(delivery_time, tee_draw()).expect("Error while starting delivery");
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
        assert_eq!(game.full_score(), &FullScore(LocalVec::from_array([(0, 1).into()])));
    }
}
