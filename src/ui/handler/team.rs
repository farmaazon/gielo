use crate::{
    game,
    game::{
        team::{teams, PerTeam},
        RunningGame,
    },
    ui,
};
use gielo_game::dirty::Dirty;
use slint::{Model, SharedString};
use std::{cmp, rc::Rc};

pub struct Handler {
    model: Rc<slint::VecModel<ui::PlayingTeam>>,
    score: PerTeam<Rc<slint::VecModel<i32>>>,
    players: PerTeam<Rc<slint::VecModel<ui::Player>>>,
}

impl Handler {
    pub fn new(game: &RunningGame, game_model: &ui::GameModel<'_>) -> Self {
        let score = teams().map(|_| Rc::new(slint::VecModel::default()));
        let players =
            game.setup().teams.as_ref().map(|team| Rc::new(Self::players_ui_model(&team.players)));
        let model = Rc::new(slint::VecModel::default());
        for team in teams() {
            let team_score = score[team].clone();
            let team_players = players[team].clone();
            model.push(Self::team_ui_model(game, team, team_score, team_players));
        }
        game_model.set_teams(model.clone().into());
        Self { score, model, players }
    }

    pub fn synchronize(&self, dirty: &Dirty, game: &RunningGame) {
        self.synchronize_score(dirty, game);
        self.synchronize_teams(dirty, game);
    }

    pub fn synchronize_score(&self, dirty: &Dirty, game: &RunningGame) {
        if dirty.score {
            let known_scores = self.score.a.row_count();
            let new_scores = game.end_scores();
            match new_scores.len().cmp(&known_scores) {
                cmp::Ordering::Greater => {
                    for (_, new_end) in new_scores.skip(known_scores) {
                        for (score, new_end_score) in self.score.as_ref().zip(*new_end) {
                            score.push(new_end_score as i32)
                        }
                    }
                }
                cmp::Ordering::Less => {
                    for score in self.score.as_ref() {
                        while score.row_count() > new_scores.len() {
                            score.remove(new_scores.len());
                        }
                    }
                }
                cmp::Ordering::Equal => {}
            }
        }
    }

    pub fn synchronize_teams(&self, dirty: &Dirty, game: &RunningGame) {
        if dirty.turn || dirty.score {
            let items = self.model.row_count();
            for (index, team) in (0..items).zip(teams()) {
                let team_score = self.score[team].clone();
                let team_players = self.players[team].clone();
                let team_struct = Self::team_ui_model(game, team, team_score, team_players);
                self.model.set_row_data(index, team_struct);
            }
        }
    }

    fn team_ui_model(
        game: &RunningGame,
        team: game::team::Team,
        score: Rc<slint::VecModel<i32>>,
        players: Rc<slint::VecModel<ui::Player>>,
    ) -> ui::PlayingTeam {
        let info = &game.setup().teams[team];
        ui::PlayingTeam {
            info: ui::Team { name: info.name.clone(), color: info.color, players: players.into() },
            stones_left: game.current().stones_left(team) as i32,
            end_score: score.into(),
            score: game.current().score[team] as i32,
            first_hammer: game.setup().starting_situation.hammer == team,
        }
    }

    fn player_ui_model(player: &game::team::player::Player<SharedString>) -> ui::Player {
        ui::Player {
            left_handed: player.used_hack == game::sheet::Hack::Right,
            name: player.name.clone(),
            skills: Default::default(),
        }
    }

    fn players_ui_model(
        players: &[game::team::player::Player<SharedString>],
    ) -> slint::VecModel<ui::Player> {
        let players: Vec<_> = players.iter().map(Self::player_ui_model).collect();
        slint::VecModel::from(players)
    }
}
