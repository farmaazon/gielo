use crate::game::team::{teams, PerTeam, Team};
use crate::game::{team, Stone};
use crate::unit::approx_eq;
use crate::unit::{feet, feet_per_second_squared, inches, seconds, Acceleration, Length};
use crate::vector::{EuclideanNorm, Vector2};
use decorum::NotNan;
use derive_more::{AsRef, Deref};
use itertools::Itertools;
use lazy_static::lazy_static;
use local_vec::LocalVec;
use std::f32::consts::PI;
use std::ops::{Index, IndexMut};
use uom::si::length::foot;

#[derive(Copy, Clone, Debug)]
pub enum Hack {
    Left,
    Right,
}

pub const STONES_PER_TEAM: usize = 8;
pub const STONE_COUNT: usize = STONES_PER_TEAM * team::TEAMS_COUNT;

lazy_static! {
    pub static ref LENGTH: Length = feet(150.0);
    pub static ref CENTER_LINE_X: Length = feet(0.0);
    pub static ref HACK_X_OFFSET: Length = inches(6.0);
    pub static ref TEE: Vector2<Length> =
        Vector2 { x: *CENTER_LINE_X, y: *playing_end::TEE_LINE_Y };
    pub static ref HOUSE_RADIUS: Length = feet(6.0);
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

pub fn hack_pos(hack: Hack) -> Vector2<Length> {
    Vector2 {
        x: match hack {
            Hack::Left => *CENTER_LINE_X + *HACK_X_OFFSET,
            Hack::Right => *CENTER_LINE_X - *HACK_X_OFFSET,
        },
        y: *delivery_end::HACK_LINE_Y,
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Parameters {
    pub friction: Acceleration,
    pub rotation_acc: Acceleration,
    pub stone_radius: Length,
    pub width: Length,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
            rotation_acc: feet_per_second_squared(245.0 / 8649.0),
            stone_radius: inches(18.0 / PI),
            width: feet(15.0) + inches(7.0),
        }
    }
}

impl Parameters {
    pub fn left_bound(&self) -> Length {
        *CENTER_LINE_X - self.width / 2.0
    }

    pub fn right_bound(&self) -> Length {
        *CENTER_LINE_X + self.width / 2.0
    }
}

#[derive(Clone, Debug, Default, AsRef, Deref)]
pub struct Stones {
    #[deref]
    stones: LocalVec<Stone, STONE_COUNT>,
    stones_count_change: isize,
}

impl Stones {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Stone> {
        self.stones.iter_mut()
    }

    pub fn push(&mut self, new_stone: Stone) {
        self.stones.push(new_stone);
        self.stones_count_change += 1;
    }

    pub fn clear(&mut self) {
        self.stones_count_change -= self.stones.len() as isize;
        self.stones.clear();
    }

    pub fn read_len(&mut self) -> (isize, usize) {
        (std::mem::take(&mut self.stones_count_change), self.stones.len())
    }
}

impl<T> Index<T> for Stones
where
    LocalVec<Stone, STONE_COUNT>: Index<T>,
{
    type Output = <LocalVec<Stone, STONE_COUNT> as Index<T>>::Output;

    fn index(&self, index: T) -> &Self::Output {
        &self.stones[index]
    }
}

impl<T> IndexMut<T> for Stones
where
    LocalVec<Stone, STONE_COUNT>: Index<T> + IndexMut<T>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        &mut self.stones[index]
    }
}

impl FromIterator<Stone> for Stones {
    fn from_iter<T: IntoIterator<Item = Stone>>(iter: T) -> Self {
        let mut stones = Self::new();
        stones.stones.extend(iter);
        stones.stones_count_change += stones.stones.len() as isize;
        stones
    }
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub stones: Stones,
    pub parameters: Parameters,
}

impl Sheet {
    pub fn new(parameters: Parameters) -> Self {
        Self { stones: Stones::new(), parameters }
    }

