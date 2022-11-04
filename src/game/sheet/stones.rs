use crate::game::sheet::stone::Stone;
use crate::game::{team, Dirty};
use derive_more::{AsRef, Deref};
use local_vec::LocalVec;
use std::ops::{Index, IndexMut};

pub const PER_TEAM: usize = 8;
pub const COUNT: usize = PER_TEAM * team::TEAMS_COUNT;

#[derive(Clone, Debug, Default, AsRef, Deref, PartialEq)]
pub struct Stones {
    stones: LocalVec<Stone, COUNT>,
}

impl Stones {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Stone> {
        self.stones.iter_mut()
    }

    pub fn push(&mut self, dirty: &mut Dirty, new_stone: Stone) {
        self.stones.push(new_stone);
        dirty.stone_count += 1;
    }

    pub fn clear(&mut self, dirty: &mut Dirty) {
        dirty.stone_count -= self.stones.len() as isize;
        self.stones.clear();
    }
}

impl<T> Index<T> for Stones
where
    LocalVec<Stone, COUNT>: Index<T>,
{
    type Output = <LocalVec<Stone, COUNT> as Index<T>>::Output;

    fn index(&self, index: T) -> &Self::Output {
        &self.stones[index]
    }
}

impl<T> IndexMut<T> for Stones
where
    LocalVec<Stone, COUNT>: Index<T> + IndexMut<T>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        &mut self.stones[index]
    }
}

impl FromIterator<Stone> for Stones {
    fn from_iter<T: IntoIterator<Item = Stone>>(iter: T) -> Self {
        let mut stones = Self::new();
        stones.stones.extend(iter);
        stones
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::game::sheet::stone;
    use crate::game::team::Team;

    #[test]
    pub fn tracking_stones_changes() {
        let mut stones = Stones::default();
        let stone = Stone::new(Team::A, stone::State::Out);

        let mut dirty = Dirty::new();
        stones.push(&mut dirty, stone.clone());
        assert_eq!(stones.len(), 1);
        assert_eq!(dirty.stone_count, 1);

        let mut dirty = Dirty::new();
        stones.push(&mut dirty, stone.clone());
        stones.push(&mut dirty, stone.clone());
        assert_eq!(stones.len(), 3);
        assert_eq!(dirty.stone_count, 2);

        let mut dirty = Dirty::new();
        stones.clear(&mut dirty);
        assert_eq!(stones.len(), 0);
        assert_eq!(dirty.stone_count, -3);
    }
}
