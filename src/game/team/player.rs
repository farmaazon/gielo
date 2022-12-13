use crate::game::team::TEAMS_COUNT;
use crate::game::{stone, turn};
use crate::unit;

pub const PER_TEAM_COUNT: usize = 4;
pub const STONES_PER_PLAYER: usize = stone::COUNT_PER_TEAM / PER_TEAM_COUNT;

pub type Id = usize;

pub fn who_is_delivering(turn: turn::Index) -> Id {
    turn / TEAMS_COUNT / STONES_PER_PLAYER
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Player {
    pub angle_std_dev: unit::Angle,
    pub weight_std_dev: unit::Time,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_is_delivering() {
        let expected_players = (0..PER_TEAM_COUNT)
            .flat_map(|p| std::iter::repeat(p).take(STONES_PER_PLAYER * TEAMS_COUNT));
        for (turn, expected_player) in (0..stone::COUNT).zip(expected_players) {
            assert_eq!(
                super::who_is_delivering(turn),
                expected_player,
                "Mismatch on turn {}",
                turn
            );
        }
    }
}
