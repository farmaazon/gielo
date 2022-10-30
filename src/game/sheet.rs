use crate::game::stones::Stones;
use crate::game::team::{teams, PerTeam, Team};
use crate::unit::{approx_eq, feet_squared_per_second_squared, AvailableEnergy};
use crate::unit::{feet, feet_per_second_squared, inches, seconds, Acceleration, Length};
use crate::vector::{EuclideanNorm, Vector2};
use decorum::NotNan;
use itertools::Itertools;
use std::f32::consts::PI;
use uom::si::length::foot;

#[derive(Copy, Clone, Debug)]
pub enum Hack {
    Left,
    Right,
}

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

    pub fn measure_dist(&self) -> Length {
        self.delivery_end.hog_line_y - self.delivery_end.tee_line_y
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

impl Default for Parameters {
    fn default() -> Self {
        Self {
            geometry: Geometry::default(),
            friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
            rotation_acc: feet_per_second_squared(245.0 / 8649.0),
            stone_radius: inches(18.0 / PI),
            static_friction: feet_squared_per_second_squared(0.25),
        }
    }
}

#[derive(Debug)]
pub struct Sheet {
    pub stones: Stones,
    pub parameters: Parameters,
}

impl Sheet {
    pub fn new(parameters: Parameters) -> Self {
        Self { stones: Stones::new(), parameters }
    }

    pub fn count_score(&self) -> PerTeam<u8> {
        let out_of_house = self.parameters.geometry.house_radius + self.parameters.stone_radius;
        let mut score = PerTeam::<u8>::default();
        let stones_distances = teams().map(|team| {
            self.stones
                .iter()
                .filter(|s| s.team() == team)
                .filter_map(|s| s.position(seconds(0.0)))
                .map(|pos| (pos - self.parameters.geometry.tee()).norm())
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
    use crate::game::team::Team::{A, B};
    use crate::game::{stone, Stone};

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
                let tee = Geometry::default().tee();
                Self {
                    stones: stones
                        .into_iter()
                        .map(|(team, x, y)| (team, tee + Vector2 { x: feet(x), y: feet(y) }))
                        .collect(),
                    score: PerTeam { a: score_a, b: score_b },
                }
            }

            fn run(self) {
                let parameters = Parameters { stone_radius: feet(0.5), ..Parameters::default() };
                let stones =
                    self.stones.iter().cloned().map(|(team, position)| {
                        Stone::new(team, stone::State::Stationary(position))
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
