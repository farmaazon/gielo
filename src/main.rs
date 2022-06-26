#[macro_use]
extern crate uom;

pub mod game;
pub mod ui;
pub mod unit;
pub mod vector;

pub use crate::game::Game;
use crate::ui::handler::Handler;
use slint::ComponentHandle;
use std::time::Duration;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Tracked<T, Change> {
    value: T,
    change: Change,
}

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    let ui = ui::Main::new();
    let weak_ui = ui.as_weak();
    let blinking = slint::Timer::default();
    blinking.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {
        if let Some(ui) = weak_ui.upgrade() {
            ui.set_blinking_stone_visible(!ui.get_blinking_stone_visible());
        }
    });
    ui.global::<ui::Functions>().initialize();
    let handler = Handler::initialize(ui.clone_strong());
    handler.on_game_start().unwrap();
    ui.run();
}
