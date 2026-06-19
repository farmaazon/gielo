use crate::game::unit::{AvailableEnergy, Length, Time};
use ::serde::{Deserialize, Serialize};
use anyhow::Result;
use slint::SharedString;
use std::fs;

mod serde;

pub mod builtin;
#[derive(Deserialize, Serialize)]
pub struct PlayerSkills {
    pub x_std_dev: Length,
    pub y_std_dev: Length,
}

#[derive(Deserialize, Serialize)]
pub struct Ice {
    pub tee_shot_hog_to_hog: Time,
    pub curling: Length,
    pub sheet_width: Length,
    pub stones_circumference: Length,
    pub static_friction: AvailableEnergy,
}

#[derive(Deserialize, Serialize)]
pub struct Rules {
    pub free_guard_rule_stones: usize,
    pub no_tick_rule_stones: usize,
}

use crate::save_load::is_file_not_found;

#[derive(Deserialize, Serialize)]
pub struct Player {
    pub name: SharedString,
    pub skills: PlayerSkills,
}

#[derive(Deserialize, Serialize)]
pub struct Team {
    pub players: [Player; 4],
}

// Deserialize and Serialize in `serde` module.
pub struct Profile<T> {
    pub name: SharedString,
    pub data: T,
}

#[derive(Deserialize, Serialize)]
pub struct Profiles {
    pub player_skills: Vec<Profile<PlayerSkills>>,
    pub ice_profile: Vec<Profile<Ice>>,
    pub rule_set: Vec<Profile<Rules>>,
    pub teams: Vec<Profile<Team>>,
}

impl Profiles {
    pub fn load_from_or_create_file(path: &std::path::Path) -> Self {
        let path_str = path.to_string_lossy();
        match Self::load_from_file(path) {
            Ok(profiles) => {
                log::info!("Profiles loaded from '{path_str}'");
                profiles
            }
            Err(err) if is_file_not_found(&err) => {
                let profiles = builtin::create();
                log::info!("Profiles file '{path_str}' not found. Generating from builtin.");
                if let Err(error) = profiles.save_to_file(path) {
                    log::error!("Failed to generate profiles file '{path_str}': {error}.");
                }
                profiles
            }
            Err(err) => {
                log::error!("Failed to load profiles from '{path_str}': {err}.");
                builtin::create()
            }
        }
    }

    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let file = fs::File::open(path)?;
        Ok(serde_yaml::from_reader(file)?)
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?
        }
        let file = fs::File::create(path)?;
        serde_yaml::to_writer(file, self)?;
        Ok(())
    }

    fn names_iterator<'a, Ts, T: 'a>(list: Ts) -> impl Iterator<Item = SharedString> + 'a
    where
        Ts: IntoIterator<Item = &'a Profile<T>> + 'a,
        Ts::IntoIter: 'a,
    {
        list.into_iter().map(|Profile { name, .. }| name.clone())
    }

    pub fn player_skills_names(&self) -> impl Iterator<Item = SharedString> + '_ {
        Self::names_iterator(&self.player_skills)
    }

    pub fn ice_profile_names(&self) -> impl Iterator<Item = SharedString> + '_ {
        Self::names_iterator(&self.ice_profile)
    }

    pub fn rule_set_names(&self) -> impl Iterator<Item = SharedString> + '_ {
        Self::names_iterator(&self.rule_set)
    }

    pub fn teams(&self) -> impl Iterator<Item = SharedString> + '_ {
        Self::names_iterator(&self.teams)
    }
}
