use uom::system;

pub use std::f64 as base_type;

pub type BaseType = f64;

pub const EPSILON: BaseType = 1e-8;

ISQ!(uom::si, BaseType, (foot, kilogram, second, ampere, kelvin, mole, candela));

pub mod new_unit {
    uom::unit! {
        system: uom::si;
        quantity: uom::si::available_energy;

        @foot_squared_per_second_squared: 0.09290304; "ft ^ 2 * s ^ -2", "foot squared per second squared", "feet squared per second squared";
    }
}

pub fn feet(value: impl Into<BaseType>) -> Length {
    Length::new::<uom::si::length::foot>(value.into())
}

pub fn inches(value: impl Into<BaseType>) -> Length {
    Length::new::<uom::si::length::inch>(value.into())
}
pub fn seconds(value: impl Into<BaseType>) -> Time {
    Time::new::<uom::si::time::second>(value.into())
}
pub fn milliseconds(value: impl Into<BaseType>) -> Time {
    Time::new::<uom::si::time::millisecond>(value.into())
}
pub fn feet_per_second(value: impl Into<BaseType>) -> Velocity {
    Velocity::new::<uom::si::velocity::foot_per_second>(value.into())
}
pub fn feet_per_second_squared(value: impl Into<BaseType>) -> Acceleration {
    Acceleration::new::<uom::si::acceleration::foot_per_second_squared>(value.into())
}
pub fn radians(value: impl Into<BaseType>) -> Angle {
    Angle::new::<uom::si::angle::radian>(value.into())
}
pub fn degrees(value: impl Into<BaseType>) -> Angle {
    Angle::new::<uom::si::angle::degree>(value.into())
}
pub fn joules_per_kilogram(value: impl Into<BaseType>) -> AvailableEnergy {
    AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(value.into())
}
pub fn feet_squared_per_second_squared(value: impl Into<BaseType>) -> AvailableEnergy {
    AvailableEnergy::new::<new_unit::foot_squared_per_second_squared>(value.into())
}

#[allow(unused_macros)]
macro_rules! float_eq {
    ($rhs:expr, $lhs:expr, $($argv:tt)*) => {
        float_eq::float_eq!($rhs.value, $lhs.value$(, $($argv)*)?)
    };
    ($rhs:expr, $lhs:expr) => {
        float_eq::float_eq!($rhs.value, $lhs.value, abs <= $crate::unit::EPSILON)
    }
}

#[allow(unused_macros)]
macro_rules! assert_float_eq {
    ($rhs:expr, $lhs:expr, $($argv:tt)*) => {
        float_eq::assert_float_eq!($rhs.value, $lhs.value, $($argv)*)
    };
    ($rhs:expr, $lhs:expr) => {
        float_eq::assert_float_eq!($rhs.value, $lhs.value, abs <= $crate::unit::EPSILON)
    }
}

#[allow(unused_imports)]
pub(crate) use assert_float_eq;
#[allow(unused_imports)]
pub(crate) use float_eq;
