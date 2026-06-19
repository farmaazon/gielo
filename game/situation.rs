use crate::{dirty::Dirty, game::TurnIndex, score::Score};
use gielo_sheet::{stone, stone::Stones};
use gielo_team as team;
use gielo_team::{TEAMS_COUNT, Team, player};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Situation {
    pub turn: TurnIndex,
    pub score: Score,
    pub hammer: Team,
    pub stones: Stones,
}

impl Default for Situation {
    fn default() -> Self {
        Self {
            turn: TurnIndex::new(0, 0),
            score: Score::default(),
            hammer: Team::B,
            stones: Stones::default(),
        }
    }
}

impl Situation {
    pub fn stone(&self) -> stone::Id {
        team::stone::QUEUE_BY_HAMMER[self.hammer][self.turn.stone()]
    }
    pub fn team(&self) -> Team {
        team::stone::team(self.stone())
    }
    pub fn player(&self) -> player::Id {
        player::who_is_delivering(self.turn.stone())
    }

    pub fn stones_left(&self, team: Team) -> usize {
        let stone = self.turn.stone();
        let hammer_team_stones_delivered = stone / TEAMS_COUNT;
        let stones_delivered = if team == self.hammer || stone.is_multiple_of(2) {
            hammer_team_stones_delivered
        } else {
            hammer_team_stones_delivered + 1
        };
        team::stone::COUNT_PER_TEAM - stones_delivered
    }

    pub fn restore(&mut self, dirty: &mut Dirty, snapshot: Situation) {
        self.stones.restore(&mut dirty.stones, snapshot.stones);
        self.turn = snapshot.turn;
        dirty.turn = true;
        self.score = snapshot.score;
        dirty.score = true;
        self.hammer = snapshot.hammer;
    }

    pub fn next_turn(&mut self, dirty: &mut Dirty) {
        self.turn = self.turn.next();
        dirty.turn = true;
    }

    pub fn add_to_score(&mut self, dirty: &mut Dirty, end_score: Score) {
        self.score += end_score;
        dirty.score = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gielo_team::teams;

    #[test]
    fn stones_left() {
        for hammer in teams() {
            let first_team = hammer.opponent();
            let mut situation =
                Situation { turn: TurnIndex::new(0, 0), hammer, ..Situation::default() };
            for expected_stones_left in (1..=team::stone::COUNT_PER_TEAM).rev() {
                assert_eq!(situation.stones_left(Team::A), expected_stones_left);
                assert_eq!(situation.stones_left(Team::B), expected_stones_left);
                situation.turn = situation.turn.next();
                assert_eq!(situation.stones_left(first_team), expected_stones_left - 1);
                assert_eq!(situation.stones_left(hammer), expected_stones_left);
                situation.turn = situation.turn.next();
            }
        }
    }
}
