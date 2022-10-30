pub mod game;
pub mod stone;
pub mod team;

use crate::{ui, Game};
use anyhow::Result;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

macro_rules! make_callback {
    ($this:ident.$method:ident($($arg:ident),*)) => {
        {
            let weak = Rc::downgrade(&$this);
            move |$($arg),*| {
                if let Some(this) = weak.upgrade() {
                    this.$method($($arg),*).unwrap()
                } else {
                    Default::default()
                }
            }
        }
    }
}

pub(crate) use make_callback;

pub struct Handler {
    ui: ui::Main,
    game: RefCell<Option<Rc<game::Handler>>>,
}

impl Handler {
    pub fn initialize(ui: ui::Main) -> Rc<Self> {
        let game_model = ui.global::<ui::GameModel>();
        game_model.initialize();
        ui.global::<ui::SheetModel>().initialize();
        let game = RefCell::new(None);
        let this = Rc::new(Self { ui: ui.clone_strong(), game });
        game_model.on_start_new_game(make_callback!(this.on_game_start()));
        this
    }

    pub fn on_game_start(&self) -> Result<()> {
        let game_model = self.ui.global::<ui::GameModel>();
        let params = game_model.parameters();
        let teams = game_model.teams_info()?;
        // let sheet_params = self.ui.global::<ui::SheetModel>().parameters();
        let sheet_params = crate::game::sheet::Parameters::default();
        let first_hammer = crate::game::team::Team::A;
        let game = Game::new(teams, params, sheet_params, first_hammer);
        let game_handler = game::Handler::initialize(self.ui.clone_strong(), game);
        *self.game.borrow_mut() = Some(game_handler);
        Ok(())
    }
}
