use crate::{
    game,
    game::{
        unit,
        unit::{
            acceleration::foot_per_second_squared, feet, length::foot, seconds, vector::Vector2,
        },
        Game,
    },
    profiles::Profiles,
    ui,
    ui::{SheetEndGeometry, SheetGeometry, StoneModel},
};
use anyhow::{anyhow, Result};
use itertools::Itertools;
use slint::{Model, ModelRc, VecModel};
use std::{any::Any, cell::RefCell, rc::Rc};

#[cfg(debug_assertions)]
const PREVIEW_STEPS: usize = 10;

#[cfg(not(debug_assertions))]
const PREVIEW_STEPS: usize = 4;

pub fn initialize_profiles(ui: &ui::Profiles, profiles: &Profiles) {
    let player = ModelRc::new(VecModel::from(profiles.player_skills_names().collect_vec()));
    let rules = ModelRc::new(VecModel::from(profiles.rule_set_names().collect_vec()));
    let ice = ModelRc::new(VecModel::from(profiles.ice_profile_names().collect_vec()));
    let teams = ModelRc::new(VecModel::from(profiles.teams().collect_vec()));
    ui.set_player_skills(player);
    ui.set_rules(rules);
    ui.set_ice(ice);
    ui.set_teams(teams);
}

pub fn set_ui_sheet_parameters(ui: &ui::SheetModel, params: game::sheet::Parameters) {
    let make_end_geometry = |geom: game::sheet::parameters::EndGeometry| SheetEndGeometry {
        back_y: geom.back_line_y.get::<foot>() as f32,
        tee_y: geom.tee_line_y.get::<foot>() as f32,
        hog_y: geom.hog_line_y.get::<foot>() as f32,
    };
    let geometry = SheetGeometry {
        width: params.geometry.width.get::<foot>() as f32,
        height: params.geometry.length.get::<foot>() as f32,
        house_radius: params.geometry.house_radius.get::<foot>() as f32,
        hack_offset: params.geometry.hack_x_offset.get::<foot>() as f32,
        tee_x: (params.geometry.center_line_x + params.geometry.width / 2.0).get::<foot>() as f32,
        delivery_end: make_end_geometry(params.geometry.delivery_end),
        playing_end: make_end_geometry(params.geometry.playing_end),
    };
    ui.set_geometry(geometry);
    ui.set_friction(params.friction.get::<foot_per_second_squared>() as f32);
    ui.set_rotation_acc(params.rotation_acc.get::<foot_per_second_squared>() as f32);
    ui.set_stone_radius(params.stone_radius.get::<foot>() as f32);
}

pub mod new_game_parameters {
    use super::*;
    use gielo_game::sheet::team::PerTeam;

    pub fn skills(ui: ui::PlayerSkills) -> game::player::Skills {
        game::player::Skills::from_tee_shot_std_dev(
            feet(ui.x_std_dev as unit::BaseType),
            feet(ui.y_std_dev as unit::BaseType),
            game::sheet::Parameters::default(),
        )
    }

    pub fn player(ui: ui::Player) -> game::player::Player {
        game::player::Player {
            name: ui.name,
            skills: skills(ui.skills),
            used_hack: match ui.left_handed {
                true => game::sheet::Hack::Right,
                false => game::sheet::Hack::Left,
            },
        }
    }

    pub fn team(ui: ui::Team) -> Result<game::TeamInfo> {
        let players: Vec<_> = ui.players.iter().map(player).collect();
        Ok(game::TeamInfo {
            name: ui.name,
            color: ui.color,
            players: players.try_into().map_err(|_| anyhow!("Wrong number of players"))?,
        })
    }

    pub fn game_parameters(
        ui: &ui::NewGameParameters,
        profiles: &Profiles,
    ) -> Result<game::Parameters> {
        Ok(game::Parameters {
            ends: ui.ends as u8,
            rules: profiles
                .rule_set
                .get(ui.rules_profile as usize)
                .ok_or_else(|| anyhow!("Unknown rule set \"{}\"", ui.rules_profile))?
                .data,
            ..game::Parameters::default()
        })
    }

