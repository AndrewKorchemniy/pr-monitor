//! The list sheet: the list area sits on `surface.sunken` below the toolbar, its top corners cut
//! away to the toolbar's colour. Checked on the shapes the frame emits, so no GPU is needed;
//! `render_sheet_gallery` draws the same scenes to PNG files for a look.

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, Shape};
use egui_kittest::Harness;
use prm_theme::Corners;

use crate::{
    testing::{
        demo::{demo_state, now, quiet_state},
        screens::{Fixture, NARROW, Shown, WINDOW, harness_at},
    },
    theme::UiTheme,
};

const THEMES: [&str; 4] = ["graphite", "vellum", "cyanotype", "paper"];

/// What the dashboard shows in a scene.
#[derive(Clone, Copy, Debug)]
enum Scene {
    /// The table, no peek.
    Table,
    /// The table with the peek docked at the right.
    Docked,
    /// A narrow window: the peek replaces the list.
    Over,
    /// A narrow window: cards.
    Cards,
    /// No pull requests.
    Empty,
    /// The table under banners.
    Banners,
}

const SCENES: [Scene; 6] = [
    Scene::Table,
    Scene::Docked,
    Scene::Over,
    Scene::Cards,
    Scene::Empty,
    Scene::Banners,
];

fn fixture(theme: &str, corners: Corners, scene: Scene) -> (Fixture, egui::Vec2) {
    let mut state = if matches!(scene, Scene::Empty) {
        quiet_state()
    } else {
        demo_state()
    };
    let mut config = (*state.config).clone();
    config.ui.theme = theme.to_owned();
    config.ui.corners = corners;
    state.config = Arc::new(config);
    let mut fixture = Fixture::new(Shown::Dashboard, state);
    fixture.now = now();
    let first = fixture.state.rows.first().map(|r| r.key.clone());
    let size = match scene {
        Scene::Table | Scene::Docked | Scene::Empty | Scene::Banners => WINDOW,
        Scene::Over | Scene::Cards => NARROW,
    };
    if matches!(scene, Scene::Docked | Scene::Over) {
        fixture.dash.peek.open = true;
        if let Some(key) = first {
            fixture.dash.view.select_key(&key);
        }
    }
    if matches!(scene, Scene::Banners) {
        super::gallery::banner_shapes(&mut fixture);
    }
    (fixture, size)
}

fn shown(theme: &str, corners: Corners, scene: Scene) -> Harness<'static, Fixture> {
    let (fixture, size) = fixture(theme, corners, scene);
    let mut h = harness_at(fixture, size);
    h.run();
    h.run_steps(1);
    h
}

fn flatten<'a>(shape: &'a Shape, into: &mut Vec<&'a Shape>) {
    if let Shape::Vec(shapes) = shape {
        for inner in shapes {
            flatten(inner, into);
        }
    } else {
        into.push(shape);
    }
}

/// Every shape of the last frame.
fn drawn<'a>(h: &'a Harness<'_, Fixture>) -> Vec<&'a Shape> {
    let mut all = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut all);
    }
    all
}

fn theme_of(h: &Harness<'_, Fixture>) -> Arc<UiTheme> {
    h.state().theme.current(&h.ctx).clone()
}

