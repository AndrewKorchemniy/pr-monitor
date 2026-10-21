//! Assertions that a screen fits its window, read from the accessibility tree and egui's areas:
//! every dialog, menu and popup lies inside the window, nothing in one runs past its sides,
//! nothing laid out beside something else is drawn over it, and the controls a user needs can be
//! clicked.

use eframe::egui::{self, Rect, Role};
use egui_kittest::{Harness, Node, kittest::NodeT as _};

/// How far a node may stick out (rounding), in points.
const SLACK: f32 = 0.5;

fn window(size: egui::Vec2) -> Rect {
    Rect::from_min_size(egui::Pos2::ZERO, size).expand(SLACK)
}

/// Asserts that what is open over the window fits a window of `size`: every dialog, menu and
/// popup lies inside the window, nothing in one runs past its own left or right edge, and no
/// two things laid out side by side in it overlap. Content below a dialog's fold may reach past
/// its bottom, since its body scrolls.
pub(crate) fn assert_overlays_fit<S>(h: &Harness<'_, S>, size: egui::Vec2) {
    let window = window(size);
    let overlays = overlays(h);
    assert!(!overlays.is_empty(), "{size:?}: nothing is open");
    for (overlay, frame) in overlays {
        assert!(
            window.contains_rect(frame),
            "{size:?}: a {:?} at {frame:?} runs out of the window",
            overlay.accesskit_node().role(),
        );
        for node in own_nodes(overlay) {
            assert_within_sides(&node, frame.expand(SLACK), size);
        }
        assert_no_overlap(overlay, size);
    }
}

/// Asserts that nothing on screen runs past the window's left or right edge, and that nothing
/// laid out side by side overlaps (a whole screen, such as first run, that scrolls vertically).
pub(crate) fn assert_screen_fits<S>(h: &Harness<'_, S>, size: egui::Vec2) {
    let window = window(size);
    for node in own_nodes(h.root()) {
        assert_within_sides(&node, window, size);
    }
    assert_no_overlap(h.root(), size);
}

/// Asserts `node` lies inside a window of `size`, and inside the dialog or menu it is in, and is
/// enabled, so a pointer can reach it.
pub(crate) fn assert_clickable<S>(h: &Harness<'_, S>, node: &Node<'_>, size: egui::Vec2) {
    let rect = node.rect();
    let accesskit = node.accesskit_node();
    let label = name(node);
    assert!(
        window(size).contains_rect(rect),
        "{size:?}: {label:?} at {rect:?} is not inside the window"
    );
    if let Some(frame) = overlay_of(h, node) {
        assert!(
            frame.expand(SLACK).contains_rect(rect),
            "{size:?}: {label:?} at {rect:?} is not inside its dialog at {frame:?}"
        );
    }
    assert!(!accesskit.is_disabled(), "{size:?}: {label:?} is disabled");
}

/// Every open dialog, menu and popup (each an area of its own; a popup sits in the tree under
/// the control that opened it), with the rect of its area: its frame.
fn overlays<'h, S>(h: &'h Harness<'_, S>) -> Vec<(Node<'h>, Rect)> {
    let areas = areas(h);
    h.root()
        .children_recursive()
        .filter(is_overlay)
        .filter_map(|node| {
            let (id, _) = node.accesskit_node().locate();
            let frame = areas.iter().find(|(area, _)| *area == id)?.1;
            Some((node, frame))
        })
        .collect()
}

fn is_overlay(node: &Node<'_>) -> bool {
    matches!(node.accesskit_node().role(), Role::Dialog | Role::Menu)
}

/// The nodes under `node` that must lie between its sides: all of them but the dialogs and menus
/// opened over it, and of a line that scrolls sideways only its scroll bar (what has scrolled
/// out of it is hidden).
fn own_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    let sideways = node.children().any(|child| {
        child.accesskit_node().role() == Role::ScrollBar
            && bounds(&child).is_some_and(|bar| bar.width() > bar.height())
    });
    let mut nodes = Vec::new();
    for child in node.children().filter(|child| !is_overlay(child)) {
        nodes.push(child);
        if !sideways {
            nodes.extend(own_nodes(child));
        }
    }
    nodes
}

