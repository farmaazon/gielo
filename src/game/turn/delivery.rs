use crate::game::dirty::Dirty;
use crate::game::sheet::Sheet;
use crate::game::stone::Rotation;
use crate::game::team::{player, PerTeam};
use crate::game::{sheet, stone, team};
use crate::unit::{degrees, seconds, Angle, Length, Time};
use crate::vector::Vector2;
use rand_distr::Distribution;
use uom::si::angle::degree;
use uom::si::time::second;

#[derive(Copy, Clone, Debug)]
pub struct Call {
    pub weight: Time,
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
            weight: seconds(3.0),
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
        let hack = sheet::Hack::Left;
        let team = stone::team(stone);
        let player_data = &self.teams[team].players[player];
        let angle_dist =
            rand_distr::Normal::new(0.0, player_data.angle_std_dev.get::<degree>()).unwrap();
        let angle_error = degrees(angle_dist.sample(&mut rand::thread_rng()));
        let weight_dist =
            rand_distr::Normal::new(0.0, player_data.weight_std_dev.get::<second>()).unwrap();
        let weight_err = seconds(weight_dist.sample(&mut rand::thread_rng()));
        ResolvedStart {
            stone,
            angle: self.call.angle(hack, &self.sheet.parameters) + angle_error,
            weight: self.call.weight + weight_err,
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

pub struct ResolvedStart<'a, 'b> {
    pub stone: stone::Id,
    pub angle: Angle,
    pub weight: Time,
    pub hack: sheet::Hack,
    pub rotation: Rotation,
    pub sheet: &'a mut Sheet,
    pub dirty: &'b mut Dirty,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet::Hack;
    use crate::unit::{assert_approx_eq, degrees, feet};

    #[test]
    fn compute_angle() {
        fn test_case((x, y): (f32, f32), expected: Angle) {
            let sheet = sheet::Parameters::default();
            for hack in [Hack::Left, Hack::Right] {
                let mark = sheet.geometry.hack_pos(hack) + Vector2 { x: feet(x), y: feet(y) };
                let call = Call { mark, weight: Time::default(), rotation: Rotation::Clockwise };
                assert_approx_eq!(call.angle(hack, &sheet), expected);
            }
        }

        test_case((-6.0, 6.0), degrees(-45.0));
        test_case((0.0, 6.0), degrees(0.0));
        test_case((6.0, 6.0), degrees(45.0));
    }
}
