use prm_config::UiPrefs;

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
