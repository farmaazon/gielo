use uom::system;
use uom::ISQ;

pub type BaseType = f32;

ISQ!(
    uom::si,
    BaseType,
    (foot, kilogram, second, ampere, kelvin, mole, candela)
);

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

// macro_rules! approx_eq {
//     ($rhs:expr, $lhs:expr$(, $argv:tt)*) => {
//         float_cmp::approx_eq!($crate::unit::BaseType, $rhs.value, $lhs.value$(, $argv)*)
//     }
// }

macro_rules! assert_approx_eq {
    ($rhs:expr, $lhs:expr$(, $argv:tt)*) => {
        float_cmp::assert_approx_eq!($crate::unit::BaseType, $rhs.value, $lhs.value$(, $argv)*)
    }
}

// pub(crate) use approx_eq;
pub(crate) use assert_approx_eq;
