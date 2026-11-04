use super::*;

#[test]
fn row_metrics_are_fixed() {
    let metrics = Metrics::new(&MetricTweaks::default(), Corners::Rounded);
    assert_eq!(metrics.row, ROW);
    assert!((ROW.row_height - 30.0).abs() < f32::EPSILON);
    assert!((ROW.statusbar_height - 32.0).abs() < f32::EPSILON);
}

#[test]
fn square_corners_are_radius_zero_everywhere() {
    let tweaks = MetricTweaks { stripes: false };
    let metrics = Metrics::new(&tweaks, Corners::Square);
    assert_eq!(
        metrics.radius,
        Radius {
            xs: 0,
            sm: 0,
            md: 0,
            lg: 0,
            pill: 0
        }
    );
    assert_eq!(metrics.corners, Corners::Square);
    assert!(!metrics.stripes);
}

#[test]
fn rounded_corners_use_the_base_radii() {
    let metrics = Metrics::new(&MetricTweaks::default(), Corners::Rounded);
    assert_eq!(
        metrics.radius,
        Radius {
            xs: 2,
            sm: 3,
            md: 5,
            lg: 8,
            pill: 255
        }
    );
    assert_eq!(metrics.corners, Corners::Rounded);
}

#[test]
fn corners_default_to_rounded_and_unknown_values_fall_back() {
    #[derive(Deserialize)]
    struct Holder {
        corners: Corners,
    }
    let parse = |text: &str| toml::from_str::<Holder>(text).unwrap().corners;
    assert_eq!(Corners::default(), Corners::Rounded);
    assert_eq!(parse("corners = \"square\""), Corners::Square);
    assert_eq!(parse("corners = \"rounded\""), Corners::Rounded);
    assert_eq!(parse("corners = \"bevelled\""), Corners::Rounded);
    assert!(Corners::Rounded.is_rounded());
    assert!(!Corners::Square.is_rounded());
}

#[test]
fn type_ramp_never_below_12() {
    for role in TypeRole::ALL {
        let spec = role.spec();
        assert!(spec.size >= 12.0, "{role:?}");
        assert!(spec.line_height > spec.size, "{role:?}");
    }
    assert_eq!(TypeRole::Display.spec().face, FontFace::Mono);
    assert_eq!(TypeRole::Body.spec().face, FontFace::Ui);
}
