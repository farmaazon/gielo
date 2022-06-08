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

    pub fn synchronize_score(&self, game: &mut Game) {
        let (count_changed, _) = game.score.read();
        if count_changed > 0 {
            let known_ends = game.score.ends().len() - count_changed;
            for new_end in game.score.ends().iter().skip(known_ends) {
                for (score, new_end) in self.score.as_ref().zip(*new_end) {
                    score.push(new_end as i32)
                }
            }
            // Full score also changed - need to synchronize teams
            self.synchronize_teams(game);
        }
    }

    pub fn synchronize_teams(&self, game: &Game) {
        let items = self.model.row_count();
        for (index, team) in (0..items).zip(game::team::teams()) {
            let team_score = self.score[team].clone();
            let team_struct = Self::team_ui_model(game, team_score, team);
            self.model.set_row_data(index, team_struct);
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
                .map_or(game::sheet::STONES_PER_TEAM, |e| e.stones_left(team))
                as i32,
            end_score: score.into(),
            score: game.score.full()[team] as i32,
        }
    }
}
