use crate::game::team::Team;
use crate::game::{stone, Delivery, Score};
use std::time;

pub type EndNo = u8;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet;

    #[test]
    fn next_end() {
        let end = End { no: 2, hammer: Team::B, stone: sheet::STONE_COUNT, playing_team: Team::B };
        let new_end_hammer_b = End { no: 3, hammer: Team::B, stone: 0, playing_team: Team::A };
        let new_end_hammer_a = End { hammer: Team::A, playing_team: Team::B, ..new_end_hammer_b };

        assert_eq!(end.next_end((2, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((1, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((0, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((0, 1).into()), new_end_hammer_a);
        assert_eq!(end.next_end((0, 2).into()), new_end_hammer_a);
    }
}
