use crate::{
    profiles::{Ice, Player, PlayerSkills, Profile, Profiles, Rules, Team},
    unit::{feet, feet_squared_per_second_squared, inches, seconds},
};

macro_rules! list {
    ($($name:literal: $data:expr),*$(,)?) => {
        vec![$(Profile { name: $name.into(), data: $data}),*]
    }
}

pub fn create() -> Profiles {
    Profiles {
        player_skills: list![
            "Beginner": PlayerSkills { x_std_dev: feet(4.0), y_std_dev: feet(8.0) },
            "Top Player": PlayerSkills { x_std_dev: feet(0.5), y_std_dev: feet(1.0) },
            "Ideal": PlayerSkills { x_std_dev: feet(0.0), y_std_dev: feet(0.0) },
        ],
        ice_profile: list![
            "Standard": Ice {
                tee_shot_hog_to_hog: seconds(14.5),
                curling: feet(5.0),
                sheet_width: feet(15.0) + inches(7.0),
                stones_circumference: inches(36.0),
                static_friction: feet_squared_per_second_squared(0.25),
            },
            "Rough": Ice {
                tee_shot_hog_to_hog: seconds(13.0),
                curling: feet(3.0),
                sheet_width: feet(15.0) + inches(7.0),
                stones_circumference: inches(36.0),
                static_friction: feet_squared_per_second_squared(0.25),
            },
        ],
        rule_set: list![
            "Standard": Rules { free_guard_rule_stones: 5, no_tick_rule_stones: 5 },
            "Without no-tick rule": Rules { free_guard_rule_stones: 5, no_tick_rule_stones: 0 },
            "4 Free guards": Rules { free_guard_rule_stones: 4, no_tick_rule_stones: 0 },
            "No free guards": Rules { free_guard_rule_stones: 0, no_tick_rule_stones: 0 },
        ],
        teams: list![
            "Newcomers": Team { players: [
                Player {
                    name: "Lead".to_owned(),
                    skills: PlayerSkills { x_std_dev: feet(4.0), y_std_dev: feet(8.0) }
                },
                Player {
                    name: "Second".to_owned(),
                    skills: PlayerSkills { x_std_dev: feet(3.7), y_std_dev: feet(7.0) }
                },
                Player {
                    name: "Third".to_owned(),
                    skills: PlayerSkills { x_std_dev: feet(3.3), y_std_dev: feet(6.0) }
                },
                Player {
                    name: "Skip".to_owned(),
                    skills: PlayerSkills { x_std_dev: feet(3.0), y_std_dev: feet(5.0) }
                },
            ] }
        ],
    }
}
