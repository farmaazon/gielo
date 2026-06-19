use crate::ui::handler::Handler;
use anyhow::Result;
use derive_more::Deref;
use slint::ComponentHandle;
use std::{cell::Cell, panic, rc::Rc, time::Duration};
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

pub mod profiles;
pub mod save_load;
pub mod ui;

pub mod game {
    pub use gielo_game::{
        slint::{Game, RunningGame, Setup},
        *,
    };
}

#[derive(Default, Deref)]
pub struct Snapshot(Cell<Option<game::RunningGame>>);

thread_local! {
    pub static CRASH_INFO: Cell<Option<String>> = Default::default();
}

#[cfg(target_family = "wasm")]
fn setup_logger() {
    wasm_logger::init(wasm_logger::Config::default());
}

#[cfg(not(target_family = "wasm"))]
fn setup_logger() {
    if let Err(error) = simple_logger::SimpleLogger::new().init() {
        eprintln!("Failed to initialize logger. {error}. Probably no logs will be reported.");
    }
}

fn initialize(
    snapshot: Rc<Snapshot>,
    crash_info: Option<String>,
) -> Result<(ui::Main, Rc<Handler>)> {
    let ui = ui::Main::new()?;
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

#[cfg_attr(target_family = "wasm", wasm_bindgen(start))]
pub fn start() {
    setup_logger();

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
