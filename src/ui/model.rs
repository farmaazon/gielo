use crate::game;
use crate::game::team::PerTeam;
use crate::game::Game;
use crate::profiles::Profiles;
use crate::ui;
use crate::ui::{SheetEndGeometry, SheetGeometry, StoneModel};
use crate::unit::{feet, seconds};
use crate::vector::Vector2;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use slint::VecModel;
use slint::{Model, ModelRc};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;

#[cfg(debug_assertions)]
const PREVIEW_STEPS: usize = 4;

#[cfg(not(debug_assertions))]
const PREVIEW_STEPS: usize = 2;

impl<'a> ui::Profiles<'a> {
    pub fn initialize(&self, profiles: &Profiles) {
        let player = ModelRc::new(VecModel::from(profiles.player_skills_names().collect_vec()));
        let rules = ModelRc::new(VecModel::from(profiles.rule_set_names().collect_vec()));
        let ice = ModelRc::new(VecModel::from(profiles.ice_profile_names().collect_vec()));
        self.set_player_skills(player);
        self.set_rules(rules);
        self.set_ice(ice);
    }
}

impl<'a> ui::SheetModel<'a> {
    pub fn set_parameters(&self, params: game::sheet::Parameters) {
        let make_end_geometry = |geom: game::sheet::parameters::EndGeometry| SheetEndGeometry {
            back_y: geom.back_line_y.get::<foot>(),
            tee_y: geom.tee_line_y.get::<foot>(),
            hog_y: geom.hog_line_y.get::<foot>(),
        };
        let geometry = SheetGeometry {
            width: params.geometry.width.get::<foot>(),
            height: params.geometry.length.get::<foot>(),
            house_radius: params.geometry.house_radius.get::<foot>(),
            hack_offset: params.geometry.hack_x_offset.get::<foot>(),
            tee_x: (params.geometry.center_line_x + params.geometry.width / 2.0).get::<foot>(),
            delivery_end: make_end_geometry(params.geometry.delivery_end),
            playing_end: make_end_geometry(params.geometry.playing_end),
        };
        self.set_geometry(geometry);
        self.set_friction(params.friction.get::<foot_per_second_squared>());
        self.set_rotation_acc(params.rotation_acc.get::<foot_per_second_squared>());
        self.set_stone_radius(params.stone_radius.get::<foot>());
    }
}

impl ui::PlayerSkills {
    pub fn game_skills(self) -> game::team::player::Skills {
        game::team::player::Skills::from_tee_shot_std_dev(
            feet(self.x_std_dev),
            feet(self.y_std_dev),
            game::sheet::Parameters::default(),
        )
    }
}

impl ui::Player {
    pub fn player_info(self) -> game::team::Player {
        game::team::Player {
            skills: self.skills.game_skills(),
            used_hack: match self.left_handed {
                true => game::sheet::Hack::Right,
                false => game::sheet::Hack::Left,
            },
        }
    }
}

impl ui::NewGameTeam {
    pub fn team_info(self) -> Result<game::team::Info> {
        let players: Vec<_> =
            self.players.iter().map(|ui_player| ui_player.player_info()).collect();
        Ok(game::team::Info {
            name: self.name,
            color: self.color,
            players: players.try_into().map_err(|_| anyhow!("Wrong number of players"))?,
        })
    }
}

impl ui::NewGameParameters {
    pub fn game_parameters(&self, profiles: &Profiles) -> Result<game::Parameters> {
        Ok(game::Parameters {
            ends: self.ends as u8,
            rules: profiles
                .rule_set
                .get(self.rules_profile as usize)
                .ok_or_else(|| anyhow!("Unknown rule set \"{}\"", self.rules_profile))?
                .data,
            ..game::Parameters::default()
        })
    }

    pub fn sheet_parameters(&self, profiles: &Profiles) -> Result<game::sheet::Parameters> {
        Ok(profiles
            .ice_profile
            .get(self.ice_profile as usize)
            .ok_or_else(|| anyhow!("Unknown ice profile \"{}\"", self.ice_profile))?
            .data)
    }

    pub fn teams(&self) -> Result<PerTeam<game::team::Info>> {
        let teams: Result<Vec<_>> = self.teams.iter().map(|team| team.team_info()).collect();
        let array: [_; game::team::TEAMS_COUNT] =
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
        game::stone::COUNT
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let game = self.game.borrow();
        let stones = &game.sheet.stones;
        let position_feet = if stones.in_play().contains(row) {
            stones.positions()[row].map(|x| x.get::<foot>())
        } else {
            Vector2 { x: -100.0, y: -100.0 }
        };
        let team = game::stone::team(row);
        Some(StoneModel { x: position_feet.x, y: position_feet.y, color: game.teams[team].color })
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<'a> ui::Shot<'a> {
    pub fn current_call(&self, sheet: &game::sheet::Parameters) -> game::turn::delivery::Call {
        game::turn::delivery::Call {
            weight: if self.get_automatic_weight() {
                sheet.velocity_for_target_y(feet(self.get_mark_y()))
            } else {
                sheet.velocity_for_hog_to_hog_time(seconds(self.get_hog_to_hog_time()))
            },
            mark: Vector2 { x: feet(self.get_mark_x()), y: feet(self.get_mark_y()) },
            rotation: if self.get_clockwise() {
                game::stone::Rotation::Clockwise
            } else {
                game::stone::Rotation::CounterClockwise
            },
        }
    }

    pub fn update_shot_preview(&self, game: &Game) {
        let call = self.current_call(&game.sheet.parameters);
        let current_team_color = game.playing_team().map(|team| game.teams[team].color);
        let preview = game.expected_path(call).unwrap_or_default();
        let commands = format!(
            "{}",
            preview.iter().step_by(PREVIEW_STEPS).enumerate().format_with(
                " ",
                |(index, pos), f| {
                    f(&format_args!(
                        "{} {} {}",
                        if index % 2 == 1 { "L" } else { "M" },
                        pos.x.get::<foot>(),
                        pos.y.get::<foot>()
                    ))
                }
            )
        );
        self.set_preview_commands(commands.into());
        let last = preview.last().copied().unwrap_or_default();
        self.set_preview_result(StoneModel {
            color: current_team_color.unwrap_or_default(),
            x: last.x.get::<foot>(),
            y: last.y.get::<foot>(),
        });
    }
}
