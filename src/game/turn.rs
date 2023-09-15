use crate::{
    game::{
        simulation,
        simulation::Simulation,
        stone,
        stone::Stones,
        team,
        team::{player, PerTeam, Team},
        Dirty, Parameters, Sheet,
    },
    unit,
    unit::{seconds, Time},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time;

pub mod delivery;

pub type Index = usize;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Violation {
    FreeGuardRule,
    NoTickRule,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum ResolvedViolation {
    #[default]
    None,
    FreeGuardRule,
    NoTickRuleStonesReplaced,
    NoTickRuleStonesLeft,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[allow(clippy::large_enum_variant)]
pub enum Phase {
    Thinking,
    Violation {
        violation: Violation,
    },
    Finished {
        snapshot: Stones,
        violation: ResolvedViolation,
    },
    // Logically it's between Violation and Finished, but must be last due to serde bug:
    // https://github.com/serde-rs/serde/issues/2614
    #[serde(skip)]
    Delivering {
        started_at: time::Instant,
        process: simulation::delivery::Process,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Current {
    pub played_stone: stone::Id,
    pub delivering_player: player::Id,
    pub stones_before: Stones,
    pub free_guards: stone::Flag,
    pub free_center_guards: stone::Flag,
    pub delivery_time: Time,
    pub phase: Phase,
}

impl Current {
    pub fn new_first(hammer: Team) -> Self {
        Self {
            played_stone: stone::QUEUE_BY_HAMMER[hammer][0],
            delivering_player: player::who_is_delivering(0),
            stones_before: Stones::default(),
            free_guards: stone::Flag(0),
            free_center_guards: stone::Flag(0),
            delivery_time: Time::default(),
            phase: Phase::Thinking,
        }
    }

    pub fn new(
        played_stone: stone::Id,
        delivering_player: player::Id,
        sheet: &Sheet,
        free_guards: bool,
        free_center_guards: bool,
    ) -> Self {
        Self {
            played_stone,
            delivering_player,
            stones_before: sheet.stones.clone(),
            free_guards: free_guards.then(|| sheet.guards()).unwrap_or_default(),
            free_center_guards: free_center_guards
                .then(|| sheet.center_guards())
                .unwrap_or_default(),
            delivery_time: Time::default(),
            phase: Phase::Thinking,
        }
    }

    pub fn new_by_index(
        index: Index,
        hammer: Team,
        sheet: &Sheet,
        parameters: &Parameters,
    ) -> Self {
        let played_stone = stone::QUEUE_BY_HAMMER[hammer][index];
        let delivering_player = player::who_is_delivering(index);
        let free_guards = index < parameters.rules.free_guard_rule_stones;
        let free_center_guards = index < parameters.rules.no_tick_rule_stones;
        Self::new(played_stone, delivering_player, sheet, free_guards, free_center_guards)
    }

    pub fn playing_team(&self) -> Team {
        stone::team(self.played_stone)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished { .. })
    }

    pub fn violation(&self) -> Option<Violation> {
        match &self.phase {
            Phase::Violation { violation } => Some(*violation),
            _ => None,
        }
    }

    pub fn update(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        simulation: &Simulation,
        now: time::Instant,
        speed_factor: f32,
    ) {
        let finished = match &mut self.phase {
            Phase::Delivering { started_at, process } => {
                let real_time = seconds((now - *started_at).as_secs_f32());
                let game_time = real_time * speed_factor as unit::BaseType;
                let finished = simulation::delivery::Update { dirty, sheet, simulation, process }
                    .run(game_time);
                self.delivery_time = process.current_time;
                finished
            }
            _ => false,
        };
        if finished {
            self.phase = match self.check_violations(sheet) {
                Some(violation) => Phase::Violation { violation },
                None => Phase::Finished {
                    snapshot: sheet.stones.clone(),
                    violation: ResolvedViolation::None,
                },
            };
            dirty.phase = true;
        }
    }

    fn check_violations(&self, sheet: &Sheet) -> Option<Violation> {
        let opponent = stone::team(self.played_stone).opponent();
        let opposition_stones = stone::Flag::TEAM[opponent];
        let out_of_play = !sheet.stones.in_play();
        let not_center_guard = !sheet.center_guards();
        let free_guards_removed = out_of_play & self.free_guards & opposition_stones;
        let free_center_guards_ticked =
            not_center_guard & self.free_center_guards & opposition_stones;
        match (free_guards_removed, free_center_guards_ticked) {
            (stone::Flag(0), stone::Flag(0)) => None,
            (stone::Flag(0), stone::Flag(_)) => Some(Violation::NoTickRule),
            (stone::Flag(_), stone::Flag(_)) => Some(Violation::FreeGuardRule),
        }
    }

    pub fn start_delivery(&mut self, delivery: delivery::Start, now: time::Instant) -> Result<()> {
        let new_phase = match &mut self.phase {
            Phase::Thinking => {
                let resolved = delivery.resolve(self.played_stone, self.delivering_player);
                log::info!("Starting delivery: {resolved:?}");
                resolved.dirty.phase = true;
                Phase::Delivering {
                    started_at: now,
                    process: simulation::delivery::Process::new(resolved),
                }
            }
            _ => bail!("Starting delivery not on thinking phase"),
        };
        self.phase = new_phase;
        Ok(())
    }

    pub fn proceed(&mut self, dirty: &mut Dirty, sheet: &mut Sheet) -> Result<()> {
        let snapshot = sheet.stones.clone();
        let violation = match &mut self.phase {
            Phase::Violation { violation: Violation::FreeGuardRule } => {
                sheet.stones.restore(dirty, self.stones_before.clone());
                ResolvedViolation::FreeGuardRule
            }
            Phase::Violation { violation: Violation::NoTickRule } => {
                ResolvedViolation::NoTickRuleStonesLeft
            }
            _ => bail!("Cannot replace stones at current phase"),
        };
        self.phase = Phase::Finished { snapshot, violation };
        Ok(())
    }

    pub fn replace_stones(&mut self, dirty: &mut Dirty, sheet: &mut Sheet) -> Result<()> {
        let violation = match &mut self.phase {
            Phase::Violation { violation: Violation::FreeGuardRule } => {
                ResolvedViolation::FreeGuardRule
            }
            Phase::Violation { violation: Violation::NoTickRule } => {
                ResolvedViolation::NoTickRuleStonesReplaced
            }
            _ => bail!("Cannot replace stones at current phase"),
        };
        let snapshot = sheet.stones.clone();
        sheet.stones.restore(dirty, self.stones_before.clone());
        self.phase = Phase::Finished { snapshot, violation };
        Ok(())
    }

    pub fn expected_path(
        &self,
        sheet: &Sheet,
        simulation: &Simulation,
        teams: &PerTeam<team::Info>,
        call: delivery::Call,
    ) -> Vec<stone::Position> {
        let mut sheet_copy = sheet.clone();
        let mut dirty = Dirty::new();
        let start = delivery::Start { call, sheet: &mut sheet_copy, teams, dirty: &mut dirty };
        let resolved = start.resolve_ideal(self.played_stone, self.delivering_player);
        let mut process = simulation::delivery::Process::new(resolved);
        let mut result = vec![sheet_copy.stones.positions()[self.played_stone]];
        let mut update = simulation::delivery::Update {
            process: &mut process,
            sheet: &mut sheet_copy,
            simulation,
            dirty: &mut dirty,
        };
        update.trace_until_event(|update| {
            result.push(update.sheet.stones.positions()[self.played_stone])
        });
        result
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Finished {
    pub played_stone: stone::Id,
    pub delivering_player: player::Id,
    pub violation: ResolvedViolation,
    pub snapshot: Stones,
}

impl TryFrom<Current> for Finished {
    type Error = anyhow::Error;

    fn try_from(current: Current) -> anyhow::Result<Self> {
        // panic!("Check how we do panic.");
        match current.phase {
            Phase::Finished { snapshot, violation, .. } => Ok(Self {
                played_stone: current.played_stone,
                delivering_player: current.delivering_player,
                snapshot,
                violation,
            }),
            _ => bail!("Trying to get Finished Turn from unfinished Current Turn"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{sheet, sheet::Hack, stone::Rotation, tests::PhaseTestSetup},
        unit::{assert_float_eq, feet},
        vector::Vector2,
    };

    #[test]
    fn progressing_turn() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let stone = 0;
        let mut turn = Current::new(stone, 0, &sheet, true, true);
        assert!(matches!(&turn.phase, Phase::Thinking));
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));

        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        turn.start_delivery(delivery, time).expect("Starting delivery failed");
        let Phase::Delivering { started_at, process } = &turn.phase else {
            panic!("Wrong stage");
        };
        assert_eq!(started_at, &time);
        assert!(process.stones[stone].is_moving());
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(turn.delivery_time, seconds(0.0));
        assert_eq!(sheet.stones.in_play(), stone::Flag::stone(stone));
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(stone),
            phase: true,
            ..Dirty::default()
        });

        let first_update = time + time::Duration::from_secs(2);
        turn.update(&mut dirty, &mut sheet, &simulation, first_update, 5.0);
        let Phase::Delivering { started_at, process } = &turn.phase else {
            panic!("Wrong stage");
        };
        assert_eq!(started_at, &time);
        assert!(process.stones[stone].is_moving());
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(turn.delivery_time, seconds(10.0));
        dirty.check_and_clear(&Dirty { stones: stone::Flag::stone(stone), ..Dirty::default() });

        let finishing_update = time + time::Duration::from_secs(7);
        turn.update(&mut dirty, &mut sheet, &simulation, finishing_update, 5.0);
        let Phase::Finished { snapshot, violation } = &turn.phase else { panic!("Wrong stage") };
        assert!(turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(snapshot, &sheet.stones);
        assert_eq!(*violation, ResolvedViolation::None);
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(stone),
            phase: true,
            ..Dirty::new()
        });
    }

    #[test]
    fn violations() {
        let sheet = sheet::Parameters::default();
        let queue = stone::QUEUE_BY_HAMMER.b;
        let center_guard_pos = sheet.geometry.tee()
            + Vector2 { x: feet(0.0), y: -sheet.geometry.house_radius - feet(4.0) };
        let corner_guard_pos = sheet.geometry.tee()
            + Vector2 {
                x: sheet.geometry.house_radius / 2.0,
                y: -sheet.geometry.house_radius - feet(4.0),
            };
        let center_guard = queue[0];
        let corner_guard = queue[2];
        let opponent_stone = queue[3];
        let same_team_stone = queue[4];

        let run_case = |stone: stone::Id, call: delivery::Call, expected: Option<Violation>| {
            let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
                PhaseTestSetup::new();
            sheet.stones = Stones::from_iter([
                (center_guard, center_guard_pos),
                (corner_guard, corner_guard_pos),
            ]);
            let mut turn = Current::new(stone, 1, &sheet, true, true);
            let delivery =
                delivery::Start { call, sheet: &mut sheet, teams: &teams, dirty: &mut dirty };
            turn.start_delivery(delivery, time).expect("Failed to start delivery");
            turn.update(
                &mut dirty,
                &mut sheet,
                &simulation,
                time + time::Duration::from_secs(10),
                10.0,
            );
            let violation = match turn.phase {
                Phase::Violation { violation } => Some(violation),
                Phase::Finished { violation, .. } => {
                    assert_eq!(violation, ResolvedViolation::None);
                    None
                }
                _ => panic!("Wrong phase after update"),
            };
            assert_eq!(violation, expected);
        };

        let corner_take_out = delivery::Call {
            weight: sheet.velocity_for_hog_to_hog_time(seconds(9.0)),
            mark: corner_guard_pos,
            rotation: Rotation::None,
        };
        run_case(opponent_stone, corner_take_out, Some(Violation::FreeGuardRule));
        run_case(same_team_stone, corner_take_out, None);

        let center_push = delivery::Call {
            weight: sheet.velocity_for_hog_to_hog_time(seconds(14.0)),
            mark: center_guard_pos,
            rotation: Rotation::None,
        };
        run_case(opponent_stone, center_push, Some(Violation::NoTickRule));
        run_case(same_team_stone, center_push, None);

        let center_take_out = delivery::Call {
            weight: sheet.velocity_for_hog_to_hog_time(seconds(9.0)),
            mark: center_guard_pos,
            rotation: Rotation::None,
        };
        run_case(opponent_stone, center_take_out, Some(Violation::FreeGuardRule));
        run_case(same_team_stone, center_take_out, None);
    }

    #[test]
    fn proceeding_after_violation() {
        let sheet_params = sheet::Parameters::default();

        let stones_before = Stones::from_iter([(0, sheet_params.geometry.tee())]);
        let stones_after = Stones::from_iter([(8, sheet_params.geometry.tee())]);
        let mut sheet = Sheet::new(sheet_params);
        sheet.stones = stones_after.clone();

        let fgz_violation = || Current {
            played_stone: 8,
            delivering_player: 0,
            stones_before: stones_before.clone(),
            free_guards: Default::default(),
            free_center_guards: Default::default(),
            delivery_time: Default::default(),
            phase: Phase::Violation { violation: Violation::FreeGuardRule },
        };
        let no_tick_rule_violation = || Current {
            phase: Phase::Violation { violation: Violation::NoTickRule },
            ..fgz_violation()
        };

        let mut dirty = Dirty::new();
        let expected_dirty_after_replacing =
            Dirty { stones: stone::Flag::stone(0) | stone::Flag::stone(8), ..Default::default() };

        let mut turn = fgz_violation();
        assert!(turn.replace_stones(&mut dirty, &mut sheet).is_ok());
        let Phase::Finished { violation, snapshot, .. } = &turn.phase else {
            panic!("Wrong phase");
        };
        assert_eq!(*violation, ResolvedViolation::FreeGuardRule);
        assert_eq!(snapshot, &stones_after);
        assert_eq!(sheet.stones, stones_before);
        dirty.check_and_clear(&expected_dirty_after_replacing);

        sheet.stones = stones_after.clone();
        let mut turn = fgz_violation();
        assert!(turn.proceed(&mut dirty, &mut sheet).is_ok());
        let Phase::Finished { violation, snapshot, .. } = &turn.phase else {
            panic!("Wrong phase");
        };
        assert_eq!(*violation, ResolvedViolation::FreeGuardRule);
        assert_eq!(snapshot, &stones_after);
        assert_eq!(sheet.stones, stones_before);
        dirty.check_and_clear(&expected_dirty_after_replacing);

        sheet.stones = stones_after.clone();
        let mut turn = no_tick_rule_violation();
        assert!(turn.replace_stones(&mut dirty, &mut sheet).is_ok());
        let Phase::Finished { violation, snapshot, .. } = &turn.phase else {
            panic!("Wrong phase");
        };
        assert_eq!(*violation, ResolvedViolation::NoTickRuleStonesReplaced);
        assert_eq!(snapshot, &stones_after);
        assert_eq!(sheet.stones, stones_before);
        dirty.check_and_clear(&expected_dirty_after_replacing);

        sheet.stones = stones_after.clone();
        let mut turn = no_tick_rule_violation();
        assert!(turn.proceed(&mut dirty, &mut sheet).is_ok());
        let Phase::Finished { violation, snapshot, .. } = &turn.phase else {
            panic!("Wrong phase");
        };
        assert_eq!(*violation, ResolvedViolation::NoTickRuleStonesLeft);
        assert_eq!(snapshot, &stones_after);
        assert_eq!(sheet.stones, stones_after);
        assert_eq!(dirty, Dirty::default());
    }

    #[test]
    fn updating_and_starting_in_wrong_stage() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let stone = 1;
        let mut turn = Current::new(stone, 0, &sheet, false, false);

        turn.update(&mut dirty, &mut sheet, &simulation, time, 5.0);
        dirty.check_and_clear(&Dirty::new());

        turn.phase = Phase::Delivering {
            started_at: time,
            process: simulation::delivery::Process::new(
                delivery::Start::tee_draw(&mut Dirty::default(), &mut sheet, &teams)
                    .resolve(stone, 0),
            ),
        };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        assert!(turn.start_delivery(delivery, time).is_err());
        dirty.check_and_clear(&Dirty::new());

        turn.phase =
            Phase::Finished { snapshot: Stones::default(), violation: ResolvedViolation::None };
        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        assert!(turn.start_delivery(delivery, time).is_err());
        dirty.check_and_clear(&Dirty::new());
        turn.update(&mut dirty, &mut sheet, &simulation, time, 5.0);
        dirty.check_and_clear(&Dirty::new());
    }

    #[test]
    fn finished_turn_conversion() {
        let mut stones = Stones::new();
        stones.put_stone(&mut Dirty::new(), 10, Vector2 { x: feet(1.0), y: feet(133.0) });
        let unfinished = Current {
            played_stone: 12,
            delivering_player: 2,
            phase: Phase::Thinking,
            free_guards: stone::Flag::all_before(5),
            free_center_guards: stone::Flag::all_before(3),
            stones_before: Stones::new(),
            delivery_time: Default::default(),
        };
        let finished = Current {
            played_stone: 12,
            delivering_player: 2,
            free_guards: stone::Flag::all_before(5),
            free_center_guards: stone::Flag::all_before(3),
            delivery_time: seconds(20.0),
            stones_before: Stones::new(),
            phase: Phase::Finished {
                snapshot: stones.clone(),
                violation: ResolvedViolation::FreeGuardRule,
            },
        };
        assert!(TryInto::<Finished>::try_into(unfinished).is_err());
        let converted: Finished =
            finished.try_into().expect("Finished turn should convert successfully");
        assert_eq!(converted.played_stone, 12);
        assert_eq!(converted.delivering_player, 2);
        assert_eq!(converted.snapshot, stones);
        assert_eq!(converted.violation, ResolvedViolation::FreeGuardRule);
    }

    #[test]
    fn shot_preview() {
        let sheet = Sheet::new(sheet::Parameters::default());
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet.parameters);
        let teams = PerTeam::default();
        let turn = Current {
            played_stone: 0,
            delivering_player: 0,
            phase: Phase::Thinking,
            free_guards: stone::Flag::default(),
            free_center_guards: stone::Flag::default(),
            stones_before: Default::default(),
            delivery_time: Default::default(),
        };
        let path = turn.expected_path(
            &sheet,
            &simulation,
            &teams,
            delivery::Call::tee_draw(&sheet.parameters),
        );
        let hack = sheet.parameters.geometry.hack_pos(Hack::Left);
        assert_float_eq!(path.first().unwrap().x, hack.x);
        assert_float_eq!(path.first().unwrap().y, hack.y);
        assert_float_eq!(path[path.len() / 4].x, feet(2.0), abs <= 2.0);
        assert_float_eq!(path[path.len() / 4].y, feet(87.0), abs <= 2.0);
        assert_float_eq!(path[path.len() / 2].x, feet(3.0), abs <= 2.0);
        assert_float_eq!(path[path.len() / 2].y, feet(113.0), abs <= 2.0);
        assert_float_eq!(path.last().unwrap().x, feet(0.0), abs <= 2.0);
        assert_float_eq!(
            path.last().unwrap().y,
            sheet.parameters.geometry.playing_end.tee_line_y,
            abs <= 2.0
        );
    }

    /// See https://github.com/serde-rs/serde/issues/2614
    #[test]
    fn serde_failure_case() {
        let phase = Phase::Violation { violation: Violation::FreeGuardRule };
        let bin = bincode2::serialize(&phase).unwrap();
        let deserialized: Phase = bincode2::deserialize(&bin).unwrap();
        assert!(matches!(deserialized, Phase::Violation { violation: Violation::FreeGuardRule }));
    }
}
