#[macro_use]
extern crate uom;

pub use ::float_eq as base_float_eq;
pub use std::f64 as base_type;
pub use uom::{
    ConstZero,
    si::{acceleration, angle, length, time, velocity},
};

pub mod vector;

pub type BaseType = f64;

pub const EPSILON: BaseType = 1e-8;

ISQ!(uom::si, BaseType, (foot, kilogram, second, ampere, kelvin, mole, candela));

pub mod available_energy {
    pub use uom::si::available_energy::joule_per_kilogram;

    unit! {
        system: uom::si;
        quantity: uom::si::available_energy;

        @foot_squared_per_second_squared: 0.09290304; "ft ^ 2 * s ^ -2", "foot squared per second squared", "feet squared per second squared";
    }
}

pub fn feet(value: impl Into<BaseType>) -> Length {
    Length::new::<length::foot>(value.into())
}

pub fn inches(value: impl Into<BaseType>) -> Length {
    Length::new::<length::inch>(value.into())
}
pub fn seconds(value: impl Into<BaseType>) -> Time {
    Time::new::<time::second>(value.into())
}
pub fn milliseconds(value: impl Into<BaseType>) -> Time {
    Time::new::<time::millisecond>(value.into())
}
pub fn feet_per_second(value: impl Into<BaseType>) -> Velocity {
    Velocity::new::<velocity::foot_per_second>(value.into())
}
pub fn feet_per_second_squared(value: impl Into<BaseType>) -> Acceleration {
    Acceleration::new::<acceleration::foot_per_second_squared>(value.into())
}
pub fn radians(value: impl Into<BaseType>) -> Angle {
    Angle::new::<angle::radian>(value.into())
}
pub fn degrees(value: impl Into<BaseType>) -> Angle {
    Angle::new::<angle::degree>(value.into())
}
pub fn joules_per_kilogram(value: impl Into<BaseType>) -> AvailableEnergy {
    AvailableEnergy::new::<available_energy::joule_per_kilogram>(value.into())
}
pub fn feet_squared_per_second_squared(value: impl Into<BaseType>) -> AvailableEnergy {
    AvailableEnergy::new::<available_energy::foot_squared_per_second_squared>(value.into())
}

#[macro_export]
macro_rules! float_eq {
    ($rhs:expr, $lhs:expr, $($argv:tt)*) => {
        $crate::base_float_eq::float_eq!($rhs.value, $lhs.value$(, $($argv)*)?)
    };
    ($rhs:expr, $lhs:expr) => {
        $crate::base_float_eq::float_eq!($rhs.value, $lhs.value, abs <= $crate::EPSILON)
    }
}

#[macro_export]
macro_rules! assert_float_eq {
    ($rhs:expr, $lhs:expr, $($argv:tt)*) => {
        $crate::base_float_eq::assert_float_eq!($rhs.value, $lhs.value, $($argv)*)
    };
    ($rhs:expr, $lhs:expr) => {
        $crate::base_float_eq::assert_float_eq!($rhs.value, $lhs.value, abs <= $crate::EPSILON)
    }
}
