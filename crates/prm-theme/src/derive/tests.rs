use std::fmt::Write as _;

use super::*;
use crate::{
    builtin::builtin_sources, contrast::contrast_ratio, file::parse_theme, oklab::to_oklab,
};

fn builtin(id: &str) -> ThemeFile {
    let (stem, src) = builtin_sources()
        .iter()
        .find(|(stem, _)| *stem == id)
        .unwrap();
    parse_theme(src, stem).unwrap().0
}

fn listing(palette: &Palette) -> String {
    let mut out = String::new();
    for (key, color) in palette.iter() {
        let _ = writeln!(out, "{key} = {color}");
    }
    out
}

#[test]
fn palette_snapshot_graphite() {
    insta::assert_snapshot!(listing(&derive_palette(&builtin("graphite"))));
}

#[test]
fn palette_snapshot_vellum() {
    insta::assert_snapshot!(listing(&derive_palette(&builtin("vellum"))));
}

#[test]
fn palette_snapshot_paper() {
    insta::assert_snapshot!(listing(&derive_palette(&builtin("paper"))));
}

#[test]
fn palette_snapshot_cyanotype() {
    insta::assert_snapshot!(listing(&derive_palette(&builtin("cyanotype"))));
}

#[test]
fn contrast_seed_monotonic() {
    let mut file = builtin("graphite");
    let mut previous = 0.0;
    for contrast in [-1.0, 0.0, 0.5, 1.0] {
        file.seeds.contrast = contrast;
        let palette = derive_palette(&file);
        let ratio = contrast_ratio(palette.border_default, palette.surface_canvas);
        assert!(
            ratio > previous,
            "contrast {contrast}: {ratio} <= {previous}"
        );
        previous = ratio;
    }
}

#[test]
fn dark_elevation_lifts_light_raised_not_darker() {
    for id in ["graphite", "cyanotype", "vellum", "paper"] {
        let palette = derive_palette(&builtin(id));
        let l = |c: Rgba8| to_oklab(c).l;
        assert!(
            l(palette.surface_raised) >= l(palette.surface_canvas),
            "{id}: raised is darker than canvas"
        );
    }
    let graphite = derive_palette(&builtin("graphite"));
    assert!(to_oklab(graphite.surface_sunken).l < to_oklab(graphite.surface_canvas).l);
}

#[test]
fn on_accent_prefers_seed_colors() {
    let graphite = derive_palette(&builtin("graphite"));
    assert_eq!(graphite.text_on_accent, builtin("graphite").seeds.base);
    let cyanotype = derive_palette(&builtin("cyanotype"));
    assert_eq!(cyanotype.text_on_accent, builtin("cyanotype").seeds.base);
}

#[test]
fn feedback_defaults_follow_status_and_accent() {
    let mut file = builtin("vellum");
    file.seeds.feedback.success = None;
    file.seeds.feedback.info = None;
    let palette = derive_palette(&file);
    assert_eq!(
        palette.feedback_success_solid,
        palette.status_reviewed_solid
    );
    assert_eq!(
        palette.feedback_info_bg,
        mix(palette.surface_sunken, file.seeds.accent, 0.11)
    );
}
