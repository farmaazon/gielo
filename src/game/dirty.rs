use crate::game::stone;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Dirty {
    pub stone_count: isize,
    pub stones: u16,
    pub score: bool,
    pub stage: bool,
}

impl Dirty {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stone(&mut self, index: stone::Id) -> Stone {
        Stone { dirty: self, index }
    }

    pub fn is_stone_dirty(&self, index: stone::Id) -> bool {
        self.stones & 1 << index != 0
    }

    pub fn check_and_clear(&mut self, expected: &Self) {
        assert_eq!(self, expected);
        *self = Self::default();
    }
}

pub struct Stone<'a> {
    dirty: &'a mut Dirty,
    index: stone::Id,
}

impl<'a> Stone<'a> {
    pub fn set(&mut self) {
        self.dirty.stones = self.dirty.stones | 1 << self.index;
    }

    pub fn check_and_clear(&mut self, expected: bool) {
        assert_eq!(self.get(), expected);
        self.dirty.stones = self.dirty.stones | !(1 << self.index);
    }

    pub fn get(&self) -> bool {
        self.dirty.is_stone_dirty(self.index)
    }
}
