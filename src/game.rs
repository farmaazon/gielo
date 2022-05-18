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

#[derive(Clone, Debug, Default, AsRef, Deref, DerefMut)]
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
