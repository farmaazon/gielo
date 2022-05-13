use crate::game::Stone;
use crate::unit::{Acceleration, Length};
use crate::vector::Vector2;
use lazy_static::lazy_static;
use local_vec::LocalVec;
use std::f32::consts::PI;
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::{foot, inch};

#[derive(Copy, Clone, Debug)]
pub enum Hack {
    Left,
    Right,
}

pub const STONE_COUNT: usize = 16;

lazy_static! {
    pub static ref LENGTH: Length = Length::new::<foot>(150.0);
    pub static ref CENTER_LINE_X: Length = Length::new::<foot>(0.0);
    pub static ref HACK_X_OFFSET: Length = Length::new::<inch>(6.0);
}

pub mod delivery_end {
    use super::*;

    lazy_static! {
        pub static ref BOARD_LINE_Y: Length = Length::new::<foot>(0.0);
        pub static ref HACK_LINE_Y: Length = Length::new::<foot>(6.0);
        pub static ref BACK_LINE_Y: Length = *HACK_LINE_Y + Length::new::<foot>(6.0);
        pub static ref TEE_LINE_Y: Length = *BACK_LINE_Y + Length::new::<foot>(6.0);
        pub static ref HOG_LINE_Y: Length = *TEE_LINE_Y + Length::new::<foot>(21.0);
    }
}

pub mod playing_end {
    use super::*;

    lazy_static! {
        pub static ref BOARD_LINE_Y: Length = *LENGTH;
        pub static ref HACK_LINE_Y: Length = *LENGTH - Length::new::<foot>(6.0);
        pub static ref BACK_LINE_Y: Length = *HACK_LINE_Y - Length::new::<foot>(6.0);
        pub static ref TEE_LINE_Y: Length = *BACK_LINE_Y - Length::new::<foot>(6.0);
        pub static ref HOG_LINE_Y: Length = *TEE_LINE_Y - Length::new::<foot>(21.0);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    pub friction: Acceleration,
    pub curl_factor: Acceleration,
    pub stone_radius: Length,
    pub width: Length,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            friction: Acceleration::new::<foot_per_second_squared>(49.0 / 93.0 / 2.0),
            curl_factor: Acceleration::new::<foot_per_second_squared>(245.0 / 8649.0),
            stone_radius: Length::new::<inch>(18.0 / PI),
            width: Length::new::<foot>(15.0) + Length::new::<inch>(7.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub stones: LocalVec<Stone, STONE_COUNT>,
    pub parameters: Parameters,
}

impl Sheet {
    pub fn new(parameters: Parameters) -> Self {
        Self {
            stones: LocalVec::new(),
            parameters,
        }
    }

    pub fn left_bound(&self) -> Length {
        *CENTER_LINE_X - self.parameters.width / 2.0
    }

    pub fn right_bound(&self) -> Length {
        *CENTER_LINE_X + self.parameters.width / 2.0
    }

    pub fn hack_pos(&self, hack: Hack) -> Vector2<Length> {
        Vector2 {
            x: match hack {
                Hack::Left => *CENTER_LINE_X - *HACK_X_OFFSET,
                Hack::Right => *CENTER_LINE_X + *HACK_X_OFFSET,
            },
            y: *delivery_end::HACK_LINE_Y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions() {
        let _params = Parameters::default();
    }
}
