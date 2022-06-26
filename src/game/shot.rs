use crate::game::sheet;
use crate::game::stone::Rotation;
use crate::unit::{Angle, Length, Time};
use crate::vector::Vector2;

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
