use super::*;
use gpui::SpringState;

fn settle_ms(config: SpringConfig) -> f32 {
    let start = SpringState { position: 0.0, velocity: 0.0 };
    config.settle_time(start, 1.0, EPSILON).as_secs_f32() * 1000.0
}

#[test]
fn springs_never_overshoot() {
    for config in [ENTER, EXIT, LINGER] {
        let (_, damping_ratio) = config.canonical();
        assert!(damping_ratio >= 1.0 - 1e-4, "damping ratio {damping_ratio}");
    }
}

#[test]
fn transitions_stay_brief_and_exit_faster() {
    let enter = settle_ms(ENTER);
    let exit = settle_ms(EXIT);
    assert!(enter <= 150.0, "enter settles in {enter} ms");
    assert!(exit < enter, "exit {exit} ms, enter {enter} ms");
}

#[test]
fn linger_stays_under_a_quarter_second() {
    let linger = settle_ms(LINGER);
    assert!(linger > settle_ms(ENTER) && linger <= 250.0, "linger settles in {linger} ms");
}
