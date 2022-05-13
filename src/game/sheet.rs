use crate::game::Stone;
use crate::unit::{feet, feet_per_second_squared, inches, Acceleration, Length};
use crate::vector::Vector2;
use lazy_static::lazy_static;
use local_vec::LocalVec;
use std::f32::consts::PI;

#[derive(Copy, Clone, Debug)]
pub enum Hack {
    Left,
    Right,
}

pub const STONE_COUNT: usize = 16;

lazy_static! {
    pub static ref LENGTH: Length = feet(150.0);
    pub static ref CENTER_LINE_X: Length = feet(0.0);
    pub static ref HACK_X_OFFSET: Length = inches(6.0);
}

pub mod delivery_end {
    use super::*;

    lazy_static! {
        pub static ref BOARD_LINE_Y: Length = feet(0.0);
        pub static ref HACK_LINE_Y: Length = feet(6.0);
        pub static ref BACK_LINE_Y: Length = *HACK_LINE_Y + feet(6.0);
        pub static ref TEE_LINE_Y: Length = *BACK_LINE_Y + feet(6.0);
        pub static ref HOG_LINE_Y: Length = *TEE_LINE_Y + feet(21.0);
    }
}

pub mod playing_end {
    use super::*;

    lazy_static! {
        pub static ref BOARD_LINE_Y: Length = *LENGTH;
        pub static ref HACK_LINE_Y: Length = *LENGTH - feet(6.0);
        pub static ref BACK_LINE_Y: Length = *HACK_LINE_Y - feet(6.0);
        pub static ref TEE_LINE_Y: Length = *BACK_LINE_Y - feet(6.0);
        pub static ref HOG_LINE_Y: Length = *TEE_LINE_Y - feet(21.0);
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
            friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
            curl_factor: feet_per_second_squared(245.0 / 8649.0),
            stone_radius: inches(18.0 / PI),
            width: feet(15.0) + inches(7.0),
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
