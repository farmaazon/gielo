use crate::{stone, Hack};
use gielo_unit::{
    acceleration::foot_per_second_squared,
    base_type::consts::PI,
    feet, feet_per_second_squared, feet_squared_per_second_squared, float_eq, inches,
    length::foot,
    seconds,
    vector::{EuclideanNorm, Vector2},
    velocity::foot_per_second,
    Acceleration, Angle, AvailableEnergy, Length, Time, Velocity,
};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct EndGeometry {
    pub hack_line_y: Length,
    pub back_line_y: Length,
    pub tee_line_y: Length,
    pub hog_line_y: Length,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
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

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Parameters {
    pub geometry: Geometry,
    pub stone_radius: Length,
    pub friction: Acceleration,
    pub rotation_acc: Acceleration,
    pub static_friction: AvailableEnergy,
}

impl Parameters {
    pub fn with_tee_shot_parameters(self, hot_to_hog: Time, curl_offset: Length) -> Self {
        let t1 = hot_to_hog;
        let s1 = self.geometry.playing_end.hog_line_y - self.geometry.delivery_end.hog_line_y;
        let s2 = self.geometry.playing_end.tee_line_y - self.geometry.playing_end.hog_line_y;
        let friction = 2.0 * (s1 + 2.0 * s2 - 2.0 * (s2 * (s1 + s2)).sqrt()) / (t1 * t1);
        let t2 = (2.0 * s2 / friction).sqrt();
        let t = t1 + t2;
        let rotation_acc = 2.0 * (2.0 * curl_offset / t / t); // Why 2.0?
        Self { friction, rotation_acc, ..self }
    }

    pub fn angle_from_mark(&self, mark: Vector2<Length>, from: Hack) -> Angle {
        let offset = mark - self.geometry.hack_pos(from);
        (offset.x / offset.y).atan()
    }

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

    pub fn dist_from_tee(&self, position: stone::Position) -> Length {
        (position - self.geometry.tee()).norm()
    }

    pub fn dist_from_tee_in_house(&self, position: stone::Position) -> Option<Length> {
        let out_of_house = self.geometry.house_radius + self.stone_radius;
        let dist = self.dist_from_tee(position);
        (dist <= out_of_house || float_eq!(dist, out_of_house)).then_some(dist)
    }

    pub fn is_in_house(&self, position: stone::Position) -> bool {
        self.dist_from_tee_in_house(position).is_some()
    }

    pub fn is_guard(&self, position: stone::Position) -> bool {
        let before_tee_line =
            (position.y + self.stone_radius) < self.geometry.playing_end.tee_line_y;
        !self.is_in_house(position) && before_tee_line
    }

    pub fn is_center_guard(&self, position: stone::Position) -> bool {
        let dist_from_center_line = (position.x - self.geometry.center_line_x).abs();
        let touches_center_line = dist_from_center_line < self.stone_radius
            || float_eq!(dist_from_center_line, self.stone_radius);
        self.is_guard(position) && touches_center_line
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

#[cfg(test)]
mod tests {
    use super::*;
    use gielo_unit as unit;
    use gielo_unit::{assert_float_eq, degrees};

    #[test]
    fn compute_angle() {
        fn test_case((x, y): (unit::BaseType, unit::BaseType), expected: Angle) {
            let sheet = Parameters::default();
            for hack in [Hack::Left, Hack::Right] {
                let mark = sheet.geometry.hack_pos(hack) + Vector2 { x: feet(x), y: feet(y) };
                let result = sheet.angle_from_mark(mark, hack);
                assert_float_eq!(result, expected);
            }
        }

        test_case((-6.0, 6.0), degrees(-45.0));
        test_case((0.0, 6.0), degrees(0.0));
        test_case((6.0, 6.0), degrees(45.0));
    }
}
