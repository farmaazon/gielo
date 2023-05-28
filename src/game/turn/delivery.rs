use crate::{
    game::{
        dirty::Dirty,
        sheet,
        sheet::Sheet,
        stone,
        stone::Rotation,
        team,
        team::{player, PerTeam, Player},
    },
    unit::{Angle, Length, Velocity},
    vector::Vector2,
};
use uom::ConstZero;

#[derive(Copy, Clone, Debug)]
pub struct Call {
    pub weight: Velocity,
    pub mark: Vector2<Length>,
    pub rotation: Rotation,
}

impl Call {
    pub fn angle(&self, from: sheet::Hack, sheet: &sheet::Parameters) -> Angle {
        let offset = self.mark - sheet.geometry.hack_pos(from);
        (offset.x / offset.y).atan()
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(sheet: &sheet::Parameters) -> Self {
        use crate::unit::feet;
        Call {
            weight: sheet.velocity_for_target_y(sheet.geometry.playing_end.tee_line_y),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }
}

#[derive(Debug)]
pub struct Start<'a, 'b, 'c> {
    pub call: Call,
    pub sheet: &'a mut Sheet,
    pub teams: &'b PerTeam<team::Info>,
    pub dirty: &'c mut Dirty,
}

impl<'a, 'b, 'c> Start<'a, 'b, 'c> {
    pub fn resolve(self, stone: stone::Id, player: player::Id) -> ResolvedStart<'a, 'c> {
        self.resolve_template(stone, player, Player::rand_angle_error, Player::rand_velocity_error)
    }

    pub fn resolve_ideal(self, stone: stone::Id, player: player::Id) -> ResolvedStart<'a, 'c> {
        self.resolve_template(stone, player, |_| Angle::ZERO, |_| Velocity::ZERO)
    }

    pub fn resolve_template(
        self,
        stone: stone::Id,
        player: player::Id,
        angle_error: impl FnOnce(&Player) -> Angle,
        velocity_error: impl FnOnce(&Player) -> Velocity,
    ) -> ResolvedStart<'a, 'c> {
        let team = stone::team(stone);
        let player_data = &self.teams[team].players[player];
        let hack = player_data.used_hack;
        ResolvedStart {
            stone,
            angle: self.call.angle(hack, &self.sheet.parameters) + angle_error(player_data),
            velocity: self.call.weight + velocity_error(player_data),
            hack,
            rotation: self.call.rotation,
            sheet: self.sheet,
            dirty: self.dirty,
        }
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(
        dirty: &'c mut Dirty,
        sheet: &'a mut Sheet,
        teams: &'b PerTeam<team::Info>,
    ) -> Self {
        Self { call: Call::tee_draw(&sheet.parameters), sheet, teams, dirty }
    }
}

#[derive(Debug)]
pub struct ResolvedStart<'a, 'b> {
    pub stone: stone::Id,
    pub angle: Angle,
    pub velocity: Velocity,
    pub hack: sheet::Hack,
    pub rotation: Rotation,
    pub sheet: &'a mut Sheet,
    pub dirty: &'b mut Dirty,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::sheet::Hack,
        unit,
        unit::{assert_float_eq, degrees, feet},
    };

    #[test]
    fn compute_angle() {
        fn test_case((x, y): (unit::BaseType, unit::BaseType), expected: Angle) {
            let sheet = sheet::Parameters::default();
            for hack in [Hack::Left, Hack::Right] {
                let mark = sheet.geometry.hack_pos(hack) + Vector2 { x: feet(x), y: feet(y) };
                let call =
                    Call { mark, weight: Velocity::default(), rotation: Rotation::Clockwise };
                assert_float_eq!(call.angle(hack, &sheet), expected);
            }
        }

        test_case((-6.0, 6.0), degrees(-45.0));
        test_case((0.0, 6.0), degrees(0.0));
        test_case((6.0, 6.0), degrees(45.0));
    }
}
