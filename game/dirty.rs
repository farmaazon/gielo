use crate::sheet::stone;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Dirty {
    pub stones: stone::Flag,
    pub score: bool,
    pub turn: bool,
    pub turn_phase: bool,
    pub preview: bool,
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
