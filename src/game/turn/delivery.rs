use crate::game::dirty::Dirty;
use crate::game::sheet::stone::{Rotation, Stone};
use crate::game::sheet::{stone, Sheet};
pub use crate::game::turn::delivery::process::Process;
use crate::game::{sheet, Team};
use crate::unit::{Angle, Length, Time};
use crate::vector::Vector2;

pub mod process;

#[derive(Copy, Clone, Debug)]
pub struct Call {
    pub weight: Time,
    pub mark: Vector2<Length>,
    pub rotation: Rotation,
}

impl Call {
    pub fn angle(&self, from: sheet::Hack, sheet: &sheet::Parameters) -> Angle {
        let offset = self.mark - sheet.geometry.hack_pos(from);
        (offset.x / offset.y).atan()
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(sheet: &sheet::Parameters) -> Self {
        use crate::unit::{feet, seconds};
        Call {
            weight: seconds(3.0),
            mark: sheet.geometry.tee() + Vector2 { x: feet(5.0), y: feet(0.0) },
            rotation: Rotation::Clockwise,
        }
    }
}

pub struct Start<'a, 'b> {
    pub call: Call,
    pub sheet: &'a mut Sheet,
    pub dirty: &'b mut Dirty,
}

impl<'a, 'b> Start<'a, 'b> {
    pub fn resolve(self, team: Team) -> ResolvedStart<'a, 'b> {
        let hack = sheet::Hack::Left;
        ResolvedStart {
            angle: self.call.angle(hack, &self.sheet.parameters),
            weight: self.call.weight,
            team,
            hack,
            rotation: self.call.rotation,
            sheet: self.sheet,
            dirty: self.dirty,
        }
    }

    #[cfg(test)]
    pub(crate) fn tee_draw(dirty: &'b mut Dirty, sheet: &'a mut Sheet) -> Self {
        Self { call: Call::tee_draw(&sheet.parameters), sheet, dirty }
    }
}

pub struct ResolvedStart<'a, 'b> {
    pub angle: Angle,
    pub weight: Time,
    pub team: Team,
    pub hack: sheet::Hack,
    pub rotation: Rotation,
    pub sheet: &'a mut Sheet,
    pub dirty: &'b mut Dirty,
}

impl<'a, 'b> ResolvedStart<'a, 'b> {
    pub fn add_delivered_stone(&mut self) {
        let delivering_dist = self.sheet.parameters.geometry.delivery_dist();
        let measure_dist = self.sheet.parameters.geometry.measure_dist();
        let release_time = self.weight * delivering_dist / measure_dist;
        let delivering_off = Vector2 {
            x: delivering_dist * self.angle.sin(),
            y: delivering_dist * self.angle.cos(),
        };
        let delivered_stone = Stone::new(
            self.team,
            stone::State::BeingDelivered(stone::state::BeingDelivered {
                release_time,
                starting_point: self.sheet.parameters.geometry.hack_pos(self.hack),
                delivering_off,
                rotation: self.rotation,
            }),
        );
        self.sheet.stones.push(self.dirty, delivered_stone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sheet::Hack;
    use crate::unit::{assert_approx_eq, degrees, feet, radians, seconds};

    #[test]
    fn compute_angle() {
        fn test_case((x, y): (f32, f32), expected: Angle) {
            let sheet = sheet::Parameters::default();
            for hack in [Hack::Left, Hack::Right] {
                let mark = sheet.geometry.hack_pos(hack) + Vector2 { x: feet(x), y: feet(y) };
                let call = Call { mark, weight: Time::default(), rotation: Rotation::Clockwise };
                assert_approx_eq!(call.angle(hack, &sheet), expected);
            }
        }

        test_case((-6.0, 6.0), degrees(-45.0));
        test_case((0.0, 6.0), degrees(0.0));
        test_case((6.0, 6.0), degrees(45.0));
    }

    #[test]
    fn adding_delivered_stone() {
        let mut sheet = Sheet::new(sheet::Parameters::default());
        let mut dirty = Dirty::new();
        let hack = Hack::Left;
        let team = Team::A;
        let rotation = Rotation::CounterClockwise;

        let mut run_case =
            |angle: Angle, weight: Time, expected_offset: Vector2<Length>, expected_time: Time| {
                let stone_count_before = sheet.stones.len();
                let mut delivery = ResolvedStart {
                    angle,
                    weight,
                    sheet: &mut sheet,
                    dirty: &mut dirty,
                    hack,
                    team,
                    rotation,
                };
                delivery.add_delivered_stone();
                assert_eq!(sheet.stones.len(), stone_count_before + 1);
                let last_stone = sheet.stones.last().unwrap();
                assert_eq!(last_stone.team(), team);
                let stone_state = match last_stone.state() {
                    stone::State::BeingDelivered(state) => state,
                    _ => panic!("Wrong delivered stone state"),
                };
                assert_eq!(stone_state.starting_point, sheet.parameters.geometry.hack_pos(hack));
                assert_eq!(stone_state.rotation, rotation);
                assert_approx_eq!(stone_state.release_time, expected_time);
                assert_approx_eq!(stone_state.delivering_off.x, expected_offset.x, epsilon = 0.1);
                assert_approx_eq!(stone_state.delivering_off.y, expected_offset.y, epsilon = 0.1);
            };

        run_case(degrees(0.0), seconds(2.1), Vector2 { x: feet(0.0), y: feet(33.0) }, seconds(3.3));
        run_case(
            radians(6.0 / 132.0),
            seconds(2.8),
            Vector2 { x: feet(6.0 * 33.0 / 132.0), y: feet(33.0) },
            seconds(4.4),
        );
        run_case(
            -radians(6.0 / 132.0),
            seconds(3.5),
            Vector2 { x: -feet(6.0 * 33.0 / 132.0), y: feet(33.0) },
            seconds(5.5),
        );
    }
}
