use crate::game::simulation::Simulation;
use crate::game::stone::Stones;
use crate::game::team::{player, PerTeam, Team};
use crate::game::{simulation, stone, team, Dirty, Parameters, Sheet};
use crate::unit::{seconds, Time};
use anyhow::{bail, Result};
use enumset::{EnumSet, EnumSetType};
use std::time;

pub mod delivery;

pub type Index = usize;

#[derive(Debug, EnumSetType)]
pub enum Violation {
    FreeGuardRule,
    NoTickRule,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Phase {
    Thinking,
    Delivering { started_at: time::Instant, process: simulation::delivery::Process },
    Finished { snapshot: Stones, delivery_time: Time, violations: EnumSet<Violation> },
}

#[derive(Debug)]
pub struct Current {
    pub played_stone: stone::Id,
    pub delivering_player: player::Id,
    pub free_guards: stone::Flag,
    pub free_center_guards: stone::Flag,
    pub phase: Phase,
}

impl Current {
    pub fn new(
        played_stone: stone::Id,
        delivering_player: player::Id,
        free_guards: stone::Flag,
        free_center_guards: stone::Flag,
    ) -> Self {
        Self {
            played_stone,
            delivering_player,
            free_guards,
            free_center_guards,
            phase: Phase::Thinking,
        }
    }

    pub fn new_by_index(
        index: Index,
        hammer: Team,
        free_guards: stone::Flag,
        free_center_guards: stone::Flag,
    ) -> Self {
        let played_stone = stone::QUEUE_BY_HAMMER[hammer][index];
        let delivering_player = player::who_is_delivering(index);
        Self::new(played_stone, delivering_player, free_guards, free_center_guards)
    }

    pub fn new_based_on_sheet(
        index: Index,
        sheet: &Sheet,
        parameters: &Parameters,
        hammer: Team,
    ) -> Self {
        let free_guards = if index < parameters.free_guard_rule_stones {
            sheet.guards()
        } else {
            stone::Flag::default()
        };
        let free_center_guards = if index < parameters.no_tick_rule_stones {
            sheet.center_guards()
        } else {
            stone::Flag::default()
        };
        Self::new_by_index(index, hammer, free_guards, free_center_guards)
    }

    pub fn playing_team(&self) -> Team {
        stone::team(self.played_stone)
    }

    pub fn delivery_time(&self) -> Time {
        match &self.phase {
            Phase::Thinking => Time::default(),
            Phase::Delivering { process, .. } => process.current_time,
            Phase::Finished { delivery_time, .. } => *delivery_time,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished { .. })
    }

    pub fn update(
        &mut self,
        dirty: &mut Dirty,
        sheet: &mut Sheet,
        simulation: &Simulation,
        now: time::Instant,
        speed_factor: f32,
    ) {
        let (finished, delivery_time) = match &mut self.phase {
            Phase::Delivering { started_at, process } => {
                let real_time = seconds((now - *started_at).as_secs_f32());
                let game_time = real_time * speed_factor;
                let finished = simulation::delivery::Update { dirty, sheet, simulation, process }
                    .run(game_time);
                (finished, process.current_time)
            }
            _ => (false, Time::default()),
        };
        if finished {
            self.phase = Phase::Finished {
                snapshot: sheet.stones.clone(),
                delivery_time,
                violations: self.check_violations(sheet),
            };
            dirty.phase = true;
        }
    }

    fn check_violations(&self, sheet: &Sheet) -> EnumSet<Violation> {
        let out_of_play = !sheet.stones.in_play();
        let not_center_guard = !sheet.center_guards();
        let free_guards_rule =
            (out_of_play & self.free_guards != stone::Flag(0)).then_some(Violation::FreeGuardRule);
        let no_tick_rule = (not_center_guard & self.free_center_guards != stone::Flag(0))
            .then_some(Violation::NoTickRule);
        free_guards_rule.into_iter().chain(no_tick_rule).collect()
    }

