use gielo_unit::{
    Angle, Length, Velocity, angle::degree, degrees, feet_per_second, radians,
    velocity::foot_per_second,
};

use crate::{TEAMS_COUNT, stone};
use gielo_sheet as sheet;
use gielo_sheet::Hack;
use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

pub const PER_TEAM_COUNT: usize = 4;
pub const STONES_PER_PLAYER: usize = stone::COUNT_PER_TEAM / PER_TEAM_COUNT;

pub type Id = usize;

pub fn who_is_delivering(delivered_stones: usize) -> Id {
    delivered_stones / TEAMS_COUNT / STONES_PER_PLAYER
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Skills {
    pub angle_std_dev: Angle,
    pub velocity_std_dev: Velocity,
}

impl Skills {
    pub fn from_tee_shot_std_dev(x: Length, y: Length, sheet: sheet::Parameters) -> Self {
        let tee_shot_y = sheet.geometry.playing_end.tee_line_y;
        let tee_shot_v = sheet.velocity_for_target_y(tee_shot_y);
        let tangent_dev = (x / (tee_shot_y - sheet.geometry.delivery_end.hack_line_y)).value;
        Self {
            angle_std_dev: radians(tangent_dev),
            velocity_std_dev: sheet.velocity_for_target_y(tee_shot_y + y) - tee_shot_v,
        }
    }
}

impl Skills {
    pub fn rand_angle_error(&self) -> Angle {
        let angle_dist = rand_distr::Normal::new(0.0, self.angle_std_dev.get::<degree>()).unwrap();
        degrees(angle_dist.sample(&mut rand::rng()))
    }

    pub fn rand_velocity_error(&self) -> Velocity {
        let velocity_dist =
            rand_distr::Normal::new(0.0, self.velocity_std_dev.get::<foot_per_second>()).unwrap();
        feet_per_second(velocity_dist.sample(&mut rand::rng()))
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Player<NameT> {
    pub name: NameT,
    pub used_hack: Hack,
    pub skills: Skills,
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
