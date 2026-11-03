//! Dialogs stay still while you type and pick: the title's top never moves, and a dialog with a
//! fixed height keeps its foot where it is. Every `step` is a real repaint (the text cursor
//! blinks between steps), because an area egui places from its previous size moved only then.

use std::sync::Arc;

use eframe::egui::{self, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable as _};

use crate::{
    copy,
    screens::settings::{Editor, Page},
    testing::{
        demo::demo_state,
        screens::{Fixture, Shown},
    },
};

/// The smallest window the app allows (`render::MIN_SIZE`).
pub(super) const SMALLEST: egui::Vec2 = crate::testing::screens::NARROW;
pub(super) const ROOMY: egui::Vec2 = egui::vec2(1280.0, 760.0);

/// Where a dialog's head and foot sit, in points.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct Marks {
    pub title_top: f32,
    pub button_bottom: f32,
}

/// A harness at `size` and display `scale` whose window is focused, so the cursor blinks.
pub(super) fn dialog_harness(
    fixture: Fixture,
    size: egui::Vec2,
    scale: f32,
) -> Harness<'static, Fixture> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(scale)
        .with_step_dt(0.3)
        .with_context_setup(crate::theme::fonts::install)
        .build_ui_state(|ui, f: &mut Fixture| f.draw(ui), fixture);
    // why: egui paints (and blinks) the text cursor only in a focused window.
    h.event(egui::Event::WindowFocused(true));
    h.run();
    h
}

/// The title and the last button labelled `button` (the dialog's, drawn after the page's).
pub(super) fn marks(h: &Harness<'_, Fixture>, title: &str, button: &str) -> Marks {
    let title = h
        .query_all_by_label(title)
        .last()
        .unwrap_or_else(|| panic!("no title {title:?}"))
        .rect();
    let button = h
        .query_all_by_role_and_label(Role::Button, button)
        .last()
        .unwrap_or_else(|| panic!("no button {button:?}"))
        .rect();
    Marks {
        title_top: title.min.y,
        button_bottom: button.max.y,
    }
}

/// The marks of the next `count` repaints.
pub(super) fn frames(
    h: &mut Harness<'_, Fixture>,
    count: usize,
    (title, button): (&str, &str),
) -> Vec<Marks> {
    (0..count)
        .map(|_| {
            h.step();
            marks(h, title, button)
        })
        .collect()
}

/// Every mark in `seen` is the first one.
pub(super) fn assert_still(seen: &[Marks], what: &str) {
    let first = seen[0];
    for (frame, mark) in seen.iter().enumerate() {
        assert_eq!(*mark, first, "{what}: frame {frame} of {seen:?}");
    }
}

/// Every title is where the first one was, whatever the rest of the dialog did.
pub(super) fn assert_title_still(seen: &[Marks], what: &str) {
    let first = seen[0].title_top;
    for (frame, mark) in seen.iter().enumerate() {
        assert_eq!(mark.title_top, first, "{what}: frame {frame} of {seen:?}");
    }
}

pub(super) fn editor_title(editor: Editor) -> &'static str {
    match editor {
        Editor::Reviewers => copy::REVIEWERS_COUNT,
        Editor::Ignored => copy::IGNORE_FROM,
        Editor::Repositories => copy::STEP_REPOS,
        Editor::Tracked | Editor::AddOrg | Editor::Licenses => copy::TRACK_FROM,
    }
}

pub(super) fn settings_fixture(editor: Editor) -> Fixture {
    let mut fixture = Fixture::new(Shown::Settings, demo_state());
    fixture.settings.page = Page::Organizations;
    fixture.settings.editing = Some(editor);
    fixture.settings.draft.orgs[0].scope.all_projects = false;
    fixture
}

/// The window sizes and display scales the dialogs are checked at.
pub(super) fn windows() -> impl Iterator<Item = (egui::Vec2, f32)> {
    [ROOMY, egui::vec2(1003.0, 731.0), SMALLEST]
        .into_iter()
        .flat_map(|size| [1.0, 1.25, 1.5].map(|scale| (size, scale)))
}

/// An error under the token field grows the dialog; it may only grow downward.
#[test]
fn the_token_dialog_grows_downward_only() {
    let title = (copy::PAT_TITLE, copy::SAVE_TOKEN);
    for (size, scale) in windows() {
        let mut h = dialog_harness(Fixture::new(Shown::Token, demo_state()), size, scale);
        let before = frames(&mut h, 3, title);
        h.state_mut().token.1 = Some(Arc::from("That token was rejected, so it was not saved."));
        let after = frames(&mut h, 3, title);
        assert_still(&before, "before the error");
        assert_still(&after, "after the error");
        let context = format!("{size:?} x{scale}");
        assert_eq!(before[0].title_top, after[0].title_top, "{context}");
        assert!(
            after[0].button_bottom > before[0].button_bottom,
            "{context}: {before:?} then {after:?}"
        );
    }
}
