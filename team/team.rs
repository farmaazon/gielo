use crate::{player, player::Player, PerTeam};
use serde::{Deserialize, Serialize};

pub const TEAMS_COUNT: usize = 2;
pub const TEAMS: PerTeam<Team> = PerTeam { a: Team::A, b: Team::B };

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

pub fn teams() -> PerTeam<Team> {
    PerTeam { a: Team::A, b: Team::B }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Info<NameT, ColorT> {
    pub name: NameT,
    pub color: ColorT,
    pub players: [Player<NameT>; player::PER_TEAM_COUNT],
}
