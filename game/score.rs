use crate::{sheet, team};
use decorum::NotNan;
use gielo_sheet::stone::Stones;
use gielo_team::{PerTeam, Team};
use gielo_unit::{BaseType, float_eq, length::foot};
use itertools::Itertools;

pub type Score = PerTeam<u8>;

pub fn count_score(stones: &Stones, sheet_params: &sheet::Parameters) -> Score {
    let out_of_house = sheet_params.geometry.house_radius + sheet_params.stone_radius;
    let mut score = Score::default();
    let stones_distances = team::stone::STONES.map(|team_stones| {
        stones
            .iter_flag(team_stones & stones.in_play())
            .filter_map(|(_, s)| sheet_params.dist_from_tee_in_house(s))
            .sorted_by_key(|dist| NotNan::<BaseType>::assert(dist.get::<foot>()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sheet::{parameters::Geometry, stone},
        team::Team::{A, B},
        unit,
    };
    use gielo_unit::{feet, vector::Vector2};

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
                let parameters =
                    sheet::Parameters { stone_radius: feet(0.5), ..sheet::Parameters::default() };
                let mut per_team = team::stone::TEAM_IDS;
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
                let score = count_score(&stones, &parameters);
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
