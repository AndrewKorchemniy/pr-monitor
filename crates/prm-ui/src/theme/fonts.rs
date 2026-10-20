//! The embedded Atkinson Hyperlegible Next (UI) and Mono (numbers, ids, branches) variable fonts,
//! registered once per weight through the `wght` axis (the bytes are shared, not copied).

use std::sync::Arc;

use eframe::{
    egui::{self, FontData, FontDefinitions, FontFamily, FontId},
    epaint::text::{FontTweak, VariationCoords},
};
use prm_theme::metrics::{FontFace, TypeSpec};

static UI: &[u8] = include_bytes!("../../assets/fonts/AtkinsonHyperlegibleNext-wght.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/AtkinsonHyperlegibleMono-wght.ttf");
pub(crate) static OFL_NEXT: &str =
    include_str!("../../assets/fonts/OFL-AtkinsonHyperlegibleNext.txt");
pub(crate) static OFL_MONO: &str =
    include_str!("../../assets/fonts/OFL-AtkinsonHyperlegibleMono.txt");

/// Mono's default instance is weight 200, so every face sets `wght` explicitly.
const FACES: [(&str, FontFace, f32); 6] = [
    ("ui-400", FontFace::Ui, 400.0),
    ("ui-500", FontFace::Ui, 500.0),
    ("ui-600", FontFace::Ui, 600.0),
    ("mono-400", FontFace::Mono, 400.0),
    ("mono-600", FontFace::Mono, 600.0),
    ("mono-800", FontFace::Mono, 800.0),
];

/// Named families for the non-regular weights (each falls back to the regular UI face).
const NAMED: [&str; 4] = ["ui-500", "ui-600", "mono-600", "mono-800"];

pub(crate) fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    for (name, face, weight) in FACES {
        let bytes = match face {
            FontFace::Ui => UI,
            FontFace::Mono => MONO,
        };
        let tweak = FontTweak {
            coords: VariationCoords::new([("wght", weight)]),
            ..FontTweak::default()
        };
        let data = FontData::from_static(bytes).tweak(tweak);
        defs.font_data.insert(name.to_owned(), Arc::new(data));
    }
    defs.families
        .insert(FontFamily::Proportional, vec!["ui-400".into()]);
    defs.families.insert(
        FontFamily::Monospace,
        vec!["mono-400".into(), "ui-400".into()],
    );
    for name in NAMED {
        defs.families.insert(
            FontFamily::Name(name.into()),
            vec![name.into(), "ui-400".into()],
        );
    }
    defs
}

/// Call before the first frame (eframe's system-font fallback still covers other scripts).
pub(crate) fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

pub(crate) fn family(face: FontFace, weight: u16) -> FontFamily {
    match (face, weight) {
        (FontFace::Ui, ..=450) => FontFamily::Proportional,
        (FontFace::Ui, 451..=550) => FontFamily::Name("ui-500".into()),
        (FontFace::Ui, _) => FontFamily::Name("ui-600".into()),
        (FontFace::Mono, ..=450) => FontFamily::Monospace,
        (FontFace::Mono, 451..=700) => FontFamily::Name("mono-600".into()),
        (FontFace::Mono, _) => FontFamily::Name("mono-800".into()),
    }
}

pub(crate) fn font_id(spec: &TypeSpec) -> FontId {
    FontId::new(spec.size, family(spec.face, spec.weight))
}
