use crate::game::{team, Stone};
use crate::Tracked;
use derive_more::{AsRef, Deref};
use local_vec::LocalVec;
use std::ops::{Index, IndexMut};

pub const PER_TEAM: usize = 8;
pub const COUNT: usize = PER_TEAM * team::TEAMS_COUNT;

pub type TrackedLen = Tracked<usize, isize>;

#[derive(Clone, Debug, Default, AsRef, Deref)]
pub struct Stones {
    #[deref]
    stones: LocalVec<Stone, COUNT>,
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

    pub fn read_len(&mut self) -> TrackedLen {
        TrackedLen {
            value: self.stones.len(),
            change: std::mem::take(&mut self.stones_count_change),
        }
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
        stones.stones_count_change += stones.stones.len() as isize;
        stones
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::game::stone;
    use crate::game::team::Team;

    #[test]
    pub fn tracking_stones_changes() {
        let mut stones = Stones::default();
        let stone = Stone { team: Team::A, state: stone::State::Out { dirty: false } };
        stones.push(stone.clone());
        assert_eq!(stones.read_len(), TrackedLen { value: 1, change: 1 });
        stones.push(stone.clone());
        stones.push(stone.clone());
        assert_eq!(stones.read_len(), TrackedLen { value: 3, change: 2 });
        stones.clear();
        assert_eq!(stones.read_len(), TrackedLen { value: 0, change: -3 });
        stones.push(stone.clone());
        stones.read_len();
        stones.push(stone);
        stones.clear();
        assert_eq!(stones.read_len(), TrackedLen { value: 0, change: -1 });
    }
}