    pub fn count_score(&self) -> PerTeam<u8> {
        let out_of_house = *HOUSE_RADIUS + self.parameters.stone_radius;
        let mut score = PerTeam::<u8>::default();
        let stones_distances = teams().map(|team| {
            self.stones
                .iter()
                .filter(|s| s.team == team)
                .filter_map(|s| s.position(seconds(0.0)))
                .map(|pos| (pos - *TEE).norm())
                .filter(|&dist| dist < out_of_house || approx_eq!(dist, out_of_house))
                .sorted_by_key(|dist| NotNan::from_inner(dist.get::<foot>()))
                .collect_vec()
        });
        let nearest_stone = stones_distances.as_ref().map(|s| s.first().cloned());
        let winner = match nearest_stone {
            PerTeam { a: Some(a), b: Some(b) } if approx_eq!(a, b) => None,
            PerTeam { a: Some(a), b: Some(b) } if a < b => Some(Team::A),
            PerTeam { a: Some(a), b: Some(b) } if b < a => Some(Team::B),
            PerTeam { a: Some(_), b: None } => Some(Team::A),
            PerTeam { a: None, b: Some(_) } => Some(Team::B),
            _ => None,
        };

        if let Some(winner) = winner {
            let not_scoring_dist =
                stones_distances[winner.opponent()].first().cloned().unwrap_or(out_of_house);
            score[winner] = stones_distances[winner]
                .iter()
                .take_while(|&&d| d < not_scoring_dist && !approx_eq!(d, not_scoring_dist))
                .count() as u8;
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::stone;
    use crate::game::team::Team::{A, B};

    #[test]
    fn counting_score() {
        #[derive(Debug)]
        struct Case {
            stones: Vec<(Team, stone::Position)>,
            score: PerTeam<u8>,
        }

        impl Case {
            fn new<const M: usize>(
                stones: [(Team, f32, f32); M],
                (score_a, score_b): (u8, u8),
            ) -> Self {
                Self {
                    stones: stones
                        .into_iter()
                        .map(|(team, x, y)| (team, *TEE + Vector2 { x: feet(x), y: feet(y) }))
                        .collect(),
                    score: PerTeam { a: score_a, b: score_b },
                }
            }

            fn run(self) {
                let parameters = Parameters { stone_radius: feet(0.5), ..Parameters::default() };
                let stones = self.stones.iter().cloned().map(|(team, position)| Stone {
                    team,
                    state: stone::State::Stationary(stone::state::Stationary::new(position)),
                });
                let sheet = Sheet { parameters, stones: stones.collect() };
                let score = sheet.count_score();
                assert_eq!(score, self.score, "Error in {:?}", self);
            }
        }

        let cases = [
            Case::new([], (0, 0)),
            Case::new([(A, 6.5, 0.0), (B, 6.0, 6.0)], (0, 0)),
            Case::new([(A, -1.0, 0.0), (B, 1.0 + f32::EPSILON, 0.0)], (0, 0)),
            Case::new([(A, 0.0, 0.0)], (1, 0)),
            Case::new([(B, 5.0, 0.0)], (0, 1)),
            Case::new([(B, 0.0, 0.0), (A, -1.0, 0.0), (B, 1.0 + f32::EPSILON, 0.0)], (0, 1)),
            Case::new(
                [(B, 0.0, 0.0), (A, -1.0, 0.0), (B, 1.0 - f32::EPSILON, 0.0), (B, 2.0, 2.0)],
                (0, 1),
            ),
            Case::new([(A, 0.0, 0.0), (A, 1.0, 1.0), (B, -2.0, 0.0), (A, 3.0, 3.0)], (2, 0)),
            Case::new(
                [
                    (B, 0.0, 0.0),
                    (B, 1.0, 1.0),
                    (B, -1.0, -1.0),
                    (A, -2.0, 0.0),
                    (B, 3.0, 3.0),
                    (A, -4.0, 0.0),
                    (A, 0.0, -4.0),
                ],
                (0, 3),
            ),
        ];

        for case in cases {
            case.run()
        }
    }
}
