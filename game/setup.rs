use crate::situation::Situation;
use gielo_sheet as sheet;
use gielo_simulation as simulation;
use gielo_team as team;
use gielo_team::PerTeam;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Rules {
    pub free_guard_rule_stones: usize,
    pub no_tick_rule_stones: usize,
    pub ends: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self { free_guard_rule_stones: 5, no_tick_rule_stones: 5, ends: 8 }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Setup<NameT, ColorT> {
    pub sheet: sheet::Parameters,
    pub rules: Rules,
    pub teams: PerTeam<team::Info<NameT, ColorT>>,
    pub starting_situation: Situation,
    pub simulation: simulation::Parameters,
}
