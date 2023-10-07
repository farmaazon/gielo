use gielo_game::{
    dirty::Dirty,
    game::TurnIndex,
    running::RunningGame,
    score::Score,
    setup::Rules,
    sheet,
    sheet::{
        stone,
        stone::{Rotation, Stones},
        Hack,
    },
    situation::Situation,
    team,
    team::Team,
    unit::{feet, feet_per_second, seconds, vector::Vector2},
    Delivery, MarkedDelivery, Setup, ViolatedRule,
};

#[test]
fn progressing_turn() {
    let stone = team::stone::QUEUE_BY_HAMMER.b[0];
    let mut game = RunningGame::new(Setup::default());
    let mut dirty = Dirty::new();
    assert!(game.is_thinking());
    assert_eq!(game.current().turn, TurnIndex::new(0, 0));
    assert_eq!(game.current().player(), 0);
    assert_eq!(game.current().team(), Team::A);

    let delivery = MarkedDelivery::tee_draw(&game.sheet_params());
    game.start_delivery_marked(&mut dirty, delivery).expect("Starting delivery failed");
    assert!(game.is_delivering());
    assert_eq!(game.current().turn, TurnIndex::new(0, 0));
    assert_eq!(game.current().player(), 0);
    assert_eq!(game.current().team(), Team::A);
    assert_eq!(game.current().stones.in_play(), stone::Flag::stone(stone));
    let last_stone_pos = game.current().stones.positions()[stone];
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(stone),
        turn_phase: true,
        ..Dirty::default()
    });

    let first_update = seconds(10.0);
    game.update(&mut dirty, first_update);
    assert!(game.is_delivering());
    assert_eq!(game.current().turn, TurnIndex::new(0, 0));
    assert_eq!(game.current().player(), 0);
    assert_eq!(game.current().team(), Team::A);
    assert_eq!(game.current().stones.in_play(), stone::Flag::stone(stone));
    assert_ne!(game.current().stones.positions()[stone], last_stone_pos);
    let last_stone_pos = game.current().stones.positions()[stone];
    dirty.check_and_clear(&Dirty { stones: stone::Flag::stone(stone), ..Dirty::default() });

    let finishing_update = seconds(70.0);
    game.update(&mut dirty, finishing_update);
    assert!(game.is_thinking());
    assert_eq!(game.current().turn, TurnIndex::new(0, 1));
    assert_eq!(game.current().player(), 0);
    assert_eq!(game.current().team(), Team::B);
    assert_eq!(game.current().stones.in_play(), stone::Flag::stone(stone));
    assert_ne!(game.current().stones.positions()[stone], last_stone_pos);
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(stone),
        turn: true,
        turn_phase: true,
        preview: true,
        ..Dirty::new()
    });
}

#[test]
fn violations() {
    let sheet = sheet::Parameters::default();
    let queue = team::stone::QUEUE_BY_HAMMER.b;
    let center_guard_pos = sheet.geometry.tee()
        + Vector2 { x: feet(0.0), y: -sheet.geometry.house_radius - feet(4.0) };
    let corner_guard_pos = sheet.geometry.tee()
        + Vector2 {
            x: sheet.geometry.house_radius / 2.0,
            y: -sheet.geometry.house_radius - feet(4.0),
        };
    let center_guard = queue[0];
    let corner_guard = queue[2];
    let opponent_stone = TurnIndex::new(0, 3);
    let same_team_stone = TurnIndex::new(0, 4);

    let run_case = |turn: TurnIndex,
                    plan: Delivery,
                    expected: Option<ViolatedRule>,
                    auto_restore: bool| {
        let initial_stones =
            Stones::from_iter([(center_guard, center_guard_pos), (corner_guard, corner_guard_pos)]);
        let mut game = RunningGame::new(Setup {
            sheet,
            starting_situation: Situation {
                turn,
                stones: initial_stones.clone(),
                ..Situation::default()
            },
            ..Setup::default()
        });
        let mut dirty = Dirty::new();
        game.start_delivery(&mut dirty, plan).expect("Failed to start delivery");
        game.update(&mut dirty, seconds(100.0));
        assert_eq!(game.violation(), expected);

        if game.violation().is_some() {
            let mut replacing_stones = game.clone();
            let mut proceeded_game = game;

            replacing_stones.replace_stones(&mut dirty).expect("Failed to replace stones");
            assert!(replacing_stones.is_thinking());
            assert_eq!(replacing_stones.current().turn, turn.next());
            assert_eq!(replacing_stones.current().stones, initial_stones);

            proceeded_game.proceed(&mut dirty).expect("Failed to proceed");
            assert!(proceeded_game.is_thinking());
            assert_eq!(proceeded_game.current().turn, turn.next());
            if auto_restore {
                assert_eq!(proceeded_game.current().stones, initial_stones);
            } else {
                assert_ne!(proceeded_game.current().stones, initial_stones);
            }
        }
    };

    let corner_take_out = Delivery {
        velocity: sheet.velocity_for_hog_to_hog_time(seconds(9.0)),
        angle: sheet.angle_from_mark(corner_guard_pos, Hack::Left),
        rotation: Rotation::None,
    };
    run_case(opponent_stone, corner_take_out, Some(ViolatedRule::FreeGuardRule), true);
    run_case(same_team_stone, corner_take_out, None, false);

    let center_push = Delivery {
        velocity: sheet.velocity_for_hog_to_hog_time(seconds(14.0)),
        angle: sheet.angle_from_mark(center_guard_pos, Hack::Left),
        rotation: Rotation::None,
    };
    run_case(opponent_stone, center_push, Some(ViolatedRule::NoTickRule), false);
    run_case(same_team_stone, center_push, None, false);

    let center_take_out = Delivery {
        velocity: sheet.velocity_for_hog_to_hog_time(seconds(9.0)),
        angle: sheet.angle_from_mark(center_guard_pos, Hack::Left),
        rotation: Rotation::None,
    };
    run_case(opponent_stone, center_take_out, Some(ViolatedRule::FreeGuardRule), true);
    run_case(same_team_stone, center_take_out, None, false);
}

