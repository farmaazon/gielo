use crate::{PerTeam, Team};
pub use gielo_sheet::stone::{Flag, Id, COUNT};

pub const COUNT_PER_TEAM: usize = 8;
pub const TEAM_IDS: PerTeam<std::ops::Range<Id>> =
    PerTeam { a: 0..COUNT_PER_TEAM, b: COUNT_PER_TEAM..COUNT };

pub const STONES: PerTeam<Flag> =
    PerTeam { a: Flag(0b0000_0000_1111_1111), b: Flag(0b1111_1111_0000_0000) };
pub const QUEUE_BY_HAMMER: PerTeam<[Id; COUNT]> = PerTeam {
    a: [8, 0, 9, 1, 10, 2, 11, 3, 12, 4, 13, 5, 14, 6, 15, 7],
    b: [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15],
};

pub fn team(id: Id) -> Team {
    if STONES.a.contains(id) {
        Team::A
    } else {
        Team::B
    }
}
