use derive_more::*;
use slint::{Color, SharedString};
use std::array;
use std::ops::{Index, IndexMut};

pub const TEAMS_COUNT: usize = 2;
pub const TEAMS: PerTeam<Team> = PerTeam { a: Team::A, b: Team::B };

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Team {
    A,
    B,
}

impl Team {
    pub fn opponent(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Add, AddAssign, Eq, PartialEq, Sum)]
pub struct PerTeam<T> {
    pub a: T,
    pub b: T,
}

impl<T> PerTeam<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> PerTeam<U> {
        PerTeam { a: f(self.a), b: f(self.b) }
    }

    pub fn zip<U>(self, rhs: PerTeam<U>) -> PerTeam<(T, U)> {
        PerTeam { a: (self.a, rhs.a), b: (self.b, rhs.b) }
    }

    pub fn as_ref(&self) -> PerTeam<&T> {
        PerTeam { a: &self.a, b: &self.b }
    }
}

impl<T> Index<Team> for PerTeam<T> {
    type Output = T;

    fn index(&self, index: Team) -> &Self::Output {
        match index {
            Team::A => &self.a,
            Team::B => &self.b,
        }
    }
}

impl<T> IndexMut<Team> for PerTeam<T> {
    fn index_mut(&mut self, index: Team) -> &mut Self::Output {
        match index {
            Team::A => &mut self.a,
            Team::B => &mut self.b,
        }
    }
}

impl<T> IntoIterator for PerTeam<T> {
    type Item = T;
    type IntoIter = array::IntoIter<T, TEAMS_COUNT>;

    fn into_iter(self) -> Self::IntoIter {
        [self.a, self.b].into_iter()
    }
}

impl<T> From<(T, T)> for PerTeam<T> {
    fn from((a, b): (T, T)) -> Self {
        Self { a, b }
    }
}

impl<T, E> From<PerTeam<Result<T, E>>> for Result<PerTeam<T>, E> {
    fn from(from: PerTeam<Result<T, E>>) -> Self {
        Ok(PerTeam { a: from.a?, b: from.b? })
    }
}

pub fn teams() -> PerTeam<Team> {
    PerTeam { a: Team::A, b: Team::B }
}

#[derive(Clone, Debug)]
pub struct Info {
    pub name: SharedString,
    pub color: Color,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::Vector2;

    #[test]
    fn per_team_map() {
        let numbers = PerTeam { a: 4, b: 15 };
        let mapped = numbers.map(|x| Vector2 { x, y: 2 * x });
        assert_eq!(mapped.a, Vector2 { x: 4, y: 8 });
        assert_eq!(mapped.b, Vector2 { x: 15, y: 30 });
    }

    #[test]
    fn per_team_zip() {
        let numbers = PerTeam { a: 4, b: 15 };
        let strings = PerTeam { a: "a".to_owned(), b: "b".to_owned() };
        let zipped = numbers.zip(strings);
        assert_eq!(zipped.a, (4, "a".to_owned()));
        assert_eq!(zipped.b, (15, "b".to_owned()));
    }

    #[test]
    fn per_team_index() {
        let mut numbers_mut = PerTeam { a: 1, b: 2 };
        let numbers = &numbers_mut;
        assert_eq!(numbers[Team::A], 1);
        assert_eq!(numbers[Team::B], 2);
        numbers_mut[Team::A] = 3;
        numbers_mut[Team::B] = 4;
        assert_eq!(numbers_mut, PerTeam { a: 3, b: 4 });
    }
}
