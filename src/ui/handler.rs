pub mod game;
pub mod stone;
pub mod team;

use crate::{game::sheet, profiles::Profiles, ui, Game};
use anyhow::{anyhow, Result};
use itertools::Itertools;
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};
use uom::si::length::foot;

const DEFAULT_PLAYER_NAMES: [&str; crate::game::team::player::PER_TEAM_COUNT] =
    ["Lead", "Second", "Third", "Fourth"];

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
    pub fn initialize(ui: ui::Main, project_dirs: &Result<directories::ProjectDirs>) -> Rc<Self> {
        ui.set_default_game_parameters(Self::default_new_game_parameters());
        let game_model = ui.global::<ui::GameModel>();
        let profiles_ui = ui.global::<ui::Profiles>();
        let sheet_ui = ui.global::<ui::SheetModel>();
        sheet_ui.set_parameters(sheet::Parameters::default());
        let profiles = Profiles::load(project_dirs);
        profiles_ui.initialize(&profiles);
        let game = RefCell::new(None);
        let this = Rc::new(Self { ui: ui.clone_strong(), game, profiles });
        profiles_ui.on_load_team_name(make_callback!(this.load_team_name(index)));
        profiles_ui.on_load_team_players(make_callback!(this.load_team_players(index)));
        profiles_ui.on_load_player_skills(make_callback!(this.load_player_skills(index)));
        profiles_ui.on_custom_player_skills_label(|skills| {
            format!("Weight: ±{} ft, angle ±{} ft", skills.y_std_dev, skills.x_std_dev).into()
        });
        game_model.on_start_new_game(make_callback!(this.on_game_start(parameters)));
        game_model.on_finish_game(make_callback!(this.on_game_finish()));
        this
    }

    fn default_new_game_parameters() -> ui::NewGameParameters {
        let default_player = |name: &str| ui::Player {
            name: name.into(),
            left_handed: false,
            skills: ui::PlayerSkills { x_std_dev: 1.0, y_std_dev: 2.0 },
        };
        let default_players = move || {
            ModelRc::new(VecModel::from(
                DEFAULT_PLAYER_NAMES.iter().copied().map(default_player).collect_vec(),
            ))
        };
        ui::NewGameParameters {
            ends: 8,
            ice_profile: 0,
            rules_profile: 0,
            teams: ModelRc::new(VecModel::from(vec![
                ui::Team {
                    color: Color::from_rgb_u8(255, 0, 0),
                    name: "Red".into(),
                    players: default_players(),
                },
                ui::Team {
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
        let teams = parameters.teams()?;
        let simulation_params = crate::game::simulation::Parameters::default();
        let first_hammer = crate::game::team::Team::A;
        // Sheet needs to be updated before game.
        sheet_model.set_parameters(sheet_params);
        let game = Game::new(teams, game_params, sheet_params, simulation_params, first_hammer);
        let game_handler = game::Handler::initialize(self.ui.clone_strong(), game);
        *self.game.borrow_mut() = Some(game_handler);
        game_model.set_game_running(true);
        Ok(())
    }

    pub fn on_game_finish(&self) -> Result<()> {
        *self.game.borrow_mut() = None;
        let game_model = self.ui.global::<ui::GameModel>();
        game_model.set_game_running(false);
        Ok(())
    }

    pub fn load_team_name(&self, index: i32) -> Result<SharedString> {
        self.profiles
            .teams
            .get(index as usize)
            .map(|team| team.name.as_str().into())
            .ok_or(anyhow!("Wrong index of team skill"))
    }

    pub fn load_team_players(&self, index: i32) -> Result<slint::ModelRc<ui::Player>> {
        let players = &self
            .profiles
            .teams
            .get(index as usize)
            .ok_or(anyhow!("Wrong index of player skill profile: {index}"))?
            .data
            .players;
        let ui_players: Vec<_> = players
            .iter()
            .map(|p| ui::Player {
                left_handed: false,
                name: p.name.as_str().into(),
                skills: ui::PlayerSkills {
                    x_std_dev: p.skills.x_std_dev.get::<foot>() as f32,
                    y_std_dev: p.skills.y_std_dev.get::<foot>() as f32,
                },
            })
            .collect();
        Ok(ModelRc::new(VecModel::from(ui_players)))
    }

    pub fn load_player_skills(&self, index: i32) -> Result<ui::PlayerSkills> {
        let profile = self
            .profiles
            .player_skills
            .get(index as usize)
            .ok_or(anyhow!("Wrong index of player skill profile: {index}"))?;
        Ok(ui::PlayerSkills {
            x_std_dev: profile.data.x_std_dev.get::<foot>() as f32,
            y_std_dev: profile.data.y_std_dev.get::<foot>() as f32,
        })
    }
}
