use crate::game;
use crate::game::Game;
use crate::ui::{GameModel, SheetEndGeometry, SheetGeometry, SheetModel, StoneModel, Team};
use crate::unit::{feet, feet_per_second_squared};
use crate::vector::Vector2;
use anyhow::{anyhow, Result};
use slint::Model;
use slint::VecModel;
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::cmp::Ordering;
use std::rc::Rc;
use uom::si::acceleration::foot_per_second_squared;
use uom::si::length::foot;

impl<'a> SheetModel<'a> {
    pub fn initialize(&self) {
        let sheet_params = game::sheet::Parameters::default();
        let geometry = SheetGeometry {
            width: sheet_params.width.get::<foot>(),
            height: game::sheet::LENGTH.get::<foot>(),
            house_radius: game::sheet::HOUSE_RADIUS.get::<foot>(),
            hack_offset: game::sheet::HACK_X_OFFSET.get::<foot>(),
            tee_x: (*game::sheet::CENTER_LINE_X + sheet_params.width / 2.0).get::<foot>(),
            delivery_end: SheetEndGeometry {
                back_y: game::sheet::delivery_end::BACK_LINE_Y.get::<foot>(),
                tee_y: game::sheet::delivery_end::TEE_LINE_Y.get::<foot>(),
                hog_y: game::sheet::delivery_end::HOG_LINE_Y.get::<foot>(),
            },
            playing_end: SheetEndGeometry {
                back_y: game::sheet::playing_end::BACK_LINE_Y.get::<foot>(),
                tee_y: game::sheet::playing_end::TEE_LINE_Y.get::<foot>(),
                hog_y: game::sheet::playing_end::HOG_LINE_Y.get::<foot>(),
            },
        };
        self.set_geometry(geometry);
        self.set_friction(sheet_params.friction.get::<foot_per_second_squared>());
        self.set_rotation_acc(sheet_params.rotation_acc.get::<foot_per_second_squared>());
        self.set_stone_radius(sheet_params.stone_radius.get::<foot>());
    }

    pub fn parameters(&self) -> game::sheet::Parameters {
        game::sheet::Parameters {
            friction: feet_per_second_squared(self.get_friction()),
            rotation_acc: feet_per_second_squared(self.get_rotation_acc()),
            stone_radius: feet(self.get_stone_radius()),
            width: feet(self.get_geometry().width),
            ..game::sheet::Parameters::default()
        }
    }
}

impl<'a> GameModel<'a> {
    pub fn initialize(&self) {
        let team_red = Team {
            name: "Red".into(),
            color: slint::Color::from_rgb_u8(255, 0, 0),
            ..Team::default()
        };
        let team_yellow = Team {
            name: "Yellow".into(),
            color: slint::Color::from_rgb_u8(255, 255, 0),
            ..Team::default()
        };
        let teams = vec![team_red, team_yellow];
        let teams_model: VecModel<Team> = teams.into();
        self.set_teams(Rc::new(teams_model).into());
    }

    pub fn parameters(&self) -> game::Parameters {
        game::Parameters::default()
    }

    pub fn teams_info(&self) -> Result<game::team::PerTeam<game::team::Info>> {
        let teams = self.get_teams();
        let mut rows = 0..teams.row_count();
        game::team::teams()
            .map(|_| {
                let row =
                    rows.next().ok_or_else(|| anyhow!("Missing rows in View's team models"))?;
                let team_model = teams
                    .row_data(row)
                    .ok_or_else(|| anyhow!("Missing info for row {} in View's team models", row))?;
                let info = game::team::Info { name: team_model.name, color: team_model.color };
                Ok(info)
            })
            .into()
    }
}

pub struct Stones {
    game: Rc<RefCell<Game>>,
    notify: slint::ModelNotify,
}

impl Stones {
    pub fn new(game: Rc<RefCell<Game>>) -> Self {
        Self { game, notify: slint::ModelNotify::default() }
    }

    pub fn synchronize(&self, game: &mut RefMut<Game>) {
        let time = game.delivery_time();
        let sheet = &mut game.sheet;
        let count = sheet.stones.read_len();
        let known_count = (count.value as isize - count.change) as usize;
        match count.change.cmp(&0) {
            Ordering::Greater => self.notify.row_added(known_count, count.change as usize),
            Ordering::Less => self.notify.row_removed(count.value, (-count.change) as usize),
            Ordering::Equal => {}
        }
        for (index, stone) in sheet.stones.iter_mut().take(known_count).enumerate() {
            let (changed, _) = stone.read_position(time);
            if changed {
                self.notify.row_changed(index);
            }
        }
    }
}

impl Model for Stones {
    type Data = StoneModel;

    fn row_count(&self) -> usize {
        self.game.borrow_mut().sheet.stones.read_len().value
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let mut game = self.game.borrow_mut();
        let time = game.delivery_time();
        let stone = &mut game.sheet.stones[row];
        let (_, position) = stone.read_position(time);
        let position_feet = position
            .map(|v| v.map(|x| x.get::<foot>()))
            .unwrap_or(Vector2 { x: -100.0, y: -100.0 });
        let team = stone.team;
        Some(StoneModel { x: position_feet.x, y: position_feet.y, color: game.teams[team].color })
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
