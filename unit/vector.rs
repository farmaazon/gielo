use crate::{Length, Velocity};
use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Vector2<T> {
    pub x: T,
    pub y: T,
}

impl<T> Vector2<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Vector2<U> {
        Vector2 { x: f(self.x), y: f(self.y) }
    }

    pub fn dot<U>(self, rhs: Vector2<U>) -> <T::Output as Add<T::Output>>::Output
    where
        T: Mul<U>,
        T::Output: Add<T::Output>,
    {
        self.x * rhs.x + self.y * rhs.y
    }
}

impl<T> From<(T, T)> for Vector2<T> {
    fn from((x, y): (T, T)) -> Self {
        Self { x, y }
    }
}

impl<T> Neg for Vector2<T>
where
    T: Neg,
{
    type Output = Vector2<T::Output>;

    fn neg(self) -> Self::Output {
        Vector2 { x: -self.x, y: -self.y }
    }
}

impl<T, U> Add<Vector2<U>> for Vector2<T>
where
    T: Add<U>,
{
    type Output = Vector2<T::Output>;

    fn add(self, rhs: Vector2<U>) -> Self::Output {
        Vector2 { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

impl<T, U> AddAssign<Vector2<U>> for Vector2<T>
where
    T: AddAssign<U>,
{
    fn add_assign(&mut self, rhs: Vector2<U>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T, U> Sub<Vector2<U>> for Vector2<T>
where
    T: Sub<U>,
{
    type Output = Vector2<T::Output>;

    fn sub(self, rhs: Vector2<U>) -> Self::Output {
        Vector2 { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}

impl<T, U> SubAssign<Vector2<U>> for Vector2<T>
where
    T: SubAssign<U>,
{
    fn sub_assign(&mut self, rhs: Vector2<U>) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T, U> Mul<U> for Vector2<T>
where
    T: Mul<U>,
    U: Copy,
{
    type Output = Vector2<T::Output>;

    fn mul(self, rhs: U) -> Self::Output {
        Vector2 { x: self.x * rhs, y: self.y * rhs }
    }
}

impl<T, U> MulAssign<U> for Vector2<T>
where
    T: MulAssign<U>,
    U: Copy,
{
    fn mul_assign(&mut self, rhs: U) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl<T, U> Div<U> for Vector2<T>
where
    T: Div<U>,
    U: Copy,
{
    type Output = Vector2<T::Output>;

    fn div(self, rhs: U) -> Self::Output {
        Vector2 { x: self.x / rhs, y: self.y / rhs }
    }
}

impl<T, U> DivAssign<U> for Vector2<T>
where
    T: DivAssign<U>,
    U: Copy,
{
    fn div_assign(&mut self, rhs: U) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl<T: uom::ConstZero> uom::ConstZero for Vector2<T> {
    const ZERO: Self = Self { x: T::ZERO, y: T::ZERO };
}

pub trait EuclideanNorm {
    type Output;

    fn norm(self) -> Self::Output;
}

impl EuclideanNorm for Vector2<Velocity> {
    type Output = Velocity;

    fn norm(self) -> Self::Output {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl EuclideanNorm for Vector2<Length> {
    type Output = Length;

    fn norm(self) -> Self::Output {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_float_eq, feet_per_second, BaseType};

    #[test]
    fn operations() {
        let mut lhs = Vector2 { x: 3.0, y: 4.0 };
        let rhs = Vector2 { x: 2.0, y: 8.0 };
        assert_eq!(lhs + rhs, Vector2 { x: 5.0, y: 12.0 });
        assert_eq!(lhs - rhs, Vector2 { x: 1.0, y: -4.0 });
        assert_eq!(lhs * 3.0, Vector2 { x: 9.0, y: 12.0 });
        assert_eq!(lhs / 2.0, Vector2 { x: 1.5, y: 2.0 });
        lhs += rhs;
        assert_eq!(lhs, Vector2 { x: 5.0, y: 12.0 });
        lhs -= rhs;
        assert_eq!(lhs, Vector2 { x: 3.0, y: 4.0 });
        lhs *= 3.0;
        assert_eq!(lhs, Vector2 { x: 9.0, y: 12.0 });
        lhs /= 2.0;
        assert_eq!(lhs, Vector2 { x: 4.5, y: 6.0 });
    }

    #[test]
    fn calculating_norm() {
        let cases = [(3.0, 4.0, 5.0), (1.0, 30.0, (901.0 as BaseType).sqrt())];
        for (x, y, expected) in cases {
            let v = Vector2 { x: feet_per_second(x), y: feet_per_second(y) };
            assert_float_eq!(v.norm(), feet_per_second(expected));
        }
    }
}
