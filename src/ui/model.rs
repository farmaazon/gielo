use crate::game;
use crate::game::team::PerTeam;
use crate::game::Game;
use crate::profiles::Profiles;
use crate::ui;
use crate::ui::{
    NewGameParameters, NewGameTeam, SheetEndGeometry, SheetGeometry, SheetModel, Shot, StoneModel,
};
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

impl<'a> ui::Profiles<'a> {
    pub fn initialize(&self, profiles: &Profiles) {
        let player = ModelRc::new(VecModel::from(profiles.player_skills_names().collect_vec()));
        let rules = ModelRc::new(VecModel::from(profiles.rule_set_names().collect_vec()));
        let ice = ModelRc::new(VecModel::from(profiles.ice_profile_names().collect_vec()));
        self.set_player_skill(player);
        self.set_rules(rules);
        self.set_ice(ice);
    }
}

impl<'a> SheetModel<'a> {
    pub fn set_parameters(&self, params: game::sheet::Parameters) {
        let make_end_geometry = |geom: game::sheet::EndGeometry| SheetEndGeometry {
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

impl ui::Player {
    pub fn player_info(self, profiles: &Profiles) -> Result<game::team::Player> {
        Ok(game::team::Player {
            skills: profiles
                .player_skills
                .get(self.skill_profile as usize)
                .ok_or_else(|| anyhow!("Unknown player profile: \"{}\"", self.skill_profile))?
                .data,
            used_hack: match self.left_handed {
                true => game::sheet::Hack::Right,
                false => game::sheet::Hack::Left,
            },
        })
    }
}

impl NewGameTeam {
    pub fn team_info(self, profiles: &Profiles) -> Result<game::team::Info> {
        let players: Result<Vec<_>> =
            self.players.iter().map(|ui_player| ui_player.player_info(profiles)).collect();
        Ok(game::team::Info {
            name: self.name,
            color: self.color,
            players: players?.try_into().map_err(|_| anyhow!("Wrong number of players"))?,
        })
    }
}

impl NewGameParameters {
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

    pub fn teams(&self, profiles: &Profiles) -> Result<PerTeam<game::team::Info>> {
        let teams: Result<Vec<_>> =
            self.teams.iter().map(|team| team.team_info(profiles)).collect();
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

impl<'a> Shot<'a> {
    pub fn current_call(&self) -> game::turn::delivery::Call {
        game::turn::delivery::Call {
            weight: seconds(self.get_weight_sec()),
            mark: Vector2 { x: feet(self.get_mark_x()), y: feet(self.get_mark_y()) },
            rotation: if self.get_clockwise() {
                game::stone::Rotation::Clockwise
            } else {
                game::stone::Rotation::CounterClockwise
            },
        }
    }
}
