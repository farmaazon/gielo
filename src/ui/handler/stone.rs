use crate::ui::model;
use crate::{game, Game};
use std::cmp::Ordering;
use std::rc::Rc;

pub struct Handler {
    model: Rc<model::Stones>,
}

impl Handler {
    pub fn new(model: Rc<model::Stones>) -> Self {
        Self { model }
    }

    pub fn synchronize(&self, dirty: &game::Dirty, game: &Game) {
        let count = game.sheet.stones.len();
        let known_count = (count as isize - dirty.stone_count) as usize;
        match dirty.stone_count.cmp(&0) {
            Ordering::Greater => {
                self.model.notify.row_added(known_count, dirty.stone_count as usize)
            }
            Ordering::Less => self.model.notify.row_removed(count, (-dirty.stone_count) as usize),
            Ordering::Equal => {}
        }
        let changed_stones = (0..known_count).filter(|&stone| dirty.is_stone_dirty(stone));
        for stone in changed_stones {
            self.model.notify.row_changed(stone);
        }
    }
}
