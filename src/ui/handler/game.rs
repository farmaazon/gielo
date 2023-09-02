use crate::{
    game,
    game::{end, stone::Flag, turn},
    ui,
    ui::handler::{make_callback, stone, team},
    unit,
    unit::feet,
    Game,
};
use anyhow::Result;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time, time::Duration};
use uom::si::time::second;

pub struct Handler {
    ui: ui::Main,
    stone_handler: stone::Handler,
    team_handler: team::Handler,
    game: Rc<RefCell<Game>>,
    update_timer: RefCell<Option<slint::Timer>>,
}

impl Handler {
    pub fn initialize(ui: ui::Main, game: Game) -> Rc<Self> {
        let game_model = ui.global::<ui::GameModel>();
        game_model.set_ends(game.params.ends as i32);
        let sheet_model = ui.global::<ui::SheetModel>();
        let team_handler = team::Handler::new(&game, &game_model);
        let game = Rc::new(RefCell::new(game));
        let stones_model = Rc::new(ui::model::Stones::new(game.clone()));
        let stone_handler = stone::Handler::new(stones_model.clone());
        let update_timer = RefCell::new(None);
        sheet_model.set_stones(stones_model.into());

        let this = Rc::new(Self {
            ui: ui.clone_strong(),
            game,
            stone_handler,
            team_handler,
            update_timer,
        });
        let to_initialize = game::Dirty {
            finished_ends_count: 0,
            stones: Flag::ALL,
            score: true,
            phase: true,
            preview: true,
        };
        this.synchronize(to_initialize);
        game_model.on_deliver(make_callback!(this.on_deliver()));
        game_model.on_proceed(make_callback!(this.on_proceed()));
        game_model.on_replace_stones(make_callback!(this.on_replace_stones()));
        ui.global::<ui::Shot>().on_update(make_callback!(this.on_shot_update()));
        this
    }

    pub fn synchronize(self: &Rc<Self>, dirty: game::Dirty) {
        let game = self.game.borrow();
        self.synchronize_phase(&dirty, &game);
        self.synchronize_violations(&dirty, &game);
        self.stone_handler.synchronize(&dirty);
        self.team_handler.synchronize_score(&dirty, &game);
        self.team_handler.synchronize_teams(&dirty, &game);
        if dirty.preview {
            self.ui.global::<ui::Shot>().update_shot_preview(&game);
        }
    }

    fn synchronize_phase(self: &Rc<Self>, dirty: &game::Dirty, game: &Game) {
        let game_model = self.ui.global::<ui::GameModel>();
        if dirty.phase {
            game_model.set_game_finished(game.is_finished());
            game_model.set_end_finished(game.is_end_finished());
            game_model.set_thinking(game.is_thinking());
            game_model.set_delivering(game.is_delivering());
            let playing_team = game.playing_team();
            game_model.set_current_team(match playing_team {
                Some(game::team::Team::A) => 0,
                Some(game::team::Team::B) => 1,
                None => 0,
            });
            game_model.set_current_player(match game.delivering_player() {
                Some(player) => player as i32,
                None => -1,
            });
            if let Some(end::Phase::Finished { score }) = game.current_end().map(|e| &e.phase) {
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
                    Duration::from_millis(10),
                    make_callback!(self.update()),
                );
                *self.update_timer.borrow_mut() = Some(timer);
            }
        }
        if dirty.finished_ends_count != 0 || dirty.phase {
            game_model.set_end(if game.is_finished() {
                game.params.ends as i32
            } else {
                game.finished_ends.len() as i32 + 1
            });
        }
    }

    fn synchronize_violations(self: &Rc<Self>, dirty: &game::Dirty, game: &Game) {
        let game_model = self.ui.global::<ui::GameModel>();
        if dirty.phase {
            let violation = game.current_turn().and_then(turn::Current::violation);
            game_model.set_fgz_rule_violated(violation == Some(turn::Violation::FreeGuardRule));
            game_model.set_no_tick_rule_violated(violation == Some(turn::Violation::NoTickRule));
        }
    }

    pub fn update(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        self.game.borrow_mut().update(&mut dirty, time::Instant::now());
        self.synchronize(dirty);
        Ok(())
    }

    pub fn on_deliver(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        let shot = self.ui.global::<ui::Shot>();
        {
            let mut game = self.game.borrow_mut();
            let call = shot.current_call(&game.sheet.parameters);
            game.start_delivery(&mut dirty, call, time::Instant::now())?;
        }
        self.synchronize(dirty);

        Ok(())
    }

    pub fn on_proceed(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        self.game.borrow_mut().proceed(&mut dirty)?;
        self.synchronize(dirty);
        Ok(())
    }

    pub fn on_replace_stones(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        self.game.borrow_mut().replace_stones(&mut dirty)?;
        self.synchronize(dirty);
        Ok(())
    }

    pub fn on_shot_update(self: &Rc<Self>) -> Result<()> {
        let game = self.game.borrow();
        let shot = self.ui.global::<ui::Shot>();
        if shot.get_automatic_weight() {
            let target_y = feet(shot.get_mark_y() as unit::BaseType);
            let velocity = game.sheet.parameters.velocity_for_target_y(target_y);
            if let Some(hog_to_hog) = game.sheet.parameters.hot_to_hog_time_from_velocity(velocity)
            {
                shot.set_hog_to_hog_time(hog_to_hog.get::<second>() as f32);
            }
        }
        shot.update_shot_preview(&game);
        Ok(())
    }
}
