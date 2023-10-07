use crate::{
    dirty::Dirty,
    game,
    game::{Game, Turn, TurnIndex},
    score::{count_score, Score},
    setup::Setup,
    sheet,
    situation::Situation,
    Delivery, MarkedDelivery, ViolatedRule,
};
use anyhow::{anyhow, bail, Result};
use gielo_sheet::{stone, stone::Stones};
use gielo_simulation as simulation;
use gielo_simulation::delivery::Process;
use gielo_team as team;
use gielo_team::{Player, Team};
use gielo_unit::Time;
use simulation::Simulation;
use std::cmp::Ordering;

#[derive(Clone, Debug)]
enum Phase {
    Thinking,
    Playing { info: PlayingInfo, process: Process },
    Violation { info: PlayingInfo, rule: ViolatedRule },
    EndFinished { score: Score },
    GameFinished,
}

#[derive(Clone, Debug, Default)]
struct PlayingInfo {
    state_before: Stones,
    plan: Delivery,
    actual: Delivery,
}

#[derive(Clone, Debug)]
pub struct RunningGame<NameT, ColorT> {
    game: Game<NameT, ColorT>,
    current: Situation,
    phase: Phase,
    simulation: Simulation,
}

impl<NameT, ColorT> RunningGame<NameT, ColorT> {
    pub fn new(setup: Setup<NameT, ColorT>) -> Self {
        let simulation = Simulation::new(setup.simulation, &setup.sheet);
        let current = setup.starting_situation.clone();
        Self { game: Game::new(setup), current, phase: Phase::Thinking, simulation }
    }

    pub fn load(game: Game<NameT, ColorT>) -> Self {
        let simulation = Simulation::new(game.setup().simulation, &game.setup().sheet);
        let current = game.setup().starting_situation.clone();
        let mut this = Self { game, current, phase: Phase::Thinking, simulation };
        if let Some(last_turn) = this.game.next_turn_index().previous() {
            this.load_after_turn(&mut Dirty::new(), last_turn).unwrap()
        }
        this
    }

    pub fn load_after_turn(&mut self, dirty: &mut Dirty, turn: TurnIndex) -> Result<()> {
        if turn < self.game.first_recorded_turn() {
            self.current.restore(dirty, self.game.setup().starting_situation.clone());
            self.phase = Phase::Thinking;
            dirty.preview = true;
            Ok(())
        } else if let Some(info) = self.game.turn(turn) {
            self.current.restore(dirty, self.game.situation_after_finished_delivery(turn).unwrap());
            if let Some(violation) = &info.violation {
                self.phase = Phase::Violation {
                    info: PlayingInfo {
                        state_before: self
                            .game
                            .stones_before(turn)
                            .ok_or_else(|| anyhow!("Cannot load situation before {turn:?}"))?,
                        plan: info.plan,
                        actual: info.actual,
                    },
                    rule: violation.rule,
                };
            } else {
                // TODO[ao]: here we store same turn in game again.
                self.finish_turn(dirty, info.plan, info.actual, None);
            }
            Ok(())
        } else {
            Err(anyhow!("Cannot load {turn:?}"))
        }
    }

    pub fn game(&self) -> &Game<NameT, ColorT> {
        &self.game
    }

    pub fn setup(&self) -> &Setup<NameT, ColorT> {
        self.game.setup()
    }

    pub fn current(&self) -> &Situation {
        &self.current
    }

    pub fn current_team_info(&self) -> &team::Info<NameT, ColorT> {
        &self.game.setup().teams[self.current.team()]
    }
    pub fn current_player(&self) -> &Player<NameT> {
        let team = self.current.team();
        let player_id = self.current.player();
        &self.game.setup().teams[team].players[player_id]
    }

    pub fn end_scores(&self) -> impl ExactSizeIterator<Item = (usize, &Score)> {
        self.game.enumerate_ends_scores()
    }

    pub fn sheet_params(&self) -> &sheet::Parameters {
        &self.setup().sheet
    }

    pub fn is_thinking(&self) -> bool {
        matches!(self.phase, Phase::Thinking)
    }

