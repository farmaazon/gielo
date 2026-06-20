use criterion::{Criterion, criterion_group, criterion_main};
use gielo_game::{
    sheet::{
        self, Hack,
        stone::{self, Position, Rotation, Stones},
    },
    simulation::{
        self, Simulation,
        delivery::{Process, StartingConditions, Update},
    },
    team::{self, stone::Flag},
    unit::{
        Angle, Velocity, feet, feet_per_second, feet_per_second_squared,
        feet_squared_per_second_squared, inches, milliseconds, radians, seconds, vector::Vector2,
    },
};
use std::{f64::consts::PI, hint::black_box};

struct DeliveryBench {
    stones: Stones,
    sheet_params: sheet::Parameters,
    simulation: Simulation,
    start: StartingConditions,
}

impl DeliveryBench {
    fn set_up(
        angle: Angle,
        velocity: Velocity,
        rotation: Rotation,
        delivered_stone: stone::Id,
        other_stones: impl IntoIterator<Item = (stone::Id, Position)>,
    ) -> Self {
        let sheet_params = sheet::Parameters {
            geometry: sheet::parameters::Geometry::default(),
            friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
            rotation_acc: feet_per_second_squared(245.0 / 8649.0),
            stone_radius: inches(18.0 / PI),
            static_friction: feet_squared_per_second_squared(0.25),
        };
        let stones: Stones = other_stones.into_iter().collect();
        let simulation = Simulation::new(
            simulation::Parameters {
                min_time_quantum: milliseconds(100.0),
                max_time_quantum: seconds(1000.0),
            },
            &sheet_params,
        );
        let start = StartingConditions {
            stone: delivered_stone,
            angle,
            velocity,
            hack: Hack::Left,
            rotation,
        };
        stones.in_play().contains(delivered_stone);

        black_box(Self { stones, sheet_params, simulation, start })
    }

    fn run(&self, bench: &mut criterion::Bencher) {
        bench.iter(|| {
            let mut stones_copy = self.stones.clone();
            let mut process =
                Process::new(self.start, &mut stones_copy, &self.sheet_params, &mut Flag(0));
            let mut update = Update {
                process: &mut process,
                sheet: &mut stones_copy,
                sheet_params: &self.sheet_params,
                simulation: &self.simulation,
                dirty: &mut Flag(0),
            };
            update.run(None);
        });
    }
}

fn inaccurate_tee_shot() -> DeliveryBench {
    DeliveryBench::set_up(radians(6.0 / 132.0), feet_per_second(7.0), Rotation::Clockwise, 0, [])
}

fn take_out_through_frozen_stone() -> DeliveryBench {
    let sheet_params = sheet::Parameters {
        static_friction: feet_squared_per_second_squared(0.0),
        geometry: sheet::parameters::Geometry::default(),
        friction: feet_per_second_squared(49.0 / 93.0 / 2.0),
        rotation_acc: feet_per_second_squared(245.0 / 8649.0),
        stone_radius: inches(18.0 / PI),
    };
    let tee = sheet_params.geometry.tee();
    let frozen = 0;
    let taken_out = 8;
    let delivered = 1;
    let stones = [
        (frozen, tee),
        (taken_out, tee + Vector2 { x: feet(0.0), y: sheet_params.stone_radius * 2.0 }),
    ];
    DeliveryBench::set_up(
        radians(3.0 / 132.0),
        feet_per_second(7.7777),
        Rotation::Clockwise,
        delivered,
        stones,
    )
}

fn crazy_eight() -> DeliveryBench {
    let tee = sheet::Parameters::default().geometry.tee();
    let opponents = team::stone::TEAM_IDS.a;
    let offsets_from_tee = [
        Vector2 { x: feet(-4.0), y: feet(-4.0) },
        Vector2 { x: feet(4.0), y: feet(-4.0) },
        Vector2 { x: feet(4.0), y: feet(4.0) },
        Vector2 { x: feet(-4.0), y: feet(4.0) },
        Vector2 { x: feet(0.0), y: feet(10.0) },
        Vector2 { x: feet(0.0), y: feet(-10.0) },
        Vector2 { x: feet(10.0), y: feet(0.0) },
        Vector2 { x: feet(10.0), y: feet(0.0) },
    ];
    DeliveryBench::set_up(
        radians(1.0 / 132.0),
        feet_per_second(10.0),
        Rotation::Clockwise,
        0,
        opponents.zip(offsets_from_tee.map(|off| tee + off)),
    )
}

fn simulation_benchmarks(c: &mut Criterion) {
    c.bench_function("simulation - inaccurate_tee_shot", |b| inaccurate_tee_shot().run(b));
    c.bench_function("simulation - take_out_through_frozen_stone", |b| take_out_through_frozen_stone().run(b));
    c.bench_function("simulation - crazy_eight", |b| crazy_eight().run(b));
}

criterion_group!(
    name = simulation;
    config = Criterion::default()
        .sample_size(500)
        .measurement_time(std::time::Duration::from_secs(15))
        .warm_up_time(std::time::Duration::from_secs(8));
    targets = simulation_benchmarks
);
criterion_main!(simulation);