    pub fn start_delivery(&mut self, delivery: delivery::Start, now: time::Instant) -> Result<()> {
        let new_phase = match &mut self.phase {
            Phase::Thinking => {
                let mut resolved = delivery.resolve(self.played_stone, self.delivering_player);
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

#[derive(Clone, Debug)]
pub struct Finished {
    pub played_stone: stone::Id,
    pub delivering_player: player::Id,
    pub violations: EnumSet<Violation>,
    pub snapshot: Stones,
}

impl TryFrom<Current> for Finished {
    type Error = anyhow::Error;

    fn try_from(current: Current) -> anyhow::Result<Self> {
        match current.phase {
            Phase::Finished { snapshot, violations, .. } => Ok(Self {
                played_stone: current.played_stone,
                delivering_player: current.delivering_player,
                snapshot,
                violations,
            }),
            _ => bail!("Trying to get Finished Turn from unfinished Current Turn"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet;
    use crate::game::sheet::Hack;
    use crate::game::stone::Rotation;
    use crate::game::tests::PhaseTestSetup;
    use crate::unit::{assert_approx_eq, feet};
    use crate::vector::Vector2;

    #[test]
    fn progressing_turn() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let stone = 0;
        let mut turn = Current::new(stone, 0, stone::Flag(0), stone::Flag(0));
        assert!(matches!(&turn.phase, Phase::Thinking));
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));

        let delivery = delivery::Start::tee_draw(&mut dirty, &mut sheet, &teams);
        turn.start_delivery(delivery, time).expect("Starting delivery failed");
        let Phase::Delivering {started_at, process} = &turn.phase else {
            panic!("Wrong stage");
        };
        assert_eq!(started_at, &time);
        assert!(process.stones[stone].is_moving());
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(turn.delivery_time(), seconds(0.0));
        assert_eq!(sheet.stones.in_play(), stone::Flag::stone(stone));
        dirty.check_and_clear(&Dirty {
            stones: stone::Flag::stone(stone),
            phase: true,
            ..Dirty::default()
        });

        let first_update = time + time::Duration::from_secs(2);
        turn.update(&mut dirty, &mut sheet, &simulation, first_update, 5.0);
        let Phase::Delivering {started_at, process} = &turn.phase else {
            panic!("Wrong stage");
        };
        assert_eq!(started_at, &time);
        assert!(process.stones[stone].is_moving());
        assert!(!turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(turn.delivery_time(), seconds(10.0));
        dirty.check_and_clear(&Dirty { stones: stone::Flag::stone(stone), ..Dirty::default() });

        let finishing_update = time + time::Duration::from_secs(7);
        turn.update(&mut dirty, &mut sheet, &simulation, finishing_update, 5.0);
        let Phase::Finished {snapshot, delivery_time, violations } = &turn.phase else {
            panic!("Wrong stage")
        };
        assert!(turn.is_finished());
        assert_eq!(turn.playing_team(), stone::team(stone));
        assert_eq!(snapshot, &sheet.stones);
        assert_eq!(*violations, EnumSet::default());
        assert_eq!(turn.delivery_time(), *delivery_time);
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
        let corner_guard = queue[1];
        let current_stone = queue[2];

        let run_case = |call: delivery::Call, expected: EnumSet<Violation>| {
            let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
                PhaseTestSetup::new();
            sheet.stones = Stones::from_iter([
                (center_guard, center_guard_pos),
                (corner_guard, corner_guard_pos),
            ]);
            let mut turn = Current::new(
                current_stone,
                0,
                stone::Flag::stone(center_guard) | stone::Flag::stone(corner_guard),
                stone::Flag::stone(center_guard),
            );
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
            let Phase::Finished { violations, .. } = turn.phase else {
                panic!("Wrong phase after update");
            };
            assert_eq!(violations, expected);
        };

        let corner_take_out = delivery::Call {
            weight: seconds(2.5),
            mark: corner_guard_pos,
            rotation: Rotation::None,
        };
        run_case(corner_take_out, EnumSet::only(Violation::FreeGuardRule));

        let center_push = delivery::Call {
            weight: seconds(3.0),
            mark: center_guard_pos,
            rotation: Rotation::None,
        };
        run_case(center_push, EnumSet::only(Violation::NoTickRule));

        let center_take_out = delivery::Call {
            weight: seconds(2.5),
            mark: center_guard_pos,
            rotation: Rotation::None,
        };
        run_case(
            center_take_out,
            [Violation::FreeGuardRule, Violation::NoTickRule].into_iter().collect(),
        );
    }

    #[test]
    fn updating_and_starting_in_wrong_stage() {
        let PhaseTestSetup { mut sheet, simulation, teams, mut dirty, time, .. } =
            PhaseTestSetup::new();
        let stone = 1;
        let mut turn = Current::new(stone, 0, stone::Flag(0), stone::Flag(0));

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

        turn.phase = Phase::Finished {
            snapshot: Stones::default(),
            delivery_time: Time::default(),
            violations: EnumSet::default(),
        };
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
        };
        let finished = Current {
            played_stone: 12,
            delivering_player: 2,
            free_guards: stone::Flag::all_before(5),
            free_center_guards: stone::Flag::all_before(3),
            phase: Phase::Finished {
                delivery_time: seconds(20.0),
                snapshot: stones.clone(),
                violations: EnumSet::only(Violation::FreeGuardRule),
            },
        };
        assert!(TryInto::<Finished>::try_into(unfinished).is_err());
        let converted: Finished =
            finished.try_into().expect("Finished turn should convert successfully");
        assert_eq!(converted.played_stone, 12);
        assert_eq!(converted.delivering_player, 2);
        assert_eq!(converted.snapshot, stones);
        assert_eq!(converted.violations, EnumSet::only(Violation::FreeGuardRule));
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
        };
        let path = turn.expected_path(
            &sheet,
            &simulation,
            &teams,
            delivery::Call::tee_draw(&sheet.parameters),
        );
        let hack = sheet.parameters.geometry.hack_pos(Hack::Left);
        assert_approx_eq!(path.first().unwrap().x, hack.x);
        assert_approx_eq!(path.first().unwrap().y, hack.y);
        assert_approx_eq!(path[path.len() / 4].x, feet(2.0), epsilon = 2.0);
        assert_approx_eq!(path[path.len() / 4].y, feet(81.0), epsilon = 2.0);
        assert_approx_eq!(path[path.len() / 2].x, feet(3.0), epsilon = 2.0);
        assert_approx_eq!(path[path.len() / 2].y, feet(110.0), epsilon = 2.0);
        assert_approx_eq!(path.last().unwrap().x, feet(0.0), epsilon = 2.0);
        assert_approx_eq!(
            path.last().unwrap().y,
            sheet.parameters.geometry.playing_end.tee_line_y,
            epsilon = 2.0
        );
    }
}
