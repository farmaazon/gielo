use crate::{
    game::{sheet, sheet::Hack, stone, team::TEAMS_COUNT, turn},
    unit,
    unit::{degrees, feet_per_second, radians},
};
use rand::distributions::Distribution;
use uom::si::{angle::degree, velocity::foot_per_second};

pub const PER_TEAM_COUNT: usize = 4;
pub const STONES_PER_PLAYER: usize = stone::COUNT_PER_TEAM / PER_TEAM_COUNT;

pub type Id = usize;

pub fn who_is_delivering(turn: turn::Index) -> Id {
    turn / TEAMS_COUNT / STONES_PER_PLAYER
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Skills {
    pub angle_std_dev: unit::Angle,
    pub velocity_std_dev: unit::Velocity,
}

impl Skills {
    pub fn from_tee_shot_std_dev(
        x: unit::Length,
        y: unit::Length,
        sheet: sheet::Parameters,
    ) -> Self {
        let tee_shot_y = sheet.geometry.playing_end.tee_line_y;
        let tee_shot_v = sheet.velocity_for_target_y(tee_shot_y);
        let tangent_dev = (x / (tee_shot_y - sheet.geometry.delivery_end.hack_line_y)).value;
        Self {
            angle_std_dev: radians(tangent_dev),
            velocity_std_dev: sheet.velocity_for_target_y(tee_shot_y + y) - tee_shot_v,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Player {
    pub used_hack: Hack,
    pub skills: Skills,
}

impl Default for Player {
    fn default() -> Self {
        Self { used_hack: Hack::Left, skills: Skills::default() }
    }
}

impl Player {
    pub fn rand_angle_error(&self) -> unit::Angle {
        let angle_dist =
            rand_distr::Normal::new(0.0, self.skills.angle_std_dev.get::<degree>()).unwrap();
        degrees(angle_dist.sample(&mut rand::thread_rng()))
    }

    pub fn rand_velocity_error(&self) -> unit::Velocity {
        let velocity_dist =
            rand_distr::Normal::new(0.0, self.skills.velocity_std_dev.get::<foot_per_second>())
                .unwrap();
        feet_per_second(velocity_dist.sample(&mut rand::thread_rng()))
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
