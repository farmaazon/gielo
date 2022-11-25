use crate::game::dirty::Dirty;
use crate::game::sheet::Sheet;
use crate::game::stone::Rotation;
use crate::game::{sheet, stone};
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

    #[cfg(test)]
    pub(crate) fn tee_draw(sheet: &sheet::Parameters) -> Self {
        use crate::unit::{feet, seconds};
        Call {
            weight: seconds(3.0),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }
}

#[derive(Debug)]
pub struct Start<'a, 'b> {
    pub call: Call,
    pub sheet: &'a mut Sheet,
    pub dirty: &'b mut Dirty,
}

impl<'a, 'b> Start<'a, 'b> {
    pub fn resolve(self, stone: stone::Id) -> ResolvedStart<'a, 'b> {
        let hack = sheet::Hack::Left;
        ResolvedStart {
            stone,
            angle: self.call.angle(hack, &self.sheet.parameters),
            weight: self.call.weight,
            hack,
            rotation: self.call.rotation,
            sheet: self.sheet,
            dirty: self.dirty,
        }
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(dirty: &'b mut Dirty, sheet: &'a mut Sheet) -> Self {
        Self { call: Call::tee_draw(&sheet.parameters), sheet, dirty }
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
