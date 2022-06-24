use crate::game::team::{Team, TEAMS_COUNT};
use crate::game::{stone, stones, Delivery, Score};
use std::cmp::Ordering;
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
        let hammer = match prev_score.a.cmp(&prev_score.b) {
            Ordering::Less => Team::A,
            Ordering::Greater => Team::B,
            Ordering::Equal => self.hammer,
        };
        End { no: self.no + 1, hammer, stone: 0, playing_team: hammer.opponent() }
    }

    pub fn stones_left(&self, team: Team) -> usize {
        let hammer_team_stones_delivered = self.stone / TEAMS_COUNT;
        let stones_delivered = if team == self.hammer || self.stone % 2 == 0 {
            hammer_team_stones_delivered
        } else {
            hammer_team_stones_delivered + 1
        };
        stones::PER_TEAM - stones_delivered
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

    #[test]
    fn next_end() {
        let end = End { no: 2, hammer: Team::B, stone: stones::COUNT, playing_team: Team::B };
        let new_end_hammer_b = End { no: 3, hammer: Team::B, stone: 0, playing_team: Team::A };
        let new_end_hammer_a = End { hammer: Team::A, playing_team: Team::B, ..new_end_hammer_b };

        assert_eq!(end.next_end((2, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((1, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((0, 0).into()), new_end_hammer_b);
        assert_eq!(end.next_end((0, 1).into()), new_end_hammer_a);
        assert_eq!(end.next_end((0, 2).into()), new_end_hammer_a);
    }
}