/// What a node is called: a label's text, or another node's accessible name.
fn name(node: &Node<'_>) -> Option<String> {
    let accesskit = node.accesskit_node();
    if accesskit.role() == Role::Label {
        accesskit.value()
    } else {
        accesskit.label()
    }
}

/// The frame of the dialog or menu `node` is in, if any.
fn overlay_of<S>(h: &Harness<'_, S>, node: &Node<'_>) -> Option<Rect> {
    let areas = areas(h);
    let mut at = node.accesskit_node().parent();
    while let Some(parent) = at {
        if matches!(parent.role(), Role::Dialog | Role::Menu) {
            let (id, _) = parent.locate();
            return areas
                .iter()
                .find(|(area, _)| *area == id)
                .map(|(_, rect)| *rect);
        }
        at = parent.parent();
    }
    None
}

/// Every area egui keeps, by the accessibility id of its `Ui`, with its rect in points.
fn areas<S>(h: &Harness<'_, S>) -> Vec<(egui::accesskit::NodeId, Rect)> {
    h.ctx.memory(|m| {
        m.layer_ids()
            .filter_map(|layer| Some((layer.id.accesskit_id(), m.area_rect(layer.id)?)))
            .collect()
    })
}

/// The node's bounds in points, when it has any (a container has none).
fn bounds(node: &Node<'_>) -> Option<Rect> {
    node.accesskit_node().bounding_box()?;
    Some(node.rect()).filter(Rect::is_positive)
}

fn assert_within_sides(node: &Node<'_>, frame: Rect, size: egui::Vec2) {
    let Some(rect) = bounds(node) else {
        return;
    };
    let accesskit = node.accesskit_node();
    assert!(
        rect.left() >= frame.left() && rect.right() <= frame.right(),
        "{size:?}: {:?} {:?} at {rect:?} runs past the side of {frame:?}",
        accesskit.role(),
        name(node),
    );
}

/// Asserts that nothing under `root` is drawn over anything else. One inside the other is
/// allowed (a chip's remove button sits on its label), the dialogs and menus opened over it are
/// left out, and the content of a scroll area that scrolls is only compared with itself, since
/// what has scrolled under a header is hidden.
fn assert_no_overlap(root: Node<'_>, size: egui::Vec2) {
    let mut groups = vec![Vec::new()];
    clip_groups(root, 0, &mut groups);
    for placed in groups {
        for (index, (a, ra)) in placed.iter().enumerate() {
            for (b, rb) in &placed[index + 1..] {
                let nested = ra.contains_rect(*rb) || rb.contains_rect(*ra);
                assert!(
                    nested || !ra.intersects(*rb),
                    "{size:?}: {:?} {:?} at {ra:?} and {:?} {:?} at {rb:?} overlap",
                    a.accesskit_node().role(),
                    name(a),
                    b.accesskit_node().role(),
                    name(b),
                );
            }
        }
    }
}

/// Adds every node under `node` that has bounds to `groups[group]`, giving the content of each
/// scroll area that scrolls (it shows a scroll bar) a group of its own.
fn clip_groups<'t>(node: Node<'t>, group: usize, groups: &mut Vec<Vec<(Node<'t>, Rect)>>) {
    let scrolls = node
        .children()
        .any(|child| child.accesskit_node().role() == Role::ScrollBar);
    for child in node.children() {
        let role = child.accesskit_node().role();
        if is_overlay(&child) || matches!(role, Role::TextRun | Role::ScrollBar) {
            continue;
        }
        let group = if scrolls {
            groups.push(Vec::new());
            groups.len() - 1
        } else {
            group
        };
        if let Some(rect) = bounds(&child) {
            groups[group].push((child, rect.shrink(SLACK)));
        }
        clip_groups(child, group, groups);
    }
}
