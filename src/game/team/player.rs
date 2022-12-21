use crate::game::sheet::Hack;
use crate::game::team::TEAMS_COUNT;
use crate::game::{stone, turn};
use crate::unit;
use crate::unit::{degrees, seconds};
use rand::distributions::Distribution;
use uom::si::angle::degree;
use uom::si::time::second;
use uom::ConstZero;

pub const PER_TEAM_COUNT: usize = 4;
pub const STONES_PER_PLAYER: usize = stone::COUNT_PER_TEAM / PER_TEAM_COUNT;

pub type Id = usize;

pub fn who_is_delivering(turn: turn::Index) -> Id {
    turn / TEAMS_COUNT / STONES_PER_PLAYER
}

#[derive(Copy, Clone, Debug)]
pub struct Player {
    pub used_hack: Hack,
    pub angle_std_dev: unit::Angle,
    pub weight_std_dev: unit::Time,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            used_hack: Hack::Left,
            angle_std_dev: unit::Angle::ZERO,
            weight_std_dev: unit::Time::ZERO,
        }
    }
}

impl Player {
    pub fn rand_angle_error(&self) -> unit::Angle {
        let angle_dist = rand_distr::Normal::new(0.0, self.angle_std_dev.get::<degree>()).unwrap();
        degrees(angle_dist.sample(&mut rand::thread_rng()))
    }

    pub fn rand_weight_error(&self) -> unit::Time {
        let weight_dist =
            rand_distr::Normal::new(0.0, self.weight_std_dev.get::<second>()).unwrap();
        seconds(weight_dist.sample(&mut rand::thread_rng()))
    }
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