    pub fn sheet_parameters(
        ui: &ui::NewGameParameters,
        profiles: &Profiles,
    ) -> Result<game::sheet::Parameters> {
        let profile = &profiles
            .ice_profile
            .get(ui.ice_profile as usize)
            .ok_or_else(|| anyhow!("Unknown ice profile \"{}\"", ui.ice_profile))?
            .data;
        let geometry =
            game::sheet::parameters::Geometry { width: profile.sheet_width, ..Default::default() };
        let parameters = game::sheet::Parameters {
            geometry,
            stone_radius: profile.stones_circumference / 2.0 / unit::base_type::consts::PI,
            static_friction: profile.static_friction,
            ..Default::default()
        };
        Ok(parameters.with_tee_shot_parameters(profile.tee_shot_hog_to_hog, profile.curling))
    }

    pub fn teams(ui: &ui::NewGameParameters) -> Result<PerTeam<game::TeamInfo>> {
        let teams: Result<Vec<_>> = ui.teams.iter().map(team).collect();
        let array: [_; game::sheet::team::TEAMS_COUNT] =
            teams?.try_into().map_err(|_| anyhow!("Wrong number of teams"))?;
        Ok(array.into())
    }
}

pub struct Stones {
    game: Rc<RefCell<Game>>,
    pub notify: slint::ModelNotify,
}

impl Stones {
    pub fn new(game: Rc<RefCell<Game>>) -> Self {
        Self { game, notify: slint::ModelNotify::default() }
    }
}

impl Model for Stones {
    type Data = StoneModel;

    fn row_count(&self) -> usize {
        game::sheet::stone::COUNT
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let game = self.game.borrow();
        let stones = &game.sheet.stones;
        let position_feet = if stones.in_play().contains(row) {
            stones.positions()[row].map(|x| x.get::<foot>())
        } else {
            Vector2 { x: -100.0, y: -100.0 }
        };
        let team = game::sheet::stone::team(row);
        Some(StoneModel {
            x: position_feet.x as f32,
            y: position_feet.y as f32,
            color: game.teams[team].color,
        })
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
pub fn call(ui: &ui::Shot, sheet: &game::sheet::Parameters) -> game::turn::delivery::Call {
    game::turn::delivery::Call {
        weight: if ui.get_automatic_weight() {
            sheet.velocity_for_target_y(feet(ui.get_mark_y() as unit::BaseType))
        } else {
            sheet.velocity_for_hog_to_hog_time(seconds(ui.get_hog_to_hog_time() as unit::BaseType))
        },
        mark: Vector2 {
            x: feet(ui.get_mark_x() as unit::BaseType),
            y: feet(ui.get_mark_y() as unit::BaseType),
        },
        rotation: if ui.get_clockwise() {
            game::sheet::stone::Rotation::Clockwise
        } else {
            game::sheet::stone::Rotation::CounterClockwise
        },
    }
}

pub fn update_shot_preview(ui: &ui::Shot, game: &Game) {
    let call = call(ui, &game.sheet.parameters);
    let current_team_color = game.playing_team().map(|team| game.teams[team].color);
    let preview = game.expected_path(call).unwrap_or_default();
    let commands = format!(
        "{}",
        preview.iter().step_by(PREVIEW_STEPS).enumerate().format_with(" ", |(index, pos), f| {
            f(&format_args!(
                "{} {} {}",
                if index % 2 == 1 { "L" } else { "M" },
                pos.x.get::<foot>(),
                pos.y.get::<foot>()
            ))
        })
    );
    ui.set_preview_commands(commands.into());
    let last = preview.last().copied().unwrap_or_default();
    ui.set_preview_result(ui::StoneModel {
        color: current_team_color.unwrap_or_default(),
        x: last.x.get::<foot>() as f32,
        y: last.y.get::<foot>() as f32,
    });
}
