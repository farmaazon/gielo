use crate::game::stone::Rotation;
use crate::ui::handler::{make_callback, stone, team};
use crate::unit::{feet, seconds};
use crate::vector::Vector2;
use crate::{game, ui, Game};
use anyhow::Result;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::time;
use std::time::Duration;

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
        let sheet_model = ui.global::<ui::SheetModel>();
        let team_handler = team::Handler::new(&game, &game_model);
        let game = Rc::new(RefCell::new(game));
        let stones_model = Rc::new(ui::model::Stones::new(game.clone()));
        let stone_handler = stone::Handler::new(stones_model.clone());
        let update_timer = RefCell::new(None);
        sheet_model.set_stones(stones_model.clone().into());
        let this = Rc::new(Self {
            ui: ui.clone_strong(),
            game,
            stone_handler,
            team_handler,
            update_timer,
        });
        let to_initialize =
            game::Dirty { stone_count: 0, stones: u16::MAX, score: true, stage: true };
        this.synchronize(to_initialize);
        game_model.on_deliver(make_callback!(this.on_deliver()));
        game_model.on_finish_end(make_callback!(this.on_finish_end()));
        this
    }

    pub fn synchronize(self: &Rc<Self>, dirty: game::Dirty) {
        let game = self.game.borrow();
        self.synchronize_stage(&dirty, &*game);
        self.stone_handler.synchronize(&dirty, &*game);
        self.team_handler.synchronize_score(&dirty, &*game);
        self.team_handler.synchronize_teams(&dirty, &*game);
    }

    fn synchronize_stage(self: &Rc<Self>, dirty: &game::Dirty, game: &Game) {
        if dirty.stage {
            let game_model = self.ui.global::<ui::GameModel>();
            match game.current_end_stage() {
                Some(end) => {
                    game_model.set_end(end.no as i32);
                    game_model.set_current_team(match end.playing_team {
                        game::team::Team::A => 0,
                        game::team::Team::B => 1,
                    });
                }
                None => {
                    game_model.set_end(game.params.ends as i32);
                }
            }
            let delivering = matches!(&game.stage, game::Stage::Delivering { .. });
            if !delivering {
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
            game_model.set_delivering(delivering);
            game_model.set_end_finished(matches!(&game.stage, game::Stage::EndConcluded(_, _)));
            game_model.set_game_finished(matches!(&game.stage, game::Stage::GameConcluded(_)));
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
        let call = game::shot::Call {
            weight: seconds(shot.get_weight_sec()),
            mark: Vector2 { x: feet(shot.get_mark_x()), y: feet(shot.get_mark_y()) },
            rotation: if shot.get_clockwise() {
                Rotation::Clockwise
            } else {
                Rotation::CounterClockwise
            },
        };
        self.game.borrow_mut().start_delivery(&mut dirty, time::Instant::now(), call)?;
        self.synchronize(dirty);

        Ok(())
    }

    pub fn on_finish_end(self: &Rc<Self>) -> Result<()> {
        let mut dirty = game::Dirty::new();
        self.game.borrow_mut().finish_end(&mut dirty)?;
        self.synchronize(dirty);
        Ok(())
    }
}
