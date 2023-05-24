use crate::game;
use crate::unit::{feet, feet_per_second_squared, Length};
use slint::SharedString;

pub struct PlayerSkills {
    pub x_std_dev: Length,
    pub y_std_dev: Length,
}

pub mod builtin {
    use super::*;

    pub fn players() -> impl IntoIterator<Item = (&'static str, PlayerSkills)> {
        [
            ("Custom", PlayerSkills { x_std_dev: feet(0.0), y_std_dev: feet(0.0) }),
            ("Beginner", PlayerSkills { x_std_dev: feet(4.0), y_std_dev: feet(8.0) }),
            ("Top Player", PlayerSkills { x_std_dev: feet(0.5), y_std_dev: feet(1.0) }),
            ("Ideal", PlayerSkills { x_std_dev: feet(0.0), y_std_dev: feet(0.0) }),
        ]
    }

    pub fn ice_profiles() -> impl IntoIterator<Item = (&'static str, game::sheet::Parameters)> {
        [
            ("Standard", game::sheet::Parameters::default()),
            (
                "Less-Curling",
                game::sheet::Parameters {
                    rotation_acc: feet_per_second_squared(0.015),
                    ..game::sheet::Parameters::default()
                },
            ),
        ]
    }

    pub fn rule_sets() -> impl IntoIterator<Item = (&'static str, game::Rules)> {
        [
            ("Standard", game::Rules { free_guard_rule_stones: 5, no_tick_rule_stones: 5 }),
            (
                "Without no-tick rule",
                game::Rules { free_guard_rule_stones: 5, no_tick_rule_stones: 0 },
            ),
            ("4 Free guards", game::Rules { free_guard_rule_stones: 4, no_tick_rule_stones: 0 }),
            ("No free guards", game::Rules { free_guard_rule_stones: 0, no_tick_rule_stones: 0 }),
        ]
    }
}

pub struct Profile<T> {
    pub name: SharedString,
    pub data: T,
}

pub struct Profiles {
    pub player_skills: Vec<Profile<PlayerSkills>>,
    pub ice_profile: Vec<Profile<game::sheet::Parameters>>,
    pub rule_set: Vec<Profile<game::Rules>>,
}

impl Profiles {
    pub fn load() -> Self {
        Self {
            player_skills: Self::builtin_list_to_map(builtin::players()),
            ice_profile: Self::builtin_list_to_map(builtin::ice_profiles()),
            rule_set: Self::builtin_list_to_map(builtin::rule_sets()),
        }
    }

    fn builtin_list_to_map<'a, T>(list: impl IntoIterator<Item = (&'a str, T)>) -> Vec<Profile<T>> {
        list.into_iter().map(|(name, data)| Profile { name: name.into(), data }).collect()
    }

    fn names_iterator<'a, Ts: 'a, T: 'a>(list: Ts) -> impl Iterator<Item = SharedString> + 'a
    where
        Ts: IntoIterator<Item = &'a Profile<T>>,
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
}
