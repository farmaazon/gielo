#[macro_use]
extern crate uom;
extern crate core;

pub mod game;
pub mod profiles;
pub mod save_load;
pub mod ui;
pub mod unit;
pub mod vector;

pub use crate::game::Game;
use crate::ui::handler::Handler;
use slint::ComponentHandle;
use std::time::Duration;

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();

    let ui = ui::Main::new().unwrap();
    let weak_ui = ui.as_weak();
    let blinking = slint::Timer::default();
    blinking.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {
        if let Some(ui) = weak_ui.upgrade() {
            ui.set_blinking_stone_visible(!ui.get_blinking_stone_visible());
        }
    });
    let weak_ui = ui.as_weak();
    ui.global::<ui::Functions>().on_rebound(move || {
        if let Some(ui) = weak_ui.upgrade() {
            ui.set_rebounding(true);
            slint::Timer::single_shot(Duration::from_millis(0), move || ui.set_rebounding(false));
        }
    });
    ui.global::<ui::Functions>().initialize();
    let _handler = Handler::initialize(ui.clone_strong());
    ui.run().unwrap();
}
