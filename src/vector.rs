use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Vector2<T> {
    pub x: T,
    pub y: T,
}

impl<T> Neg for Vector2<T>
where
    T: Neg,
{
    type Output = Vector2<T::Output>;

    fn neg(self) -> Self::Output {
        Vector2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl<T, U> Add<Vector2<U>> for Vector2<T>
where
    T: Add<U>,
{
    type Output = Vector2<T::Output>;

    fn add(self, rhs: Vector2<U>) -> Self::Output {
        Vector2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
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
        Vector2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
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
        Vector2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
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
        Vector2 {
            x: self.x / rhs,
            y: self.y / rhs,
        }
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

pub trait EuclideanNorm {
    type Output;

    fn norm(self) -> Self::Output;
}

impl EuclideanNorm for crate::game::stone::Velocity {
    type Output = crate::unit::Velocity;

    fn norm(self) -> Self::Output {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}
