use crate::ui::model;
use gielo_game::dirty::Dirty;
use std::rc::Rc;

pub struct Handler {
    model: Rc<model::Stones>,
}

impl Handler {
    pub fn new(model: Rc<model::Stones>) -> Self {
        Self { model }
    }

    pub fn synchronize(&self, dirty: &Dirty) {
        for stone in dirty.stones.iter_ids() {
            self.model.notify.row_changed(stone);
        }
    }
}
