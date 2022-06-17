use crate::game::stone::Rotation;
use crate::ui::handler::{make_callback, team};
use crate::unit::{feet, seconds};
use crate::vector::Vector2;
use crate::{game, ui, Game};
use anyhow::Result;
use slint::ComponentHandle;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::time;
use std::time::Duration;

pub struct Handler {
    ui: ui::Main,
    stones_model: Rc<ui::model::Stones>,
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
        let update_timer = RefCell::new(None);
        sheet_model.set_stones(stones_model.clone().into());
        let this =
            Rc::new(Self { ui: ui.clone_strong(), game, team_handler, stones_model, update_timer });
        this.synchronize_stage(&mut this.game.borrow_mut());
        game_model.on_deliver(make_callback!(this.on_deliver()));
        game_model.on_finish_end(make_callback!(this.on_finish_end()));
        this
    }

    pub fn synchronize_stage(&self, game: &mut RefMut<Game>) {
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
        game_model.set_delivering(matches!(&game.stage, game::Stage::Delivering { .. }));
        game_model.set_end_finished(matches!(&game.stage, game::Stage::EndConcluded(_, _)));
        game_model.set_game_finished(matches!(&game.stage, game::Stage::GameConcluded(_)));
        self.team_handler.synchronize_teams(&*game);
    }

    pub fn update(&self) -> Result<()> {
        let mut game = self.game.borrow_mut();
        game.update(time::Instant::now());
        self.stones_model.synchronize(&mut game);
        let still_delivering = matches!(game.stage, game::Stage::Delivering { .. });
        if !still_delivering {
            self.team_handler.synchronize_score(&mut *game);
            self.synchronize_stage(&mut game);
            if let Some(timer) = self.update_timer.take() {
                timer.stop();
            }
        }
        Ok(())
    }

    pub fn on_deliver(self: Rc<Self>) -> Result<()> {
        let mut game = self.game.borrow_mut();
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
        game.start_delivery(time::Instant::now(), call)?;
        let game_model = self.ui.global::<ui::GameModel>();
        game_model.set_delivering(true);
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(10),
            make_callback!(self.update()),
        );
        *self.update_timer.borrow_mut() = Some(timer);
        Ok(())
    }

    pub fn on_finish_end(&self) -> Result<()> {
        let mut game = self.game.borrow_mut();
        game.finish_end()?;
        self.stones_model.synchronize(&mut game);
        self.synchronize_stage(&mut game);
        Ok(())
    }
}
