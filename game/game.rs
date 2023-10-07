use crate::{score::Score, setup::Setup, situation::Situation, Delivery, ViolatedRule};
use gielo_sheet::{stone, stone::Stones};
use gielo_team::Team;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct TurnIndex(usize);

impl TurnIndex {
    pub fn new(end: usize, stone: usize) -> Self {
        Self(end * stone::COUNT + stone)
    }

    pub fn end(&self) -> usize {
        self.0 / stone::COUNT
    }

    pub fn stone(&self) -> usize {
        self.0 % stone::COUNT
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn previous(&self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Violation {
    pub rule: ViolatedRule,
    pub restored: Option<Stones>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Turn {
    pub plan: Delivery,
    pub actual: Delivery,
    pub result: Stones,
    pub violation: Option<Violation>,
}

impl Turn {
    fn outcome(&self) -> &Stones {
        self.violation.as_ref().and_then(|v| v.restored.as_ref()).unwrap_or(&self.result)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game<NameT, ColorT> {
    setup: Setup<NameT, ColorT>,
    turns: Vec<Turn>,
    end_scores: Vec<Score>,
}

impl<NameT, ColorT> Game<NameT, ColorT> {
    pub fn new(setup: Setup<NameT, ColorT>) -> Self {
        Self { setup, turns: vec![], end_scores: vec![] }
    }

    pub fn setup(&self) -> &Setup<NameT, ColorT> {
        &self.setup
    }

    pub fn first_recorded_turn(&self) -> TurnIndex {
        self.setup.starting_situation.turn
    }

    pub fn next_turn_index(&self) -> TurnIndex {
        TurnIndex(self.first_recorded_turn().0 + self.turns.len())
    }

    pub fn turn(&self, index: TurnIndex) -> Option<&Turn> {
        if index < self.first_recorded_turn() {
            None
        } else {
            self.turns.get(self.turn_offset(index))
        }
    }

    pub fn enumerate_turns(&self) -> impl ExactSizeIterator<Item = (TurnIndex, &Turn)> {
        let start_index = self.first_recorded_turn().0;
        (start_index..start_index + self.turns.len()).map(TurnIndex).zip(&self.turns)
    }

    pub fn enumerate_ends_scores(&self) -> impl ExactSizeIterator<Item = (usize, &Score)> {
        let start_index = self.first_recorded_turn().end();
        (start_index..start_index + self.end_scores.len()).zip(&self.end_scores)
    }
    pub fn stones_before(&self, index: TurnIndex) -> Option<Stones> {
        if index == self.first_recorded_turn() {
            Some(self.setup.starting_situation.stones.clone())
        } else if index.stone() == 0 {
            Some(Stones::new())
        } else if let Some(previous_turn) = index.previous().and_then(|prev| self.turn(prev)) {
            Some(previous_turn.outcome().clone())
        } else {
            None
        }
    }

    pub fn score_in_turn(&self, index: TurnIndex) -> Score {
        let end = index.end();
        self.end_scores.iter().take(self.end_score_index(end)).copied().sum::<Score>()
            + self.setup.starting_situation.score
    }

    pub fn situation_after_finished_delivery(&self, index: TurnIndex) -> Option<Situation> {
        let turn = self.turn(index)?;
        Some(Situation {
            turn: index,
            score: self.score_in_turn(index),
            hammer: self.hammer_in_end(index.end()),
            stones: turn.result.clone(),
        })
    }

    pub fn hammer_in_end(&self, end: usize) -> Team {
        self.end_scores
            .iter()
            .take(self.end_score_index(end))
            .rev()
            .find_map(|&end_score| hammer_change(end_score))
            .unwrap_or(self.setup.starting_situation.hammer)
    }

    fn turn_offset(&self, index: TurnIndex) -> usize {
        index.0 - self.first_recorded_turn().0
    }

    fn end_score_index(&self, end: usize) -> usize {
        end - self.first_recorded_turn().end()
    }

    pub fn store_turn(&mut self, index: TurnIndex, turn: Turn) {
        let offset = self.turn_offset(index);
        if offset == self.turns.len() {
            self.turns.push(turn)
        } else {
            self.turns[offset] = turn
        }
    }

    pub fn store_end_score(&mut self, end: usize, score: Score) {
        let offset = self.end_score_index(end);
        if offset == self.end_scores.len() {
            self.end_scores.push(score)
        } else {
            self.end_scores[offset] = score
        }
    }
}

fn hammer_change(prev_end_score: Score) -> Option<Team> {
    match prev_end_score.a.cmp(&prev_end_score.b) {
        Ordering::Less => Some(Team::B),
        Ordering::Equal => None,
        Ordering::Greater => Some(Team::A),
    }
}
