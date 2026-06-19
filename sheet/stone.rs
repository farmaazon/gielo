use derive_more::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use gielo_unit as unit;
use gielo_unit::{ConstZero, vector::Vector2};
use serde::{Deserialize, Serialize};

use crate::Parameters;

pub const COUNT: usize = 16;

pub type Id = usize;
pub type Position = Vector2<unit::Length>;

#[derive(
    Copy,
    Clone,
    Default,
    Eq,
    PartialEq,
    BitAnd,
    BitAndAssign,
    BitOr,
    BitOrAssign,
    BitXor,
    BitXorAssign,
    Not,
    Deserialize,
    Serialize,
)]
pub struct Flag(pub u16);

impl Flag {
    pub const ALL: Self = Self(u16::MAX);

    pub fn stone(id: Id) -> Self {
        Self(1 << id)
    }

    pub fn all_before(id: Id) -> Self {
        if id >= COUNT { Self::ALL } else { Self((1 << id) - 1) }
    }

    pub fn range(range: impl std::ops::RangeBounds<Id>) -> Self {
        use std::ops::Bound::*;
        let from_start = match range.start_bound() {
            Included(&id) => !Self::all_before(id),
            Excluded(&id) => !(Self::all_before(id) | Self::stone(id)),
            Unbounded => Flag::ALL,
        };
        let to_end = match range.end_bound() {
            Included(&id) => Self::all_before(id) | Self::stone(id),
            Excluded(&id) => Self::all_before(id),
            Unbounded => Flag::ALL,
        };
        from_start & to_end
    }

    pub fn set(&mut self, id: Id) {
        *self |= Flag::stone(id)
    }

    pub fn unset(&mut self, id: Id) {
        *self &= !Flag::stone(id)
    }

    pub fn contains(self, id: Id) -> bool {
        self & Flag::stone(id) != Flag(0)
    }

    pub fn iter_ids(self) -> impl Iterator<Item = Id> {
        (0..COUNT).filter(move |&id| self.contains(id))
    }

    pub fn assert_and_clear(&mut self, rhs: Self) {
        assert_eq!(*self, rhs);
        *self = Self(0);
    }
}

impl FromIterator<Id> for Flag {
    fn from_iter<T: IntoIterator<Item = Id>>(iter: T) -> Self {
        iter.into_iter().fold(Flag(0), |flag, id| flag | Flag::stone(id))
    }
}

impl std::fmt::Debug for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Flag({:0>16b})", self.0)
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Stones {
    positions: [Position; COUNT],
    in_play: Flag,
}

impl Stones {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn positions(&self) -> &[Position; COUNT] {
        &self.positions
    }

    pub fn in_play(&self) -> Flag {
        self.in_play
    }

    pub fn iter_flag(&self, flag: Flag) -> impl Iterator<Item = (Id, Position)> + '_ {
        flag.iter_ids().map(|id| (id, self.positions[id]))
    }

    pub fn iter_in_play(&self) -> impl Iterator<Item = (Id, Position)> + '_ {
        self.iter_flag(self.in_play)
    }

    pub fn set_position(&mut self, dirty: &mut Flag, id: Id, position: Position) {
        self.positions[id] = position;
        dirty.set(id)
    }

    pub fn put_stone(&mut self, dirty: &mut Flag, id: Id, position: Position) {
        self.set_position(dirty, id, position);
        self.in_play.set(id);
    }

    pub fn remove_stone(&mut self, dirty: &mut Flag, id: Id) {
        self.set_position(dirty, id, Position::ZERO);
        self.in_play.unset(id);
    }

    pub fn clear(&mut self, dirty: &mut Flag) {
        *dirty |= self.in_play;
        self.in_play = Flag(0);
    }

    pub fn restore(&mut self, dirty: &mut Flag, snapshot: Stones) {
        *dirty |= self.in_play | snapshot.in_play;
        *self = snapshot;
    }

    pub fn stone_flags<F: FnMut(Position) -> bool>(&self, mut predicate: F) -> Flag {
        self.iter_in_play().filter_map(|(id, stone)| predicate(stone).then_some(id)).collect()
    }

    pub fn guards(&self, parameters: &Parameters) -> Flag {
        self.stone_flags(|stone| parameters.is_guard(stone)) & self.in_play()
    }

    pub fn center_guards(&self, parameters: &Parameters) -> Flag {
        self.stone_flags(|stone| parameters.is_center_guard(stone)) & self.in_play()
    }
}

impl PartialEq for Stones {
    fn eq(&self, other: &Self) -> bool {
        self.iter_in_play().eq(other.iter_in_play())
    }
}

impl FromIterator<(Id, Position)> for Stones {
    fn from_iter<T: IntoIterator<Item = (Id, Position)>>(iter: T) -> Self {
        let mut stones = Self::new();
        for (id, position) in iter {
            stones.positions[id] = position;
            stones.in_play |= Flag::stone(id);
        }
        stones
    }
}

impl std::fmt::Debug for Stones {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter_in_play()).finish()
    }
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Rotation {
    #[default]
    None,
    Clockwise,
    CounterClockwise,
}

#[cfg(test)]
mod tests {
    use crate::parameters::Geometry;

    use super::*;
    use gielo_unit::feet;
    use unit::length::foot;

