use criterion::{Criterion, criterion_group, criterion_main};
use gielo_game::{
    Delivery,
    heatmap::{HeatmapFactory, heatmap_of_delivery_chances},
    sheet::{
        self,
        stone::{self, Position, Rotation, Stones},
    },
    simulation::{self, Simulation},
    team::{Player, player},
    unit::{Angle, Velocity, feet, feet_per_second, radians, vector::Vector2},
};

struct HeatmapBench {
    factory: HeatmapFactory,
    stones: Stones,
    played_stone: stone::Id,
    sheet: sheet::Parameters,
    player: Player<String>,
    probes_count: usize,
    plan: Delivery,
    simulation: Simulation,
}

impl HeatmapBench {
    fn set_up(
        probes_count: usize,
        angle: Angle,
        velocity: Velocity,
        rotation: Rotation,
        played_stone: stone::Id,
        other_stones: impl IntoIterator<Item = (stone::Id, Position)>,
    ) -> Self {
        let sheet = sheet::Parameters::default();
        let player: Player<String> = Player {
            skills: player::Skills {
                angle_std_dev: radians(0.015),
                velocity_std_dev: feet_per_second(1.0),
            },
            ..Player::default()
        };
        let plan = Delivery { angle, velocity, rotation };
        let simulation = Simulation::new(simulation::Parameters::default(), &sheet);
        Self {
            factory: HeatmapFactory::new(374, sheet),
            stones: other_stones.into_iter().collect(),
            played_stone,
            sheet,
            player,
            probes_count,
            plan,
            simulation,
        }
    }

    fn run(&mut self, b: &mut criterion::Bencher) {
        b.iter(|| {
            heatmap_of_delivery_chances(
                &mut self.factory,
                &self.stones,
                self.played_stone,
                &self.sheet,
                &self.player,
                self.plan,
                self.probes_count,
                &self.simulation,
            );
        });
    }
}

fn come_around() -> HeatmapBench {
    let tee = sheet::Parameters::default().geometry.tee();
    let probes_count = 1000;
    let played_stone = 9;
    let stones = [
        (0, tee + Vector2 { x: feet(0.0), y: feet(-7.0) }),
        (1, tee + Vector2 { x: feet(5.0), y: feet(-7.0) }),
        (8, tee),
    ];
    HeatmapBench::set_up(
        probes_count,
        radians(6.0 / 132.0),
        feet_per_second(7.0),
        Rotation::Clockwise,
        played_stone,
        stones,
    )
}

fn crazy_eight() -> HeatmapBench {
    let tee = sheet::Parameters::default().geometry.tee();
    let probes_count = 1000;
    let played_stone = 9;
    let stones = [
        (0, tee + Vector2 { x: feet(-4.0), y: feet(-4.0) }),
        (1, tee + Vector2 { x: feet(4.0), y: feet(-4.0) }),
        (2, tee + Vector2 { x: feet(4.0), y: feet(4.0) }),
        (3, tee + Vector2 { x: feet(-4.0), y: feet(4.0) }),
        (4, tee + Vector2 { x: feet(0.0), y: feet(10.0) }),
        (5, tee + Vector2 { x: feet(0.0), y: feet(-10.0) }),
        (6, tee + Vector2 { x: feet(10.0), y: feet(0.0) }),
        (7, tee + Vector2 { x: feet(10.0), y: feet(0.0) }),
    ];
    HeatmapBench::set_up(
        probes_count,
        radians(6.0 / 132.0),
        feet_per_second(7.0),
        Rotation::Clockwise,
        played_stone,
        stones,
    )
}

fn heatmap_benchmarks(c: &mut Criterion) {
    c.bench_function("heatmap - come_around", |b| come_around().run(b));
    c.bench_function("heatmap - crazy_eight", |b| crazy_eight().run(b));
}

criterion_group!(
    name = heatmap;
    config = Criterion::default().measurement_time(std::time::Duration::from_secs(15));
    targets = heatmap_benchmarks
);
criterion_main!(heatmap);
