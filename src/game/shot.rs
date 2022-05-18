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
    pub fn angle(&self, from: sheet::Hack) -> Angle {
        let offset = self.mark - sheet::hack_pos(from);
        (offset.x / offset.y).atan()
    }
}