    #[test]
    fn setting_and_unsetting_flag() {
        let mut flag = Flag(0);
        flag |= Flag::stone(1);
        flag |= Flag::stone(3);
        assert!(!flag.contains(0));
        assert!(flag.contains(1));
        assert!(!flag.contains(2));
        assert!(flag.contains(3));
        assert!(!flag.contains(4));

        assert!((flag | Flag::stone(2)).contains(2));
        assert!(!(flag & Flag::stone(3)).contains(1));
        assert!((flag & Flag::stone(3)).contains(3));

        flag &= Flag::stone(3);
        assert!(!flag.contains(0));
        assert!(!flag.contains(1));
        assert!(!flag.contains(2));
        assert!(flag.contains(3));
        assert!(!flag.contains(4));
    }

    #[test]
    fn iterating_ids_in_flag() {
        assert_eq!(Flag(0).iter_ids().collect::<Vec<_>>(), Vec::<Id>::new());
        assert_eq!(Flag(1).iter_ids().collect::<Vec<_>>(), vec![0]);
        assert_eq!(Flag(2).iter_ids().collect::<Vec<_>>(), vec![1]);
        assert_eq!(Flag(3).iter_ids().collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(Flag(4).iter_ids().collect::<Vec<_>>(), vec![2]);
        assert_eq!(Flag(15).iter_ids().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(Flag::ALL.iter_ids().collect::<Vec<_>>(), Vec::from_iter(0..COUNT));
    }

    #[test]
    fn creating_flag_all_before() {
        for i in 0..COUNT {
            let flag = Flag::all_before(i);
            for j in 0..COUNT {
                assert_eq!(flag.contains(j), j < i);
            }
        }
    }

    #[test]
    fn modifying_stones() {
        let mut stones = Stones::new();
        let mut dirty = Flag(0);
        let position_a = Position { x: feet(1.0), y: feet(100.0) };
        let position_b = Position { x: feet(1.0), y: feet(120.0) };
        let position_c = Position { x: feet(1.0), y: feet(120.0) };

        stones.put_stone(&mut dirty, 3, position_a);
        assert_eq!(stones.positions()[3], position_a);
        assert_eq!(stones.in_play(), Flag::stone(3));
        dirty.assert_and_clear(Flag::stone(3));

        stones.put_stone(&mut dirty, 15, position_b);
        assert_eq!(stones.positions()[3], position_a);
        assert_eq!(stones.positions()[15], position_b);
        assert_eq!(stones.in_play(), Flag::stone(3) | Flag::stone(15));
        dirty.assert_and_clear(Flag::stone(15));

        stones.set_position(&mut dirty, 3, position_c);
        assert_eq!(stones.positions()[3], position_c);
        assert_eq!(stones.positions()[15], position_b);
        assert_eq!(stones.in_play(), Flag::stone(3) | Flag::stone(15));
        dirty.assert_and_clear(Flag::stone(3));

        stones.remove_stone(&mut dirty, 3);
        assert_eq!(stones.positions()[15], position_b);
        assert_eq!(stones.in_play(), Flag::stone(15));
        dirty.assert_and_clear(Flag::stone(3));

        stones.put_stone(&mut Flag(0), 0, position_a);
        stones.clear(&mut dirty);
        assert_eq!(stones.in_play(), Flag(0));
        dirty.assert_and_clear(Flag::stone(0) | Flag::stone(15));
    }

    #[test]
    fn stone_flags() {
        let mut stones = Stones::new();
        let position_a = Position { x: feet(1.0), y: feet(100.0) };
        let position_b = Position { x: feet(1.0), y: feet(120.0) };
        stones.put_stone(&mut Flag(0), 1, position_a);
        stones.put_stone(&mut Flag(0), 2, position_b);
        stones.put_stone(&mut Flag(0), 4, position_b);
        stones.put_stone(&mut Flag(0), 5, position_b);
        stones.put_stone(&mut Flag(0), 8, position_a);
        stones.put_stone(&mut Flag(0), 15, position_a);

        let a_stones = stones.stone_flags(|s| s == position_a);
        assert_eq!(a_stones, Flag::stone(1) | Flag::stone(8) | Flag::stone(15));

        stones.remove_stone(&mut Flag(0), 4);
        let b_stones = stones.stone_flags(|s| s == position_b);
        assert_eq!(b_stones, Flag::stone(2) | Flag::stone(5));
    }

    #[derive(Debug)]
    struct FlagTest {
        parameters: Parameters,
        stones: Stones,
        expected: Flag,
    }

    impl FlagTest {
        fn new(
            data: impl IntoIterator<Item = (Id, (unit::BaseType, unit::BaseType), bool)>,
        ) -> Self {
            let parameters = Parameters::default();
            let tee = parameters.geometry.tee();
            let mut expected = Flag(0);
            let stones = data
                .into_iter()
                .map(|(id, (x, y), exp)| {
                    if exp {
                        expected |= Flag::stone(id);
                    }
                    (id, tee + Vector2 { x: feet(x), y: feet(y) })
                })
                .collect();
            Self { parameters, stones, expected }
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
        assert_eq!(test.stones.guards(&test.parameters), test.expected)
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
        assert_eq!(test.stones.center_guards(&test.parameters), test.expected)
    }
}