    pub fn is_delivering(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. })
    }

    pub fn violation(&self) -> Option<ViolatedRule> {
        if let Phase::Violation { rule, .. } = &self.phase {
            Some(*rule)
        } else {
            None
        }
    }

    pub fn end_finished(&self) -> Option<Score> {
        if let Phase::EndFinished { score } = self.phase {
            Some(score)
        } else {
            None
        }
    }

    pub fn game_finished(&self) -> bool {
        matches!(self.phase, Phase::GameFinished { .. })
    }

    pub fn start_delivery(&mut self, dirty: &mut Dirty, plan: Delivery) -> Result<()> {
        if let Phase::Thinking = self.phase {
            let state_before = self.current.stones.clone();
            let player = self.current_player();
            let actual = plan.apply_error(&player.skills);
            let info = PlayingInfo { state_before, plan, actual };
            let process_params = simulation::delivery::StartingConditions {
                stone: self.current.stone(),
                angle: actual.angle,
                velocity: actual.velocity,
                hack: player.used_hack,
                rotation: actual.rotation,
            };
            let process = Process::new(
                process_params,
                &mut self.current.stones,
                &self.game.setup().sheet,
                &mut dirty.stones,
            );
            self.phase = Phase::Playing { info, process };
            dirty.turn_phase = true;
            Ok(())
        } else {
            bail!("Started delivery at wrong phase")
        }
    }

    pub fn resolve_marked(&self, delivery: MarkedDelivery) -> Delivery {
        let player = self.current_player();
        delivery.to_delivery(self.sheet_params(), player.used_hack)
    }

    pub fn start_delivery_marked(&mut self, dirty: &mut Dirty, plan: MarkedDelivery) -> Result<()> {
        self.start_delivery(dirty, self.resolve_marked(plan))
    }

    pub fn update(&mut self, dirty: &mut Dirty, time: Time) {
        let finished = if let Phase::Playing { info, process } = &mut self.phase {
            let mut update = simulation::delivery::Update {
                process,
                sheet: &mut self.current.stones,
                sheet_params: &self.game.setup().sheet,
                simulation: &self.simulation,
                dirty: &mut dirty.stones,
            };
            let finished = update.run(time);
            finished.then(|| std::mem::take(info))
        } else {
            None
        };

        if let Some(info) = finished {
            if let Some(rule) = self.violated_rule(&info.state_before) {
                self.phase = Phase::Violation { info, rule };
                dirty.turn_phase = true;
            } else {
                self.finish_turn(dirty, info.plan, info.actual, None);
            }
        }
    }

    pub fn proceed(&mut self, dirty: &mut Dirty) -> Result<()> {
        if let Phase::EndFinished { score } = self.phase {
            if self.current.turn.end() + 1 >= self.game.setup().rules.ends
                && self.current.score.a != self.current.score.b
            {
                self.phase = Phase::GameFinished;
                dirty.turn_phase = true
            } else {
                self.current.next_turn(dirty);
                self.current.stones.clear(&mut dirty.stones);
                self.current.hammer = match score.a.cmp(&score.b) {
                    Ordering::Less => Team::A,
                    Ordering::Equal => self.current.hammer,
                    Ordering::Greater => Team::B,
                };
                self.phase = Phase::Thinking;
                dirty.turn_phase = true;
                dirty.preview = true;
            }
            Ok(())
        } else {
            self.resolve_violation(dirty, false).map_err(|_| anyhow!("Proceeding at wrong phase"))
        }
    }

    pub fn replace_stones(&mut self, dirty: &mut Dirty) -> Result<()> {
        self.resolve_violation(dirty, true)
    }

    fn resolve_violation(&mut self, dirty: &mut Dirty, replace_stones: bool) -> Result<()> {
        let (plan, actual, violation) = if let Phase::Violation { info, rule } = &mut self.phase {
            (
                info.plan,
                info.actual,
                game::Violation {
                    rule: *rule,
                    restored: if replace_stones || *rule == ViolatedRule::FreeGuardRule {
                        Some(std::mem::take(&mut info.state_before))
                    } else {
                        None
                    },
                },
            )
        } else {
            bail!("Resolving violation at wrong phase")
        };
        self.finish_turn(dirty, plan, actual, Some(violation));
        Ok(())
    }

    fn violated_rule(&self, state_before: &Stones) -> Option<ViolatedRule> {
        let sheet = &self.current.stones;
        let sheet_params = &self.sheet_params();
        let opponent = self.current.team().opponent();
        let opposition_stones = team::stone::STONES[opponent];
        let stone_no = self.current.turn.stone();
        let rules = &self.game.setup().rules;
        let guards_removed =
            state_before.guards(sheet_params) & opposition_stones & !sheet.in_play();
        let center_guards_ticked = state_before.center_guards(sheet_params)
            & opposition_stones
            & !sheet.center_guards(sheet_params);
        if stone_no < rules.free_guard_rule_stones && guards_removed != stone::Flag(0) {
            Some(ViolatedRule::FreeGuardRule)
        } else if stone_no < rules.no_tick_rule_stones && center_guards_ticked != stone::Flag(0) {
            Some(ViolatedRule::NoTickRule)
        } else {
            None
        }
    }

    fn finish_turn(
        &mut self,
        dirty: &mut Dirty,
        plan: Delivery,
        actual: Delivery,
        resolved_violation: Option<game::Violation>,
    ) {
        let turn = Turn {
            plan,
            actual,
            result: self.current.stones.clone(),
            violation: resolved_violation.clone(),
        };
        self.game.store_turn(self.current.turn, turn);
        if let Some(restored_stones) = resolved_violation.and_then(|v| v.restored) {
            self.current.stones.restore(&mut dirty.stones, restored_stones)
        }
        if self.current.turn.stone() + 1 >= stone::COUNT {
            let score = count_score(&self.current.stones, &self.sheet_params());
            self.current.add_to_score(dirty, score);
            self.game.store_end_score(self.current.turn.end(), score);
            self.phase = Phase::EndFinished { score };
        } else {
            self.current.next_turn(dirty);
            self.phase = Phase::Thinking;
            dirty.preview = true;
        }
        dirty.turn_phase = true;
    }

    pub fn expected_path(&self, plan: Delivery) -> Vec<stone::Position> {
        let mut stones_copy = self.current.stones.clone();
        let mut dirty = stone::Flag::default();
        let player = self.current_player();
        let process_params = simulation::delivery::StartingConditions {
            stone: self.current.stone(),
            angle: plan.angle,
            velocity: plan.velocity,
            hack: player.used_hack,
            rotation: plan.rotation,
        };
        let mut process =
            Process::new(process_params, &mut stones_copy, &self.sheet_params(), &mut dirty);
        let mut result = vec![stones_copy.positions()[self.current.stone()]];
        let mut update = simulation::delivery::Update {
            process: &mut process,
            sheet: &mut stones_copy,
            sheet_params: &self.sheet_params(),
            simulation: &self.simulation,
            dirty: &mut dirty,
        };
        update.trace_until_event(|update| {
            result.push(update.sheet.positions()[self.current.stone()])
        });
        result
    }
}
