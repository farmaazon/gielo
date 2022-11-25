use crate::game::stone;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Dirty {
    pub finished_ends_count: isize,
    pub stones: stone::Flag,
    pub score: bool,
    pub phase: bool,
}

impl Dirty {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check_and_clear(&mut self, expected: &Self) {
        assert_eq!(self, expected);
        *self = Self::default();
    }
}
