use crate::{
    sheet, unit,
    unit::{milliseconds, seconds, Time, Velocity},
};
use serde::{Deserialize, Serialize};
use uom::{
    si::{Quantity, ISQ},
    typenum::{N1, P2, Z0},
};

type TimeQuantumFactorDimension = ISQ<N1, Z0, P2, Z0, Z0, Z0, Z0, dyn uom::Kind>;
type TimeQuantumFactor = Quantity<TimeQuantumFactorDimension, unit::Units, unit::BaseType>;

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Parameters {
    pub min_time_quantum: Time,
    pub max_time_quantum: Time,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { min_time_quantum: milliseconds(100.0), max_time_quantum: seconds(10.0) }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Simulation {
    pub parameters: Parameters,
    time_quantum_factor: TimeQuantumFactor,
}

impl Simulation {
    pub fn new(parameters: Parameters, sheet: &sheet::Parameters) -> Self {
        Self { parameters, time_quantum_factor: Self::compute_time_quantum_factor(sheet) }
    }

    pub fn update_sheet_parameters(&mut self, sheet: &sheet::Parameters) {
        self.time_quantum_factor = Self::compute_time_quantum_factor(sheet)
    }

    fn compute_time_quantum_factor(sheet: &sheet::Parameters) -> TimeQuantumFactor {
        let angle_epsilon = 1.0 / sheet.rotation_acc.value / 65536.0;
        // let angle_epsilon = 1.0 / 128.0;
        let u = (sheet.friction / sheet.rotation_acc * angle_epsilon).value.exp();
        (u - 1.0) / (u * sheet.friction)
    }

    #[cfg(test)]
    pub(crate) fn new_mock(parameters: Parameters, time_quantum_factor: TimeQuantumFactor) -> Self {
        Self { parameters, time_quantum_factor }
    }

    pub fn new_time_quantum_for_stone(&self, v: Velocity) -> Time {
        let adaptive_quantum = self.time_quantum_factor * v;
        adaptive_quantum.min(self.parameters.max_time_quantum).max(self.parameters.min_time_quantum)
    }
}
