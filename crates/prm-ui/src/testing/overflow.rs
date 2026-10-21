//! Assertions that a screen fits its window, read from the accessibility tree: nothing runs past
//! the window's sides, dialogs lie inside it, and the controls a user needs can be clicked.

use eframe::egui::{self, Rect, Role};
use egui_kittest::{
    Harness, Node,
    kittest::{NodeT as _, Queryable as _},
};

/// How far a node may stick out (rounding), in points.
const SLACK: f32 = 0.5;

fn window(size: egui::Vec2) -> Rect {
    Rect::from_min_size(egui::Pos2::ZERO, size).expand(SLACK)
}

/// Asserts that what is open over the window fits a window of `size`: every dialog and popup,
/// and its frame, lies inside the window, and nothing in one runs past the window's left or
/// right edge. Content below a dialog's fold may reach past the bottom, since its body scrolls.
pub(crate) fn assert_overlays_fit<S>(h: &Harness<'_, S>, size: egui::Vec2) {
    let window = window(size);
    let dialogs: Vec<Node<'_>> = h.query_all_by_role(Role::Dialog).collect();
    assert!(!dialogs.is_empty(), "{size:?}: nothing is open");
    for dialog in dialogs {
        for frame in std::iter::once(dialog).chain(dialog.children()) {
            if let Some(rect) = bounds(&frame) {
                assert!(
                    window.contains_rect(rect),
                    "{size:?}: a dialog's frame at {rect:?} runs out of the window"
                );
            }
        }
        for node in dialog.children_recursive() {
            assert_within_sides(&node, window, size);
        }
    }
}

/// Asserts that nothing on screen runs past the window's left or right edge (a whole screen,
/// such as first run, that scrolls vertically).
pub(crate) fn assert_screen_fits<S>(h: &Harness<'_, S>, size: egui::Vec2) {
    let window = window(size);
    for node in h.root().children_recursive() {
        assert_within_sides(&node, window, size);
    }
}

/// Asserts `node` lies inside a window of `size` and is enabled, so a pointer can reach it.
pub(crate) fn assert_clickable(node: &Node<'_>, size: egui::Vec2) {
    let rect = node.rect();
    let accesskit = node.accesskit_node();
    let label = accesskit.label();
    assert!(
        window(size).contains_rect(rect),
        "{size:?}: {label:?} at {rect:?} is not inside the window"
    );
    assert!(!accesskit.is_disabled(), "{size:?}: {label:?} is disabled");
}

/// The node's bounds in points, when it has any (an empty container has none).
fn bounds(node: &Node<'_>) -> Option<Rect> {
    let rect = node.accesskit_node().bounding_box()?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "screen coordinates in a test window"
    )]
    let rect = Rect::from_min_max(
        egui::pos2(rect.x0 as f32, rect.y0 as f32),
        egui::pos2(rect.x1 as f32, rect.y1 as f32),
    );
    rect.is_positive().then_some(rect)
}

fn assert_within_sides(node: &Node<'_>, window: Rect, size: egui::Vec2) {
    let Some(rect) = bounds(node) else {
        return;
    };
    let accesskit = node.accesskit_node();
    assert!(
        rect.left() >= window.left() && rect.right() <= window.right(),
        "{size:?}: {:?} {:?} at {rect:?} runs past the side of the window",
        accesskit.role(),
        accesskit.label(),
    );
}
