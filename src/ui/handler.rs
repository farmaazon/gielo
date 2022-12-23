pub mod game;
pub mod stone;
pub mod team;

use crate::profiles::Profiles;
use crate::{ui, Game};
use anyhow::Result;
use itertools::Itertools;
use slint::{Color, ComponentHandle, ModelRc, VecModel};
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
    profiles: Profiles,
}

impl Handler {
    pub fn initialize(ui: ui::Main) -> Rc<Self> {
        ui.set_default_game_parameters(Self::default_new_game_parameters());
        let game_model = ui.global::<ui::GameModel>();
        let profiles_ui = ui.global::<ui::Profiles>();
        let profiles = Profiles::load();
        profiles_ui.initialize(&profiles);
        let game = RefCell::new(None);
        let this = Rc::new(Self { ui: ui.clone_strong(), game, profiles });
        game_model.on_start_new_game(make_callback!(this.on_game_start(parameters)));
        game_model.on_finish_game(make_callback!(this.on_game_finish()));
        this
    }

    fn default_new_game_parameters() -> ui::NewGameParameters {
        let default_player = ui::Player { left_handed: false, skill_profile: 0 };
        let default_players = move || {
            ModelRc::new(VecModel::from(
                std::iter::repeat(default_player.clone())
                    .take(crate::game::team::player::PER_TEAM_COUNT)
                    .collect_vec(),
            ))
        };
        ui::NewGameParameters {
            ends: 8,
            ice_profile: 0,
            rules_profile: 0,
            teams: ModelRc::new(VecModel::from(vec![
                ui::NewGameTeam {
                    color: Color::from_rgb_u8(255, 0, 0),
                    name: "Red".into(),
                    players: default_players(),
                },
                ui::NewGameTeam {
                    color: Color::from_rgb_u8(255, 255, 0),
                    name: "Yellow".into(),
                    players: default_players(),
                },
            ])),
        }
    }

    pub fn on_game_start(&self, parameters: ui::NewGameParameters) -> Result<()> {
        let game_model = self.ui.global::<ui::GameModel>();
        let sheet_model = self.ui.global::<ui::SheetModel>();
        let game_params = parameters.game_parameters(&self.profiles)?;
        let sheet_params = parameters.sheet_parameters(&self.profiles)?;
        let teams = parameters.teams(&self.profiles)?;
        let simulation_params = crate::game::simulation::Parameters::default();
        let first_hammer = crate::game::team::Team::A;
        let game = Game::new(teams, game_params, sheet_params, simulation_params, first_hammer);
        let game_handler = game::Handler::initialize(self.ui.clone_strong(), game);
        *self.game.borrow_mut() = Some(game_handler);
        sheet_model.set_parameters(sheet_params);
        game_model.set_game_running(true);
        Ok(())
    }

    pub fn on_game_finish(&self) -> Result<()> {
        *self.game.borrow_mut() = None;
        let game_model = self.ui.global::<ui::GameModel>();
        game_model.set_game_running(false);
        Ok(())
    }
}
