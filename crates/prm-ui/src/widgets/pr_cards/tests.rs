use super::card::share;

#[test]
fn share_keeps_every_width_while_they_fit() {
    assert_eq!(share([40.0, 0.0, 70.0], 200.0), [40.0, 0.0, 70.0]);
}

#[test]
fn share_keeps_the_narrow_parts_whole_and_splits_the_rest() {
    let cases = [
        ([30.0, 120.0, 90.0], 150.0, [30.0, 60.0, 60.0]),
        ([0.0, 100.0, 100.0], 120.0, [0.0, 60.0, 60.0]),
        ([50.0, 50.0, 50.0], 90.0, [30.0, 30.0, 30.0]),
        ([10.0, 80.0, 20.0], -5.0, [0.0, 0.0, 0.0]),
    ];
    for (natural, room, expected) in cases {
        assert_eq!(share(natural, room), expected, "{natural:?} in {room}");
    }
}
