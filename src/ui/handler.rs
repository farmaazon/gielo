pub mod game;
pub mod stone;
pub mod team;

use crate::{
    Snapshot,
    game::RunningGame,
    profiles::{self, Profiles},
    save_load::SaveLoad,
    ui,
};
use anyhow::{Result, anyhow};
use itertools::Itertools;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};

const DEFAULT_PLAYER_NAMES: [&str; crate::game::team::player::PER_TEAM_COUNT] =
    ["Lead", "Second", "Third", "Fourth"];

macro_rules! make_callback {
    ($this:ident.$method:ident($($arg:ident),*)) => {
        {
            let weak = Rc::downgrade(&$this);
            move |$($arg),*| {
                if let Some(this) = weak.upgrade() {
                    let error_ui = this.ui.global::<$crate::ui::Error>();
                    match (this.$method($($arg),*)) {
                        Ok(value) => {
                            error_ui.invoke_report_success();
                            value
                        }
                        Err(err) => {
                            error_ui.invoke_report_error(err.to_string().into());
                            Default::default()
                        }
                    }
                } else {
                    Default::default()
                }
            }
        }
    }
}

use crate::{
    game::unit::length::foot,
    ui::model::{initialize_profiles, new_game_parameters, set_ui_sheet_parameters},
};
pub(crate) use make_callback;

pub struct Handler {
    ui: ui::Main,
    game: RefCell<Option<Rc<game::Handler>>>,
    snapshot: Rc<Snapshot>,
    profiles: Profiles,
    save_load: RefCell<SaveLoad>,
    saves_model: Rc<VecModel<SharedString>>,
    _blinking: slint::Timer,
}

impl Handler {
    pub fn initialize(ui: ui::Main, game: Option<RunningGame>, snapshot: Rc<Snapshot>) -> Rc<Self> {
        let save_load = SaveLoad::new();
        ui.set_default_game_parameters(Self::default_new_game_parameters());
        let game_model = ui.global::<ui::GameModel>();
        let profiles_ui = ui.global::<ui::Profiles>();
        let sheet_ui = ui.global::<ui::SheetModel>();
        let save_load_ui = ui.global::<ui::SaveLoad>();
        set_ui_sheet_parameters(&sheet_ui, &crate::game::sheet::Parameters::default());
        let profiles = if cfg!(target_family = "wasm") {
            profiles::builtin::create()
        } else {
            save_load.load_profiles()
        };
        initialize_profiles(&profiles_ui, &profiles);
        let game_handler = game.map(|game| {
            game::Handler::initialize(ui.clone_strong(), game.clone(), snapshot.clone())
        });
        let saves_model = Rc::new(VecModel::from(Self::saves_vec(&save_load)));
        if !cfg!(target_family = "wasm") {
            save_load_ui.set_enabled(true);
            save_load_ui.set_saves(saves_model.clone().into());
        }
        
        let weak_ui = ui.as_weak();
        let blinking = slint::Timer::default();
        blinking.start(slint::TimerMode::Repeated, Duration::from_millis(500), move || {
            if let Some(ui) = weak_ui.upgrade() {
                ui.set_blinking_stone_visible(!ui.get_blinking_stone_visible());
            }
        });
        let this = Rc::new(Self {
            ui: ui.clone_strong(),
            game: RefCell::new(game_handler),
            snapshot,
            profiles,
            save_load: RefCell::new(save_load),
            saves_model,
            _blinking: blinking,
        });
        profiles_ui.on_load_team_name(make_callback!(this.load_team_name(index)));
        profiles_ui.on_load_team_players(make_callback!(this.load_team_players(index)));
        profiles_ui.on_load_player_skills(make_callback!(this.load_player_skills(index)));
        profiles_ui.on_custom_player_skills_label(|skills| {
            format!("Weight: ±{} ft, angle ±{} ft", skills.y_std_dev, skills.x_std_dev).into()
        });
        game_model.on_start_new_game(make_callback!(this.on_game_start(parameters)));
        game_model.on_finish_game(make_callback!(this.on_game_finish()));
        save_load_ui.on_save(make_callback!(this.save_game()));
        save_load_ui.on_load(make_callback!(this.load_game(save_index)));
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
        let game_params = new_game_parameters::game_parameters(&parameters, &self.profiles)?;
        let game = RunningGame::new(game_params);
        let game_handler =
            game::Handler::initialize(self.ui.clone_strong(), game, self.snapshot.clone());
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

    pub fn save_game(&self) -> Result<()> {
        let game = self.game.borrow();
        let running = game.as_ref().ok_or_else(|| anyhow!("Cannot save game when not running!"))?;
        let mut save_load = self.save_load.borrow_mut();
        let new_save = running.save(&mut save_load)?;
        self.saves_model.insert(0, new_save.name.clone());
        Ok(())
    }

    pub fn load_game(&self, model_index: i32) -> Result<()> {
        let index = self.saves_model.row_count() - model_index as usize - 1;
        let new_game = self.save_load.borrow().load_game(index)?;
        let running = RunningGame::load(new_game);
        let new_game_handler =
            game::Handler::initialize(self.ui.clone_strong(), running, self.snapshot.clone());
        self.game.replace(Some(new_game_handler));
        Ok(())
    }

    pub fn update_save_list(&self) {
        let mut save_load = self.save_load.borrow_mut();
        if let Err(err) = save_load.reload_saves_list() {
            log::error!("Could not refresh save list: {err}");
        }
        self.saves_model.set_vec(Self::saves_vec(&save_load))
    }

    fn saves_vec(save_load: &SaveLoad) -> Vec<SharedString> {
        save_load.known_saves.iter().map(|save| save.name.clone()).rev().collect()
    }
}
