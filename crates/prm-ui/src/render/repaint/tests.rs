use prm_domain::testing::ts;

use super::*;

/// 30 s into a minute.
const NOW: i64 = 1_792_411_530;

fn inputs() -> RepaintInputs<'static> {
    RepaintInputs {
        visible: true,
        focused: true,
        low_power: false,
        now: Some(ts(NOW)),
        next_refresh: None,
        deadlines: &[],
    }
}

#[test]
fn hidden_windows_never_schedule_a_repaint() {
    let hidden = RepaintInputs {
        visible: false,
        ..inputs()
    };
    assert_eq!(next_repaint(&hidden), None);
}

#[test]
fn visible_windows_wake_at_the_next_minute() {
    assert_eq!(next_repaint(&inputs()), Some(Duration::from_secs(30)));
}

#[test]
fn the_last_minute_of_a_countdown_ticks_every_second_only_when_focused() {
    let soon = RepaintInputs {
        next_refresh: Some(ts(NOW + 20)),
        ..inputs()
    };
    assert_eq!(next_repaint(&soon), Some(Duration::from_secs(1)));
    let unfocused = RepaintInputs {
        focused: false,
        ..soon.clone()
    };
    assert_eq!(next_repaint(&unfocused), Some(Duration::from_secs(30)));
    let low_power = RepaintInputs {
        low_power: true,
        ..soon
    };
    assert_eq!(next_repaint(&low_power), Some(Duration::from_secs(30)));
}

#[test]
fn deadlines_pull_the_repaint_earlier_and_past_ones_mean_now() {
    let soon = [None, Some(ts(NOW + 4)), None];
    let pending = RepaintInputs {
        deadlines: &soon,
        ..inputs()
    };
    assert_eq!(next_repaint(&pending), Some(Duration::from_secs(4)));
    let past = [Some(ts(NOW - 4))];
    let overdue = RepaintInputs {
        deadlines: &past,
        ..inputs()
    };
    assert_eq!(next_repaint(&overdue), Some(Duration::ZERO));
}
