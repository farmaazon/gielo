use crate::game::team::PerTeam;
use crate::game::MAX_ENDS;
use local_vec::LocalVec;

pub type Score = PerTeam<u8>;

#[derive(Clone, Debug, Default)]
pub struct Table {
    ends: LocalVec<Score, MAX_ENDS>,
    full: Score,
    scored_ends_change: usize,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_end_scores(end_scores: impl IntoIterator<Item = impl Into<Score>>) -> Self {
        let mut this = Self::new();
        for end_score in end_scores {
            this.push_end(end_score.into())
        }
        this
    }

    pub fn push_end(&mut self, score: Score) {
        self.ends.push(score);
        self.full += score;
        self.scored_ends_change += 1;
    }

    pub fn full(&self) -> Score {
        self.full
    }

    pub fn ends(&self) -> &[Score] {
        &self.ends
    }

    pub fn read(&mut self) -> (usize, Score) {
        (std::mem::take(&mut self.scored_ends_change), self.full)
    }
}
