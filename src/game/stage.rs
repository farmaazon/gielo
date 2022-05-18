use crate::game::team::Team;
use crate::game::{stone, Delivery, Score};
use std::time;

pub type EndNo = u8;

#[derive(Copy, Clone, Debug)]
pub struct End {
    pub no: EndNo,
    pub hammer: Team,
    pub stone: stone::Id,
    pub playing_team: Team,
}

impl End {
    pub fn first_end(hammer: Team) -> Self {
        Self { no: 1, hammer, stone: 0, playing_team: hammer.opponent() }
    }

    pub fn next_end(self, prev_score: Score) -> Self {
        let hammer = if prev_score.a > prev_score.b {
            Team::B
        } else if prev_score.a < prev_score.b {
            Team::A
        } else {
            self.hammer
        };
        End { no: self.no + 1, hammer, stone: 0, playing_team: hammer.opponent() }
    }
}

#[derive(Debug)]
pub enum Stage {
    Thinking(End),
    Delivering { end: End, started_at: time::Instant, delivery: Delivery },
    EndConcluded(End, Score),
    GameConcluded(Score),
}