#[test]
fn finishing_end() {
    let mut game = RunningGame::new(Setup {
        starting_situation: Situation {
            turn: TurnIndex::new(0, stone::COUNT - 1),
            hammer: Team::A,
            ..Situation::default()
        },
        ..Setup::default()
    });
    let mut dirty = Dirty::new();
    game.start_delivery_marked(&mut dirty, MarkedDelivery::tee_draw(&game.setup().sheet)).unwrap();
    game.update(&mut dirty, seconds(70));
    assert_eq!(game.end_finished(), Some(Score { a: 1, b: 0 }));
    assert_eq!(game.current().turn, TurnIndex::new(0, stone::COUNT - 1));
    assert_eq!(game.current().hammer, Team::A);
    assert_eq!(game.current().score, Score { a: 1, b: 0 });
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(*team::stone::QUEUE_BY_HAMMER.a.last().unwrap()),
        score: true,
        turn_phase: true,
        ..Dirty::default()
    });

    game.proceed(&mut dirty).unwrap();
    assert!(game.is_thinking());
    assert_eq!(game.current().turn, TurnIndex::new(1, 0));
    assert_eq!(game.current().stones.in_play(), stone::Flag::default());
    assert_eq!(game.current().hammer, Team::B);
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(*team::stone::QUEUE_BY_HAMMER.a.last().unwrap()),
        turn_phase: true,
        turn: true,
        preview: true,
        ..Dirty::default()
    });
}

#[test]
fn finishing_blank_end() {
    let mut game = RunningGame::new(Setup {
        starting_situation: Situation {
            turn: TurnIndex::new(0, stone::COUNT - 1),
            hammer: Team::A,
            ..Situation::default()
        },
        ..Setup::default()
    });
    let mut dirty = Dirty::new();
    game.start_delivery(
        &mut dirty,
        Delivery { velocity: feet_per_second(50.0), ..Delivery::default() },
    )
    .unwrap();
    game.update(&mut dirty, seconds(70));

    game.proceed(&mut dirty).unwrap();
    assert!(game.is_thinking());
    assert_eq!(game.current().turn, TurnIndex::new(1, 0));
    assert_eq!(game.current().hammer, Team::A);
}

#[test]
fn finishing_game() {
    let played_ends = 7;
    let mut game = RunningGame::new(Setup {
        starting_situation: Situation {
            turn: TurnIndex::new(played_ends - 1, stone::COUNT - 1),
            hammer: Team::A,
            score: Score { a: 4, b: 7 },
            ..Situation::default()
        },
        rules: Rules { ends: played_ends, ..Rules::default() },
        ..Setup::default()
    });
    let mut dirty = Dirty::new();
    game.start_delivery_marked(&mut dirty, MarkedDelivery::tee_draw(&game.setup().sheet)).unwrap();
    game.update(&mut dirty, seconds(70));
    assert_eq!(game.end_finished(), Some(Score { a: 1, b: 0 }));
    assert_eq!(game.current().turn, TurnIndex::new(played_ends - 1, stone::COUNT - 1));
    assert_eq!(game.current().score, Score { a: 5, b: 7 });
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(*team::stone::QUEUE_BY_HAMMER.a.last().unwrap()),
        score: true,
        turn_phase: true,
        ..Dirty::default()
    });

    game.proceed(&mut dirty).unwrap();
    assert!(game.game_finished());
    dirty.check_and_clear(&Dirty { turn_phase: true, ..Dirty::default() });
}

#[test]
fn extra_end() {
    let played_ends = 7;
    let mut game = RunningGame::new(Setup {
        starting_situation: Situation {
            turn: TurnIndex::new(played_ends - 1, stone::COUNT - 1),
            hammer: Team::A,
            score: Score { a: 6, b: 7 },
            ..Situation::default()
        },
        rules: Rules { ends: played_ends, ..Rules::default() },
        ..Setup::default()
    });
    let mut dirty = Dirty::new();
    game.start_delivery_marked(&mut dirty, MarkedDelivery::tee_draw(&game.setup().sheet)).unwrap();
    game.update(&mut dirty, seconds(70));
    assert_eq!(game.end_finished(), Some(Score { a: 1, b: 0 }));
    assert_eq!(game.current().turn, TurnIndex::new(played_ends - 1, stone::COUNT - 1));
    assert_eq!(game.current().score, Score { a: 7, b: 7 });
    dirty.check_and_clear(&Dirty {
        stones: stone::Flag::stone(*team::stone::QUEUE_BY_HAMMER.a.last().unwrap()),
        score: true,
        turn_phase: true,
        ..Dirty::default()
    });

    game.proceed(&mut dirty).unwrap();
    assert!(game.is_thinking());
    assert_eq!(game.current().turn, TurnIndex::new(played_ends, 0));
}
