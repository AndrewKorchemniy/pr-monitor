use std::f32::consts::TAU;

use super::{SPIN_PERIOD, spin_angle};

#[test]
fn a_spinner_turns_once_a_period_and_starts_over() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    assert!(close(spin_angle(0.0), 0.0));
    assert!(close(spin_angle(SPIN_PERIOD / 2.0), TAU / 2.0));
    assert!(close(spin_angle(SPIN_PERIOD * 3.25), TAU / 4.0));
    assert!(spin_angle(SPIN_PERIOD * 1_000.9) < TAU);
}
