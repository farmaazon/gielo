use crate::game::stone::Flag;
use crate::game::turn::Phase;
use crate::game::{end, turn};
use crate::ui::handler::{make_callback, stone, team};
use crate::ui::StoneModel;
use crate::{game, ui, Game};
use anyhow::Result;
use itertools::Itertools;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::time;
use std::time::Duration;
use uom::si::length::foot;

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
        ui.global::<ui::Shot>().on_update_preview(make_callback!(this.on_shot_update()));
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
            self.update_shot_preview(&game);
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
            let violations = game
                .current_turn()
                .and_then(|turn| match &turn.phase {
                    Phase::Finished { violations, .. } => Some(*violations),
                    _ => None,
                })
                .unwrap_or_default();
            game_model.set_fgz_rule_violated(violations.contains(turn::Violation::FreeGuardRule));
            game_model.set_no_tick_rule_violated(violations.contains(turn::Violation::NoTickRule));
        }
    }

    pub fn update_shot_preview(self: &Rc<Self>, game: &Game) {
        let shot = self.ui.global::<ui::Shot>();
        let call = shot.current_call();
        let current_team_color = game.playing_team().map(|team| game.teams[team].color);
        let preview = game.expected_path(call).unwrap_or_default();
        let commands = format!(
            "{}",
            preview.iter().enumerate().format_with(" ", |(index, pos), f| {
                f(&format_args!(
                    "{} {} {}",
                    if index % 2 == 1 { "L" } else { "M" },
                    pos.x.get::<foot>(),
                    pos.y.get::<foot>()
                ))
            })
        );
        shot.set_preview_commands(commands.into());
        let last = preview.last().copied().unwrap_or_default();
        shot.set_preview_result(StoneModel {
            color: current_team_color.unwrap_or_default(),
            x: last.x.get::<foot>(),
            y: last.y.get::<foot>(),
        });
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
        let call = shot.current_call();
        self.game.borrow_mut().start_delivery(&mut dirty, call, time::Instant::now())?;
        self.synchronize(dirty);

        Ok(())
    }

    pub fn on_proceed(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        self.game.borrow_mut().proceed(&mut dirty)?;
        self.synchronize(dirty);
        Ok(())
    }

    pub fn on_shot_update(self: &Rc<Self>) -> Result<()> {
        self.update_shot_preview(&self.game.borrow());
        Ok(())
    }
}
