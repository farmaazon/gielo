use crate::game::stone;
use crate::game::stone::Stones;
use crate::game::team::{PerTeam, Team};
use crate::unit::{approx_eq, feet_squared_per_second_squared, AvailableEnergy};
use crate::unit::{feet, feet_per_second_squared, inches, Acceleration, Length};
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
        self.playing_end.hog_line_y - self.delivery_end.hog_line_y
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
            friction: feet_per_second_squared(0.24),
            rotation_acc: feet_per_second_squared(0.025),
            stone_radius: inches(18.0 / PI),
            static_friction: feet_squared_per_second_squared(0.25),
        }
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

    pub fn dist_from_tee(&self, position: stone::Position) -> Length {
        (position - self.parameters.geometry.tee()).norm()
    }

    pub fn dist_from_tee_in_house(&self, position: stone::Position) -> Option<Length> {
        let out_of_house = self.parameters.geometry.house_radius + self.parameters.stone_radius;
        let dist = self.dist_from_tee(position);
        (dist <= out_of_house || approx_eq!(dist, out_of_house)).then_some(dist)
    }

    pub fn is_in_house(&self, position: stone::Position) -> bool {
        self.dist_from_tee_in_house(position).is_some()
    }

    pub fn is_guard(&self, position: stone::Position) -> bool {
        let before_tee_line = (position.y + self.parameters.stone_radius)
            < self.parameters.geometry.playing_end.tee_line_y;
        !self.is_in_house(position) && before_tee_line
    }

    pub fn is_center_guard(&self, position: stone::Position) -> bool {
        let dist_from_center_line = (position.x - self.parameters.geometry.center_line_x).abs();
        let touches_center_line = dist_from_center_line < self.parameters.stone_radius
            || approx_eq!(dist_from_center_line, self.parameters.stone_radius);
        self.is_guard(position) && touches_center_line
    }

    pub fn guards(&self) -> stone::Flag {
        self.stones.stone_flags(|stone| self.is_guard(stone)) & self.stones.in_play()
    }

    pub fn center_guards(&self) -> stone::Flag {
        self.stones.stone_flags(|stone| self.is_center_guard(stone)) & self.stones.in_play()
    }

    pub fn count_score(&self) -> PerTeam<u8> {
        let out_of_house = self.parameters.geometry.house_radius + self.parameters.stone_radius;
        let mut score = PerTeam::<u8>::default();
        let stones_distances = stone::Flag::TEAM.map(|team_stones| {
            self.stones
                .iter_flag(team_stones & self.stones.in_play())
                .filter_map(|(_, s)| self.dist_from_tee_in_house(s))
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
    use crate::game::sheet::stone;
    use crate::game::team::Team::{A, B};

    #[derive(Debug)]
    struct FlagTest {
        stones: Vec<(stone::Id, stone::Position)>,
        expected: stone::Flag,
    }

    impl FlagTest {
        fn new(data: impl IntoIterator<Item = (stone::Id, (f32, f32), bool)>) -> Self {
            let tee = Geometry::default().tee();
            let mut expected = stone::Flag(0);
            let stones = data
                .into_iter()
                .map(|(id, (x, y), exp)| {
                    if exp {
                        expected |= stone::Flag::stone(id);
                    }
                    (id, tee + Vector2 { x: feet(x), y: feet(y) })
                })
                .collect();
            Self { stones, expected }
        }

        fn sheet(&self) -> Sheet {
            Sheet {
                parameters: Parameters::default(),
                stones: self.stones.iter().cloned().collect(),
            }
        }
    }

    #[test]
    fn guards() {
        let house_radius = Geometry::default().house_radius.value;
        let test = FlagTest::new([
            (0, (0.0, 0.0), false),
            (1, (0.0, -house_radius), false),
            (2, (0.0, -house_radius - 1.0), true),
            (3, (-4.0, -house_radius - 4.0), true),
            (4, (4.0, -house_radius - 4.0), true),
            (5, (house_radius - 0.1, house_radius - 0.1), false),
            (8, (-house_radius + 0.1, house_radius - 0.1), false),
        ]);
        assert_eq!(test.sheet().guards(), test.expected)
    }

    #[test]
    fn center_guards() {
        let house_radius = Geometry::default().house_radius.get::<foot>();
        let stone_radius = Parameters::default().stone_radius.get::<foot>();
        let test = FlagTest::new([
            (0, (0.0, 0.0), false),
            (1, (0.0, -house_radius), false),
            (2, (0.0, -house_radius - 1.0), true),
            (3, (-4.0, -house_radius - 4.0), false),
            (4, (4.0, -house_radius - 4.0), false),
            (5, (house_radius - 0.1, house_radius - 0.1), false),
            (8, (-house_radius + 0.1, house_radius - 0.1), false),
            (9, (-stone_radius, -house_radius - 4.0), true),
            (10, (stone_radius, -house_radius - 4.0), true),
        ]);
        assert_eq!(test.sheet().center_guards(), test.expected)
    }

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
                let mut per_team = stone::TEAM_IDS;
                let stones = self
                    .stones
                    .iter()
                    .map(|&(team, position)| {
                        (
                            per_team[team].next().expect("Too many stones of one team in case"),
                            position,
                        )
                    })
                    .collect();
                let sheet = Sheet { parameters, stones };
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