/// The biggest filled rectangle of `color`: the surface the frame painted.
fn surface(h: &Harness<'_, Fixture>, color: Color32) -> Option<Rect> {
    drawn(h)
        .into_iter()
        .filter_map(|shape| {
            let Shape::Rect(r) = shape else { return None };
            (r.fill == color).then_some(r.rect)
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
}

/// The notches painted: the bounds of every mesh made only of the canvas colour and nothing.
fn notches(h: &Harness<'_, Fixture>) -> Vec<Rect> {
    let canvas = theme_of(h).colors.surface_canvas;
    drawn(h)
        .into_iter()
        .filter_map(|shape| {
            let Shape::Mesh(mesh) = shape else {
                return None;
            };
            let only_canvas = mesh
                .vertices
                .iter()
                .all(|v| v.color == canvas || v.color == Color32::TRANSPARENT);
            let points: Vec<_> = mesh.vertices.iter().map(|v| v.pos).collect();
            (!points.is_empty() && only_canvas).then(|| Rect::from_points(&points))
        })
        .collect()
}

fn radius(h: &Harness<'_, Fixture>) -> f32 {
    f32::from(theme_of(h).tokens.metrics.radius.lg)
}

/// `notch` is the `r` x `r` square at `corner`.
fn assert_notch_at(notch: Rect, corner: egui::Pos2, r: f32, what: &str) {
    let inward = if notch.min.x < corner.x - 0.5 { -r } else { r };
    let x = if inward < 0.0 { corner.x - r } else { corner.x };
    // why: the soft edge keeps inside the square; the fan reaches both its sides.
    assert!(
        (notch.min.x - x).abs() < 0.01
            && (notch.min.y - corner.y).abs() < 0.01
            && (notch.width() - r).abs() < 0.01
            && (notch.height() - r).abs() < 0.01,
        "{what}: {notch:?} is not the {r} pt square at {corner:?}"
    );
}

#[test]
fn rounded_corners_cut_the_sheet_top_corners_to_the_toolbar_colour() {
    for name in THEMES {
        for scene in [Scene::Table, Scene::Cards, Scene::Empty, Scene::Banners] {
            let h = shown(name, Corners::Rounded, scene);
            let theme = theme_of(&h);
            let r = radius(&h);
            assert!(r > 0.0, "{name}: lg radius");
            let sheet = surface(&h, theme.colors.surface_sunken)
                .unwrap_or_else(|| panic!("{name} {scene:?}: no sunken sheet"));
            let mut found = notches(&h);
            assert_eq!(found.len(), 2, "{name} {scene:?}: {found:?}");
            found.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
            assert_notch_at(found[0], sheet.left_top(), r, "left");
            assert_notch_at(found[1], sheet.right_top(), r, "right");
        }
    }
}

#[test]
fn the_sheet_starts_below_the_toolbar_and_banners() {
    let bare = shown("graphite", Corners::Rounded, Scene::Table);
    let banners = shown("graphite", Corners::Rounded, Scene::Banners);
    let sunken = theme_of(&bare).colors.surface_sunken;
    let top = |h: &Harness<'_, Fixture>| surface(h, sunken).map(|r| r.top());
    let (bare, banners) = (top(&bare).unwrap_or(0.0), top(&banners).unwrap_or(0.0));
    assert!(bare > 0.0 && banners > bare + 20.0, "{bare} {banners}");
}

#[test]
fn square_corners_paint_no_notch_in_any_theme() {
    for name in THEMES {
        for scene in SCENES {
            let h = shown(name, Corners::Square, scene);
            assert_eq!(radius(&h), 0.0, "{name}");
            assert!(
                notches(&h).is_empty(),
                "{name} {scene:?}: {:?}",
                notches(&h)
            );
        }
    }
}

#[test]
fn a_docked_peek_takes_the_right_corner_and_the_junction_stays_square() {
    for name in THEMES {
        let h = shown(name, Corners::Rounded, Scene::Docked);
        let theme = theme_of(&h);
        let r = radius(&h);
        let sheet = surface(&h, theme.colors.surface_sunken).expect("sheet");
        let peek = surface(&h, theme.colors.surface_raised).expect("peek");
        assert!(sheet.right() <= peek.left() + 0.5, "{sheet:?} {peek:?}");
        let mut found = notches(&h);
        assert_eq!(found.len(), 2, "{name}: {found:?}");
        found.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
        assert_notch_at(found[0], sheet.left_top(), r, "the sheet's left");
        assert_notch_at(found[1], peek.right_top(), r, "the peek's right");
    }
}

#[test]
fn a_peek_over_the_list_rounds_both_its_top_corners() {
    for name in THEMES {
        let h = shown(name, Corners::Rounded, Scene::Over);
        let theme = theme_of(&h);
        let r = radius(&h);
        let peek = surface(&h, theme.colors.surface_raised).expect("peek");
        let mut found = notches(&h);
        assert_eq!(found.len(), 2, "{name}: {found:?}");
        found.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
        assert_notch_at(found[0], peek.left_top(), r, "left");
        assert_notch_at(found[1], peek.right_top(), r, "right");
    }
}

/// Measured: Graphite 2.8, Cyanotype 2.7, Vellum 3.5, Paper 3.4 L* (dark themes step darker,
/// light themes lighter).
#[test]
fn the_sheet_is_a_modest_step_from_the_canvas() {
    for name in THEMES {
        let h = shown(name, Corners::Rounded, Scene::Table);
        let c = &theme_of(&h).colors;
        let step = (lightness(c.surface_sunken) - lightness(c.surface_canvas)).abs();
        assert!((1.5..8.0).contains(&step), "{name}: {step} L*");
    }
}

/// CIE lightness L*, 0 to 100, of an sRGB colour.
fn lightness(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let y = 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
    if y > 0.008_856 {
        116.0 * y.cbrt() - 16.0
    } else {
        903.3 * y
    }
}

/// Renders every scene, theme, corner style and scale to PNG files for a look:
/// `cargo test -p prm-ui render_sheet_gallery -- --ignored`, into `$PRM_GALLERY_DIR/sheet`
/// or `target/gallery/sheet`.
#[test]
#[ignore = "renders on the GPU and writes PNG files; run on demand for design review"]
fn render_sheet_gallery() {
    let dir = super::gallery::out_dir().join("sheet");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("sheet gallery dir");
    for scene in SCENES {
        for name in THEMES {
            for (corners, ppp) in [Corners::Rounded, Corners::Square]
                .into_iter()
                .flat_map(|c| [(c, 1.0_f32), (c, 1.25)])
            {
                let file = format!("{scene:?}-{name}-{corners:?}-{ppp}x.png").to_lowercase();
                render(scene, (name, corners), ppp, &dir.join(file));
            }
        }
    }
}

/// One scene on the GPU at `ppp` pixels per point, saved to `path`.
fn render(scene: Scene, (name, corners): (&str, Corners), ppp: f32, path: &std::path::Path) {
    let (fixture, size) = fixture(name, corners, scene);
    let mut h = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_accessibility_check(false)
        .with_context_setup(crate::theme::fonts::install)
        .wgpu()
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture);
    h.run();
    h.run_steps(1);
    let image = h.render().unwrap_or_else(|e| panic!("{scene:?}: {e}"));
    image.save(path).expect("png");
}
