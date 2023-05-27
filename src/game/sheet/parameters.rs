use crate::unit::{
    feet, feet_per_second_squared, feet_squared_per_second_squared, inches, seconds, Acceleration,
    AvailableEnergy, Length, Time, Velocity,
};
use crate::vector::Vector2;
use std::f32::consts::PI;
use uom::si::{acceleration::foot_per_second_squared, length::foot, velocity::foot_per_second};

use super::Hack;

#[derive(Copy, Clone, Debug)]
pub struct EndGeometry {
    pub hack_line_y: Length,
    pub back_line_y: Length,
    pub tee_line_y: Length,
    pub hog_line_y: Length,
}

#[derive(Copy, Clone, Debug)]
pub struct Geometry {
    pub width: Length,
    pub length: Length,
    pub center_line_x: Length,
    pub hack_x_offset: Length,
    pub house_radius: Length,
    pub delivery_end: EndGeometry,
    pub playing_end: EndGeometry,
}

impl Default for Geometry {
    fn default() -> Self {
        let length = feet(150.0);
        Self {
            width: feet(15.0) + inches(7.0),
            length,
            center_line_x: feet(0.0),
            hack_x_offset: inches(6.0),
            house_radius: feet(6.0),
            delivery_end: EndGeometry {
                hack_line_y: feet(6.0),
                back_line_y: feet(12.0),
                tee_line_y: feet(18.0),
                hog_line_y: feet(39.0),
            },
            playing_end: EndGeometry {
                hack_line_y: length - feet(6.0),
                back_line_y: length - feet(12.0),
                tee_line_y: length - feet(18.0),
                hog_line_y: length - feet(39.0),
            },
        }
    }
}

impl Geometry {
    pub fn hack_pos(&self, hack: Hack) -> Vector2<Length> {
        Vector2 {
            x: match hack {
                Hack::Left => self.center_line_x + self.hack_x_offset,
                Hack::Right => self.center_line_x - self.hack_x_offset,
            },
            y: self.delivery_end.hack_line_y,
        }
    }

    pub fn tee(&self) -> Vector2<Length> {
        Vector2 { x: self.center_line_x, y: self.playing_end.tee_line_y }
    }

    pub fn left_bound(&self) -> Length {
        self.center_line_x - self.width / 2.0
    }

    pub fn right_bound(&self) -> Length {
        self.center_line_x + self.width / 2.0
    }

    pub fn delivery_dist(&self) -> Length {
        self.delivery_end.hog_line_y - self.delivery_end.hack_line_y
    }

    pub fn hog_to_hog_dist(&self) -> Length {
        self.playing_end.hog_line_y - self.delivery_end.hog_line_y
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    pub geometry: Geometry,
    pub stone_radius: Length,
    pub friction: Acceleration,
    pub rotation_acc: Acceleration,
    pub static_friction: AvailableEnergy,
}

impl Parameters {
    pub fn velocity_for_target_y(&self, y: Length) -> Velocity {
        let s = y - self.geometry.delivery_end.hog_line_y;
        (2.0 * self.friction * s).sqrt()
    }

    pub fn velocity_for_hog_to_hog_time(&self, t: Time) -> Velocity {
        self.geometry.hog_to_hog_dist() / t + self.friction * t / 2.0
    }

    pub fn hot_to_hog_time_from_velocity(&self, velocity: Velocity) -> Option<Time> {
        let a2 = -self.friction.get::<foot_per_second_squared>() / 2.0;
        let a1 = velocity.get::<foot_per_second>();
        let a0 = -self.geometry.hog_to_hog_dist().get::<foot>();
        let roots = roots::find_roots_quadratic(a2, a1, a0);
        roots.as_ref().iter().next().copied().map(seconds)
    }
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            geometry: Geometry::default(),
            friction: feet_per_second_squared(0.24),
            rotation_acc: feet_per_second_squared(0.025),
            stone_radius: inches(18.0 / PI),
            static_friction: feet_squared_per_second_squared(0.25),
        }
    }
}
