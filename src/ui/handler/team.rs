use crate::{game, ui, Game};
use slint::Model;
use std::{cmp, rc::Rc};

pub struct Handler {
    model: Rc<slint::VecModel<ui::PlayingTeam>>,
    score: game::team::PerTeam<Rc<slint::VecModel<i32>>>,
    players: game::team::PerTeam<Rc<slint::VecModel<ui::Player>>>,
}

impl Handler {
    pub fn new(game: &Game, game_model: &ui::GameModel<'_>) -> Self {
        let score = game::team::teams().map(|_| Rc::new(slint::VecModel::default()));
        let players =
            game.teams.as_ref().map(|team| Rc::new(Self::players_ui_model(&team.players)));
        let model = Rc::new(slint::VecModel::default());
        for team in game::team::teams() {
            let team_score = score[team].clone();
            let team_players = players[team].clone();
            model.push(Self::team_ui_model(game, team, team_score, team_players));
        }
        game_model.set_teams(model.clone().into());
        Self { score, model, players }
    }

    pub fn synchronize(&self, dirty: &game::Dirty, game: &Game) {
        self.synchronize_score(dirty, game);
        self.synchronize_teams(dirty, game);
    }

    pub fn synchronize_score(&self, dirty: &game::Dirty, game: &Game) {
        let known_ends = self.score.a.row_count();
        match dirty.finished_ends_count.cmp(&0) {
            cmp::Ordering::Greater => {
                for new_end in game.finished_ends.iter().skip(known_ends) {
                    let new_end_score = new_end.score;
                    for (score, new_end_score) in self.score.as_ref().zip(new_end_score) {
                        score.push(new_end_score as i32)
                    }
                }
            }
            cmp::Ordering::Less => {
                for score in self.score.as_ref() {
                    while score.row_count() > game.finished_ends.len() {
                        score.remove(game.finished_ends.len());
                    }
                }
            }
            cmp::Ordering::Equal => {}
        }
    }

    pub fn synchronize_teams(&self, dirty: &game::Dirty, game: &Game) {
        if dirty.phase || dirty.score {
            let items = self.model.row_count();
            for (index, team) in (0..items).zip(game::team::teams()) {
                let team_score = self.score[team].clone();
                let team_players = self.players[team].clone();
                let team_struct = Self::team_ui_model(game, team, team_score, team_players);
                self.model.set_row_data(index, team_struct);
            }
        }
    }

    fn team_ui_model(
        game: &Game,
        team: game::team::Team,
        score: Rc<slint::VecModel<i32>>,
        players: Rc<slint::VecModel<ui::Player>>,
    ) -> ui::PlayingTeam {
        let info = &game.teams[team];
        ui::PlayingTeam {
            info: ui::Team { name: info.name.clone(), color: info.color, players: players.into() },
            stones_left: game
                .current_end()
                .map_or(game::stone::COUNT_PER_TEAM, |e| e.stones_left(team))
                as i32,
            end_score: score.into(),
            score: game.score[team] as i32,
            first_hammer: game.first_hammer() == team,
        }
    }

    fn player_ui_model(player: &game::team::Player) -> ui::Player {
        ui::Player {
            left_handed: player.used_hack == game::sheet::Hack::Right,
            name: player.name.clone(),
            skills: Default::default(),
        }
    }

    fn players_ui_model(players: &[game::team::Player]) -> slint::VecModel<ui::Player> {
        let players: Vec<_> = players.iter().map(Self::player_ui_model).collect();
        slint::VecModel::from(players)
    }
}
