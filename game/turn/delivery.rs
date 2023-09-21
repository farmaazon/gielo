use crate::{
    player,
    player::Player,
    sheet::{stone, stone::Rotation, team::PerTeam, Sheet},
    simulation::delivery::StartingConditions,
    unit::{vector::Vector2, Angle, ConstZero, Length, Velocity},
    TeamInfo,
};

#[derive(Copy, Clone, Debug)]
pub struct Call {
    pub weight: Velocity,
    pub mark: Vector2<Length>,
    pub rotation: Rotation,
}

impl Call {
    #[cfg(test)]
    pub(crate) fn tee_draw(sheet: &crate::sheet::Parameters) -> Self {
        use crate::unit::feet;
        Call {
            weight: sheet.velocity_for_target_y(sheet.geometry.playing_end.tee_line_y),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }
}

#[derive(Debug)]
pub struct Start<'a, 'b> {
    pub call: Call,
    pub sheet: &'a mut Sheet,
    pub teams: &'b PerTeam<TeamInfo>,
}

impl<'a, 'b> Start<'a, 'b> {
    pub fn resolve(&self, stone: stone::Id, player: player::Id) -> StartingConditions {
        self.resolve_template(stone, player, Player::rand_angle_error, Player::rand_velocity_error)
    }

    pub fn resolve_ideal(&self, stone: stone::Id, player: player::Id) -> StartingConditions {
        self.resolve_template(stone, player, |_| Angle::ZERO, |_| Velocity::ZERO)
    }

    pub fn resolve_template(
        &self,
        stone: stone::Id,
        player: player::Id,
        angle_error: impl FnOnce(&Player) -> Angle,
        velocity_error: impl FnOnce(&Player) -> Velocity,
    ) -> StartingConditions {
        let team = stone::team(stone);
        let player_data = &self.teams[team].players[player];
        let hack = player_data.used_hack;
        let called_angle = self.sheet.parameters.angle_from_mark(self.call.mark, hack);
        StartingConditions {
            stone,
            angle: called_angle + angle_error(player_data),
            velocity: self.call.weight + velocity_error(player_data),
            hack,
            rotation: self.call.rotation,
        }
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(sheet: &'a mut Sheet, teams: &'b PerTeam<TeamInfo>) -> Self {
        Self { call: Call::tee_draw(&sheet.parameters), sheet, teams }
    }
}
