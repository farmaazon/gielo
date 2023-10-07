use gielo_game::{
    sheet,
    sheet::{
        stone::{Flag, Rotation, Stones},
        Hack,
    },
    simulation,
    simulation::Simulation,
    unit::{assert_float_eq, feet, seconds, vector::Vector2, ConstZero, Length},
};

#[test]
fn tee_shot_parameters() {
    #[derive(Debug)]
    struct Case {
        hog_to_hog_s: f64,
        curl_offset_feet: f64,
    }

    impl Case {
        fn run(self) {
            log::debug!("Running case: {self:?}");
            let hog_to_hog = seconds(self.hog_to_hog_s);
            let curl_offset = feet(self.curl_offset_feet);
            let parameters =
                sheet::Parameters::default().with_tee_shot_parameters(hog_to_hog, curl_offset);
            let tee = parameters.geometry.tee();

            let velocity = parameters.velocity_for_hog_to_hog_time(hog_to_hog);
            assert_float_eq!(velocity, parameters.velocity_for_target_y(tee.y));
            let mark = tee + Vector2 { x: curl_offset, y: Length::ZERO };

            let mut dirty = Flag(0);
            let simulation = Simulation::new(Default::default(), &parameters);
            let mut sheet = Stones::new();
            let start = simulation::delivery::StartingConditions {
                stone: 0,
                velocity,
                angle: parameters.angle_from_mark(mark, Hack::default()),
                rotation: Rotation::Clockwise,
                hack: Default::default(),
            };
            let mut process =
                simulation::delivery::Process::new(start, &mut sheet, &parameters, &mut dirty);
            assert!(simulation::delivery::Update {
                process: &mut process,
                sheet: &mut sheet,
                sheet_params: &parameters,
                simulation: &simulation,
                dirty: &mut dirty,
            }
            .run(seconds(120.0)));
            assert_float_eq!(sheet.positions()[0].x, tee.x, abs <= 0.5);
            assert_float_eq!(sheet.positions()[0].y, tee.y, abs <= 0.5);
        }
    }

    for case in [
        Case { hog_to_hog_s: 14.5, curl_offset_feet: 5.0 },
        Case { hog_to_hog_s: 11.0, curl_offset_feet: 5.0 },
        Case { hog_to_hog_s: 14.5, curl_offset_feet: 1.0 },
        Case { hog_to_hog_s: 11.0, curl_offset_feet: 1.0 },
        Case { hog_to_hog_s: 25.0, curl_offset_feet: 8.0 },
    ] {
        case.run();
    }
}
