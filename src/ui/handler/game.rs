use crate::{
    game,
    game::{
        RunningGame,
        dirty::Dirty,
        unit::{feet, seconds, time::second},
    },
    save_load::{SaveEntry, SaveLoad},
    ui,
    ui::{
        handler::{Snapshot, make_callback, stone, team},
        model::{call, set_ui_sheet_parameters, update_shot_preview},
    },
};
use anyhow::Result;
use gielo_game::{ViolatedRule, unit};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

const SIMULATION_FPS_CAP: u32 = 120;

pub struct Handler {
    ui: ui::Main,
    stone_handler: stone::Handler,
    team_handler: team::Handler,
    game: Rc<RefCell<RunningGame>>,
    snapshot: Rc<Snapshot>,
    delivery_start: Cell<web_time::Instant>,
    update_timer: RefCell<Option<slint::Timer>>,
}

impl Handler {
    pub fn initialize(ui: ui::Main, game: RunningGame, snapshot: Rc<Snapshot>) -> Rc<Self> {
        let game_model = ui.global::<ui::GameModel>();
        game_model.set_ends(game.setup().rules.ends as i32);
        let sheet_model = ui.global::<ui::SheetModel>();
        set_ui_sheet_parameters(&sheet_model, game.sheet_params());
        snapshot.set(Some(game.clone()));
        let team_handler = team::Handler::new(&game, &game_model);
        let game = Rc::new(RefCell::new(game));
        let stones_model = Rc::new(ui::model::Stones::new(game.clone()));
        let stone_handler = stone::Handler::new(stones_model.clone());
        sheet_model.set_stones(stones_model.into());

        let this = Rc::new(Self {
            ui: ui.clone_strong(),
            game,
            stone_handler,
            team_handler,
            delivery_start: Cell::new(web_time::Instant::now()),
            update_timer: RefCell::new(None),
            snapshot,
        });
        let to_initialize = Dirty {
            stones: game::sheet::stone::Flag::ALL,
            score: true,
            turn: true,
            turn_phase: true,
            preview: true,
        };
        this.synchronize(to_initialize);
        game_model.on_deliver(make_callback!(this.on_deliver()));
        game_model.on_proceed(make_callback!(this.on_proceed()));
        game_model.on_replace_stones(make_callback!(this.on_replace_stones()));
        ui.global::<ui::Shot>().on_update(make_callback!(this.on_shot_update()));
        this
    }

    pub fn synchronize(self: &Rc<Self>, dirty: Dirty) {
        let game = self.game.borrow();
        self.synchronize_phase(&dirty, &game);
        self.synchronize_violations(&dirty, &game);
        self.stone_handler.synchronize(&dirty);
        self.team_handler.synchronize_score(&dirty, &game);
        self.team_handler.synchronize_teams(&dirty, &game);
        if dirty.preview {
            update_shot_preview(&self.ui.global::<ui::Shot>(), &game);
        }
    }

    fn synchronize_phase(self: &Rc<Self>, dirty: &Dirty, game: &RunningGame) {
        let game_model = self.ui.global::<ui::GameModel>();
        if dirty.turn_phase {
            game_model.set_game_finished(game.game_finished());
            game_model.set_end_finished(game.end_finished().is_some());
            game_model.set_thinking(game.is_thinking());
            game_model.set_delivering(game.is_delivering());
            game_model.set_current_team(match game.current().team() {
                game::team::Team::A => 0,
                game::team::Team::B => 1,
            });
            game_model.set_current_player(game.current().player() as i32);
            if let Some(score) = game.end_finished() {
                game_model.set_end_score(if score.a > 0 {
                    score.a as i32
                } else {
                    -(score.b as i32)
                });
            }
            if !game.is_delivering() {
                if let Some(timer) = self.update_timer.take() {
                    timer.stop();
                }
            } else if self.update_timer.borrow().is_none() {
                let timer = slint::Timer::default();
                timer.start(
                    slint::TimerMode::Repeated,
                    Duration::from_secs(1) / SIMULATION_FPS_CAP,
                    make_callback!(self.update()),
                );
                *self.update_timer.borrow_mut() = Some(timer);
            }
        }
        if dirty.turn {
            game_model.set_end(game.current().turn.end() as i32);
        }
    }

    fn synchronize_violations(self: &Rc<Self>, dirty: &Dirty, game: &RunningGame) {
        let game_model = self.ui.global::<ui::GameModel>();
        if dirty.turn_phase {
            let violation = game.violation();
            game_model.set_fgz_rule_violated(violation == Some(ViolatedRule::FreeGuardRule));
            game_model.set_no_tick_rule_violated(violation == Some(ViolatedRule::NoTickRule));
        }
    }

    fn make_snapshot(&self) {
        self.snapshot.set(Some(self.game.borrow().clone()))
    }

    pub fn update(self: &Rc<Self>) -> Result<()> {
        let mut dirty = Dirty::new();
        let delivery_duration = web_time::Instant::now() - self.delivery_start.get();
        let delivery_time = seconds(delivery_duration.as_secs_f32() * self.ui.get_speed());
        self.game.borrow_mut().update(&mut dirty, delivery_time);
        self.synchronize(dirty);
        Ok(())
    }

    pub fn on_deliver(self: &Rc<Self>) -> Result<()> {
        let mut dirty = Dirty::new();
        let shot = self.ui.global::<ui::Shot>();
        {
            let mut game = self.game.borrow_mut();
            let call = call(&shot, game.sheet_params());
            game.start_delivery_marked(&mut dirty, call)?;
            self.delivery_start.set(web_time::Instant::now());
        }
        self.synchronize(dirty);
        self.make_snapshot();
        Ok(())
    }

    pub fn on_proceed(self: &Rc<Self>) -> Result<()> {
        let mut dirty = Dirty::new();
        self.game.borrow_mut().proceed(&mut dirty)?;
        self.synchronize(dirty);
        self.make_snapshot();
        Ok(())
    }

    pub fn on_replace_stones(self: &Rc<Self>) -> Result<()> {
        let mut dirty = Dirty::new();
        self.game.borrow_mut().replace_stones(&mut dirty)?;
        self.synchronize(dirty);
        self.make_snapshot();
        Ok(())
    }

    pub fn on_shot_update(self: &Rc<Self>) -> Result<()> {
        let game = self.game.borrow();
        let shot = self.ui.global::<ui::Shot>();
        if shot.get_automatic_weight() {
            let target_y = feet(shot.get_mark_y() as unit::BaseType);
            let velocity = game.sheet_params().velocity_for_target_y(target_y);
            if let Some(hog_to_hog) = game.sheet_params().hot_to_hog_time_from_velocity(velocity) {
                shot.set_hog_to_hog_time(hog_to_hog.get::<second>() as f32);
            }
        }
        update_shot_preview(&shot, &game);
        Ok(())
    }

    pub fn save<'a>(&self, save_load: &'a mut SaveLoad) -> Result<&'a SaveEntry> {
        let game = self.game.borrow();
        save_load.save_game(game.game())
    }
}
