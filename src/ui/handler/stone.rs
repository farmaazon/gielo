use crate::{game, ui::model};
use std::rc::Rc;

pub struct Handler {
    model: Rc<model::Stones>,
}

impl Handler {
    pub fn new(model: Rc<model::Stones>) -> Self {
        Self { model }
    }

    pub fn synchronize(&self, dirty: &game::Dirty) {
        for stone in dirty.stones.iter_ids() {
            self.model.notify.row_changed(stone);
        }
    }
}
