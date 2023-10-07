pub use gielo_sheet as sheet;
pub use gielo_simulation as simulation;
pub use gielo_team as team;
pub use gielo_unit as unit;

use gielo_sheet::{
    stone::{Position, Rotation},
    Hack,
};
use gielo_team::player;
use gielo_unit::{feet, vector::Vector2, Angle, Velocity};
use serde::{Deserialize, Serialize};

pub mod dirty;
pub mod game;
pub mod running;
pub mod score;
pub mod setup;
pub mod situation;

pub struct MarkedDelivery {
    pub mark: Position,
    pub velocity: Velocity,
    pub rotation: Rotation,
}

impl MarkedDelivery {
    pub fn to_delivery(self, sheet: &sheet::Parameters, hack: Hack) -> Delivery {
        Delivery {
            angle: sheet.angle_from_mark(self.mark, hack),
            velocity: self.velocity,
            rotation: self.rotation,
        }
    }

    pub fn tee_draw(sheet: &sheet::Parameters) -> Self {
        Self {
            velocity: sheet.velocity_for_target_y(sheet.geometry.playing_end.tee_line_y),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Delivery {
    pub angle: Angle,
    pub velocity: Velocity,
    pub rotation: Rotation,
}

impl Delivery {
    pub fn apply_error(mut self, player_skills: &player::Skills) -> Self {
        self.angle += player_skills.rand_angle_error();
        self.velocity += player_skills.rand_velocity_error();
        self
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum ViolatedRule {
    FreeGuardRule,
    NoTickRule,
}

pub type Game = game::Game<(), ()>;
pub type Setup = setup::Setup<(), ()>;
pub type RunningGame = running::RunningGame<(), ()>;

#[cfg(feature = "slint")]
pub mod slint {
    use crate::{game, running, setup};
    use slint::{Color, SharedString};

    pub type Game = game::Game<SharedString, Color>;
    pub type Setup = setup::Setup<SharedString, Color>;
    pub type RunningGame = running::RunningGame<SharedString, Color>;
}
