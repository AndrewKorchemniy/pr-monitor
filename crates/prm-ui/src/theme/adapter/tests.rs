use std::path::{Path, PathBuf};

use super::*;
use crate::theme::fallback;

/// Literal color constructors that would bypass the theme.
const FORBIDDEN: [&str; 6] = [
    "Color32::from_rgb",
    "Color32::from_rgba",
    "Color32::from_gray",
    "Color32::from_hex",
    "Color32::WHITE",
    "Color32::BLACK",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable src dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_color_literals_outside_the_adapter() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let offenders: Vec<String> = files
        .iter()
        .filter(|p| !p.ends_with(Path::new("theme").join("adapter.rs")))
        .filter(|p| p.file_name().is_none_or(|n| n != "tests.rs"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            FORBIDDEN
                .iter()
                .any(|f| text.contains(f))
                .then(|| p.display().to_string())
        })
        .collect();
    assert!(offenders.is_empty(), "color literals in {offenders:?}");
}

#[test]
fn status_tones_are_distinct() {
    let colors = &fallback().colors;
    let solids =
        [Tone::NeedsReview, Tone::ActiveComments, Tone::Reviewed].map(|t| colors.tone(t).solid);
    assert_ne!(solids[0], solids[1]);
    assert_ne!(solids[1], solids[2]);
    assert_ne!(solids[0], solids[2]);
}

#[test]
fn low_power_removes_animation_and_popup_shadows() {
    let theme = fallback();
    let full = style(
        &theme,
        RenderPolicy {
            low_power: false,
            animations: true,
        },
    );
    let lean = style(
        &theme,
        RenderPolicy {
            low_power: true,
            animations: false,
        },
    );
    assert!(full.animation_time > 0.0);
    assert!(lean.animation_time.abs() < f32::EPSILON);
    assert_eq!(lean.visuals.popup_shadow, Shadow::NONE);
}

#[test]
fn rgba8_converts_without_premultiplying_opaque_colors() {
    let c = color(Rgba8 {
        r: 10,
        g: 20,
        b: 30,
        a: 255,
    });
    assert_eq!(c.to_array(), [10, 20, 30, 255]);
}
