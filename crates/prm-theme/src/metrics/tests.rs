use super::*;

#[test]
fn density_metrics_rows_24_30() {
    assert!((Density::Compact.metrics().row_height - 24.0).abs() < f32::EPSILON);
    assert!((Density::Comfortable.metrics().row_height - 30.0).abs() < f32::EPSILON);
}

#[test]
fn radius_scale_zero_is_square() {
    let tweaks = MetricTweaks {
        radius_scale: 0.0,
        stripes: false,
    };
    let metrics = Metrics::new(Density::Compact, &tweaks);
    assert_eq!(
        metrics.radius,
        Radius {
            sm: 0,
            md: 0,
            lg: 0,
            pill: 0
        }
    );
    assert!(!metrics.stripes);
}

#[test]
fn radius_scale_one_is_base() {
    let metrics = Metrics::new(Density::Comfortable, &MetricTweaks::default());
    assert_eq!(
        metrics.radius,
        Radius {
            sm: 3,
            md: 5,
            lg: 8,
            pill: 255
        }
    );
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
