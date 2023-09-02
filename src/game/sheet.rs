use crate::{
    game::{
        stone,
        stone::Stones,
        team::{PerTeam, Team},
    },
    unit::{float_eq, Length},
    vector::EuclideanNorm,
};
use decorum::NotNan;
use itertools::Itertools;
use uom::si::length::foot;

pub mod parameters;

pub use parameters::Parameters;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Hack {
    #[default]
    Left,
    Right,
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
        (dist <= out_of_house || float_eq!(dist, out_of_house)).then_some(dist)
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
            || float_eq!(dist_from_center_line, self.parameters.stone_radius);
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
            PerTeam { a: Some(a), b: Some(b) } if float_eq!(a, b) => None,
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
                .take_while(|&&d| d < not_scoring_dist && !float_eq!(d, not_scoring_dist))
                .count() as u8;
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{
            sheet::{parameters::Geometry, stone},
            team::Team::{A, B},
        },
        unit,
        unit::feet,
        vector::Vector2,
    };

    #[derive(Debug)]
    struct FlagTest {
        stones: Vec<(stone::Id, stone::Position)>,
        expected: stone::Flag,
    }

    impl FlagTest {
        fn new(
            data: impl IntoIterator<Item = (stone::Id, (unit::BaseType, unit::BaseType), bool)>,
        ) -> Self {
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
                stones: [(Team, unit::BaseType, unit::BaseType); M],
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
            Case::new([(A, -1.0, 0.0), (B, 1.0 + unit::BaseType::EPSILON, 0.0)], (0, 0)),
            Case::new([(A, 0.0, 0.0)], (1, 0)),
            Case::new([(B, 5.0, 0.0)], (0, 1)),
            Case::new(
                [(B, 0.0, 0.0), (A, -1.0, 0.0), (B, 1.0 + unit::BaseType::EPSILON, 0.0)],
                (0, 1),
            ),
            Case::new(
                [
                    (B, 0.0, 0.0),
                    (A, -1.0, 0.0),
                    (B, 1.0 - unit::BaseType::EPSILON, 0.0),
                    (B, 2.0, 2.0),
                ],
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
