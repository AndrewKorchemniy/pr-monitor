use prm_config::UiPrefs;
use prm_theme::Corners;

use super::*;

const POLICY: RenderPolicy = RenderPolicy {
    low_power: false,
    animations: true,
};

#[test]
fn a_broken_user_theme_stays_reported_after_the_selection_changes() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("broken.toml"), "this is not = [ a theme").expect("write");
    let mut runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    let mentions_broken = |r: &ThemeRuntime| r.issues.iter().any(|i| i.theme.as_str() == "broken");
    assert!(mentions_broken(&runtime), "{:?}", runtime.issues);
    let mut selection = selection_from(&UiPrefs::default());
    selection.theme = prm_theme::registry::DEFAULT_LIGHT.to_owned();
    runtime.set_selection(selection);
    assert!(mentions_broken(&runtime), "{:?}", runtime.issues);
    runtime.set_preview(Some("cyanotype".to_owned()));
    assert!(mentions_broken(&runtime));
}

#[test]
fn theme_choices_follow_the_registry_across_reloads() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    let listed = |r: &ThemeRuntime| theme_choices(r.registry());
    assert_eq!(runtime.choices(), listed(&runtime).as_slice());
    assert!(
        runtime
            .choices()
            .iter()
            .any(|(id, _)| id == prm_theme::registry::DEFAULT_DARK)
    );
    runtime.reload();
    assert_eq!(runtime.choices(), listed(&runtime).as_slice());
}

#[test]
fn the_built_in_themes_have_no_issues() {
    let dir = tempfile::tempdir().expect("temp dir");
    let runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    assert!(runtime.issues.is_empty(), "{:?}", runtime.issues);
}

fn dark_radius(runtime: &ThemeRuntime) -> prm_theme::metrics::Radius {
    runtime.dark.tokens.metrics.radius.clone()
}

#[test]
fn the_corner_preview_restyles_once_and_reverts_on_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    let rounded = dark_radius(&runtime);
    let generation = runtime.generation;

    runtime.set_look_preview(Some(Look {
        corners: Corners::Square,
        ..Look::default()
    }));
    assert_eq!(
        dark_radius(&runtime),
        prm_theme::metrics::Radius::new(Corners::Square)
    );
    assert_eq!(runtime.generation, generation + 1);
    runtime.set_look_preview(Some(Look {
        corners: Corners::Square,
        ..Look::default()
    }));
    assert_eq!(
        runtime.generation,
        generation + 1,
        "same preview, no rebuild"
    );

    runtime.set_look_preview(None);
    assert_eq!(dark_radius(&runtime), rounded);
}

#[test]
fn saved_corners_apply_and_the_preview_wins_over_them() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    let prefs = UiPrefs {
        corners: Corners::Square,
        ..UiPrefs::default()
    };
    runtime.set_selection(selection_from(&prefs));
    assert_eq!(
        dark_radius(&runtime),
        prm_theme::metrics::Radius::new(Corners::Square)
    );
    runtime.set_look_preview(Some(Look {
        corners: Corners::Rounded,
        ..Look::default()
    }));
    assert_eq!(
        dark_radius(&runtime),
        prm_theme::metrics::Radius::new(Corners::Rounded)
    );
    runtime.set_look_preview(None);
    assert_eq!(
        dark_radius(&runtime),
        prm_theme::metrics::Radius::new(Corners::Square)
    );
}

fn dark_stripes(runtime: &ThemeRuntime) -> bool {
    runtime.dark.tokens.metrics.stripes
}

#[test]
fn stripes_are_on_by_default_and_the_preview_wins_over_the_saved_choice() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime = ThemeRuntime::new(dir.path(), &UiPrefs::default(), POLICY);
    assert!(dark_stripes(&runtime));
    let generation = runtime.generation;

    let off = Look {
        stripes: false,
        ..Look::default()
    };
    runtime.set_look_preview(Some(off));
    assert!(!dark_stripes(&runtime));
    runtime.set_look_preview(Some(off));
    assert_eq!(
        runtime.generation,
        generation + 1,
        "same preview, no rebuild"
    );

    runtime.set_look_preview(None);
    assert!(dark_stripes(&runtime));

    let prefs = UiPrefs {
        striped_rows: false,
        ..UiPrefs::default()
    };
    runtime.set_selection(selection_from(&prefs));
    assert!(!dark_stripes(&runtime));
    runtime.set_look_preview(Some(Look::default()));
    assert!(dark_stripes(&runtime));
}
