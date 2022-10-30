use crate::game::team::PerTeam;
use crate::game::MAX_ENDS;
use local_vec::LocalVec;
use crate::game::dirty::Dirty;

pub type Score = PerTeam<u8>;

#[derive(Debug, Default)]
pub struct Table {
    ends: LocalVec<Score, MAX_ENDS>,
    full: Score,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_end_scores(end_scores: impl IntoIterator<Item = impl Into<Score>>) -> Self {
        let mut this = Self::new();
        for end_score in end_scores {
            let score = end_score.into();
            this.ends.push(score);
            this.full += score;
        }
        this
    }

    pub fn push_end(&mut self, dirty: &mut Dirty, score: Score) {
        self.ends.push(score);
        self.full += score;
        dirty.score = true;
    }

    pub fn full(&self) -> Score {
        self.full
    }

    pub fn ends(&self) -> &[Score] {
        &self.ends
    }
}
