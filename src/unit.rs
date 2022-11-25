use uom::system;

pub type BaseType = f32;
ISQ!(uom::si, BaseType, (foot, kilogram, second, ampere, kelvin, mole, candela));

pub mod new_unit {
    uom::unit! {
        system: uom::si;
        quantity: uom::si::available_energy;

        @foot_squared_per_second_squared: 0.09290304; "ft ^ 2 * s ^ -2", "foot squared per second squared", "feet squared per second squared";
    }
}

pub fn feet(value: BaseType) -> Length {
    Length::new::<uom::si::length::foot>(value)
}

pub fn inches(value: BaseType) -> Length {
    Length::new::<uom::si::length::inch>(value)
}
pub fn seconds(value: BaseType) -> Time {
    Time::new::<uom::si::time::second>(value)
}
pub fn milliseconds(value: BaseType) -> Time {
    Time::new::<uom::si::time::millisecond>(value)
}
pub fn feet_per_second(value: BaseType) -> Velocity {
    Velocity::new::<uom::si::velocity::foot_per_second>(value)
}
pub fn feet_per_second_squared(value: BaseType) -> Acceleration {
    Acceleration::new::<uom::si::acceleration::foot_per_second_squared>(value)
}
pub fn radians(value: BaseType) -> Angle {
    Angle::new::<uom::si::angle::radian>(value)
}
pub fn degrees(value: BaseType) -> Angle {
    Angle::new::<uom::si::angle::degree>(value)
}
pub fn joules_per_kilogram(value: BaseType) -> AvailableEnergy {
    AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(value)
}
pub fn feet_squared_per_second_squared(value: BaseType) -> AvailableEnergy {
    AvailableEnergy::new::<new_unit::foot_squared_per_second_squared>(value)
}

#[allow(unused_macros)]
macro_rules! approx_eq {
    ($rhs:expr, $lhs:expr$(, $($argv:tt)*)?) => {
        float_cmp::approx_eq!($crate::unit::BaseType, $rhs.value, $lhs.value$(, $($argv)*)?)
    }
}

#[allow(unused_macros)]
macro_rules! assert_approx_eq {
    ($rhs:expr, $lhs:expr$(, $($argv:tt)*)?) => {
        float_cmp::assert_approx_eq!($crate::unit::BaseType, $rhs.value, $lhs.value$(, $($argv)*)?)
    }
}

#[allow(unused_imports)]
pub(crate) use approx_eq;
#[allow(unused_imports)]
pub(crate) use assert_approx_eq;
