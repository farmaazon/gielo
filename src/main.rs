use crate::ui::handler::Handler;
use anyhow::Result;
use derive_more::Deref;
use slint::ComponentHandle;
use std::{cell::Cell, panic, rc::Rc, time::Duration};

pub mod profiles;
pub mod save_load;
pub mod ui;
pub use gielo_game as game;
use gielo_game::Game;

#[derive(Default, Deref)]
pub struct Snapshot(Cell<Option<Game>>);

thread_local! {
    pub static CRASH_INFO: Cell<Option<String>> = Default::default();
}

fn initialize(
    snapshot: Rc<Snapshot>,
    crash_info: Option<String>,
) -> Result<(ui::Main, Rc<Handler>)> {
    let ui = ui::Main::new()?;
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
    if let Some(panic) = crash_info {
        ui.global::<ui::Error>().invoke_report_error(
            format!("Application crashed!. Recovered last known valid state.\n\n {panic}").into(),
        );
    }
    let handler = Handler::initialize(ui.clone_strong(), snapshot.take(), snapshot);
    ui.show()?;
    Ok((ui, handler))
}

fn main() {
    if let Err(error) = simple_logger::SimpleLogger::new().init() {
        eprintln!("Failed to initialize logger. {error}. Probably no logs will be reported.");
    }

    let std_panic_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic| {
        std_panic_hook(panic);
        CRASH_INFO.with(|info| info.set(Some(panic.to_string())));
    }));

    let snapshot = Rc::new(Snapshot::default());
    loop {
        let last_crash = CRASH_INFO.with(|info| info.take());
        let (ui, _handler) =
            initialize(snapshot.clone(), last_crash).expect("Failed to initialize application");
        let loop_result = panic::catch_unwind(slint::run_event_loop);
        ui.hide().expect("Failed to tear down application");
        match loop_result {
            Ok(result) => {
                result.expect("Failed to run event loop");
                break;
            }
            Err(_) => {
                log::error!("Trying to restore application after crash");
            }
        }
    }
}
