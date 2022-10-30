use crate::{game, ui, Game};
use slint::Model;
use std::rc::Rc;

pub struct Handler {
    model: Rc<slint::VecModel<ui::Team>>,
    score: game::team::PerTeam<Rc<slint::VecModel<i32>>>,
}

impl Handler {
    pub fn new(game: &Game, game_model: &ui::GameModel<'_>) -> Self {
        let score = game::team::teams().map(|_| Rc::new(slint::VecModel::default()));
        let model = Rc::new(slint::VecModel::default());
        for team in game::team::teams() {
            let team_score = score[team].clone();
            model.push(Self::team_ui_model(game, team_score, team));
        }
        game_model.set_teams(model.clone().into());
        Self { score, model }
    }

    pub fn synchronize(&self, dirty: &game::Dirty, game: &Game) {
        self.synchronize_score(dirty, game);
        self.synchronize_teams(dirty, game);
    }

    pub fn synchronize_score(&self, dirty: &game::Dirty, game: &Game) {
        if dirty.score {
            let known_ends = self.score.a.row_count();
            for new_end in game.score.ends().iter().skip(known_ends) {
                for (score, new_end) in self.score.as_ref().zip(*new_end) {
                    score.push(new_end as i32)
                }
            }
        }
    }

    pub fn synchronize_teams(&self, dirty: &game::Dirty, game: &Game) {
        if dirty.stage || dirty.score {
            let items = self.model.row_count();
            for (index, team) in (0..items).zip(game::team::teams()) {
                let team_score = self.score[team].clone();
                let team_struct = Self::team_ui_model(game, team_score, team);
                self.model.set_row_data(index, team_struct);
            }
        }
    }

    fn team_ui_model(
        game: &Game,
        score: Rc<slint::VecModel<i32>>,
        team: game::team::Team,
    ) -> ui::Team {
        let info = &game.teams[team];
        ui::Team {
            name: info.name.clone(),
            color: info.color,
            stones_left: game
                .current_end_stage()
                .map_or(game::stones::PER_TEAM, |e| e.stones_left(team))
                as i32,
            end_score: score.into(),
            score: game.score.full()[team] as i32,
        }
    }
}
