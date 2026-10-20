//! Vector icons painted with strokes and fills (no icon font, so no missing glyphs).

use std::f32::consts::TAU;

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    /// Needs review: a hollow circle.
    Ring,
    /// Active comments: a filled diamond.
    Diamond,
    /// Approved: a check.
    Check,
    /// Approved with suggestions: a check with a dot.
    CheckDot,
    /// Waiting for author: two bars.
    Pause,
    /// Rejected or close: a cross.
    Cross,
    Search,
    Refresh,
    Gear,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    /// Continue or submit.
    ArrowRight,
    /// Back.
    ArrowLeft,
    Plus,
    /// Merge conflicts or warnings: a triangle with a bar.
    Warning,
    Info,
    /// A filled dot (status-bar auth indicator).
    Dot,
    /// The change bar mark.
    Bar,
}

fn stroke(color: Color32, rect: Rect) -> Stroke {
    Stroke::new((rect.height() / 8.0).clamp(1.0, 2.0), color)
}

/// Paints the icon to fill `rect` (square works best).
#[expect(
    clippy::too_many_lines,
    reason = "one short arm per icon reads best side by side"
)]
pub(crate) fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    let s = stroke(color, rect);
    let at = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    match icon {
        Icon::Ring => {
            painter.circle_stroke(c, r * 0.8, s);
        }
        Icon::Dot => {
            painter.circle_filled(c, r * 0.6, color);
        }
        Icon::Diamond => {
            let points = vec![at(0.0, -0.9), at(0.9, 0.0), at(0.0, 0.9), at(-0.9, 0.0)];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Check => check(painter, &at, s),
        Icon::CheckDot => {
            check(painter, &at, s);
            painter.circle_filled(at(0.75, 0.7), r * 0.15, color);
        }
        Icon::Pause => {
            painter.line_segment([at(-0.35, -0.7), at(-0.35, 0.7)], s);
            painter.line_segment([at(0.35, -0.7), at(0.35, 0.7)], s);
        }
        Icon::Cross => {
            painter.line_segment([at(-0.6, -0.6), at(0.6, 0.6)], s);
            painter.line_segment([at(0.6, -0.6), at(-0.6, 0.6)], s);
        }
        Icon::Search => {
            painter.circle_stroke(at(-0.2, -0.2), r * 0.55, s);
            painter.line_segment([at(0.2, 0.2), at(0.8, 0.8)], s);
        }
        Icon::Refresh => arc_with_head(painter, c, r * 0.7, s),
        Icon::Gear => {
            painter.circle_stroke(c, r * 0.45, s);
            for i in 0..8 {
                let angle = TAU * f32::from(u8::try_from(i).unwrap_or(0)) / 8.0;
                let dir = vec2(angle.cos(), angle.sin());
                painter.line_segment([c + dir * r * 0.6, c + dir * r * 0.9], s);
            }
        }
        Icon::ChevronRight => {
            painter.line_segment([at(-0.3, -0.6), at(0.3, 0.0)], s);
            painter.line_segment([at(0.3, 0.0), at(-0.3, 0.6)], s);
        }
        Icon::ChevronDown => {
            painter.line_segment([at(-0.6, -0.3), at(0.0, 0.3)], s);
            painter.line_segment([at(0.0, 0.3), at(0.6, -0.3)], s);
        }
        Icon::ChevronUp => {
            painter.line_segment([at(-0.6, 0.3), at(0.0, -0.3)], s);
            painter.line_segment([at(0.0, -0.3), at(0.6, 0.3)], s);
        }
        Icon::ArrowRight => {
            painter.line_segment([at(-0.7, 0.0), at(0.7, 0.0)], s);
            painter.line_segment([at(0.15, -0.55), at(0.7, 0.0)], s);
            painter.line_segment([at(0.7, 0.0), at(0.15, 0.55)], s);
        }
        Icon::ArrowLeft => {
            painter.line_segment([at(0.7, 0.0), at(-0.7, 0.0)], s);
            painter.line_segment([at(-0.15, -0.55), at(-0.7, 0.0)], s);
            painter.line_segment([at(-0.7, 0.0), at(-0.15, 0.55)], s);
        }
        Icon::Plus => {
            painter.line_segment([at(-0.7, 0.0), at(0.7, 0.0)], s);
            painter.line_segment([at(0.0, -0.7), at(0.0, 0.7)], s);
        }
        Icon::Warning => {
            let triangle = vec![at(0.0, -0.85), at(0.9, 0.75), at(-0.9, 0.75)];
            painter.add(Shape::closed_line(triangle, s));
            painter.line_segment([at(0.0, -0.3), at(0.0, 0.25)], s);
            painter.circle_filled(at(0.0, 0.5), s.width * 0.6, color);
        }
        Icon::Info => {
            painter.circle_stroke(c, r * 0.85, s);
            painter.line_segment([at(0.0, -0.1), at(0.0, 0.5)], s);
            painter.circle_filled(at(0.0, -0.4), s.width * 0.6, color);
        }
        Icon::Bar => {
            painter.rect_filled(
                Rect::from_center_size(c, vec2(r * 0.35, r * 1.8)),
                0.0,
                color,
            );
        }
    }
}

fn check(painter: &Painter, at: &impl Fn(f32, f32) -> Pos2, s: Stroke) {
    painter.line_segment([at(-0.7, 0.05), at(-0.2, 0.55)], s);
    painter.line_segment([at(-0.2, 0.55), at(0.75, -0.55)], s);
}

fn arc_with_head(painter: &Painter, center: Pos2, radius: f32, s: Stroke) {
    const STEPS: u8 = 16;
    let point = |t: f32| {
        let angle = -TAU * 0.1 + t * TAU * 0.8;
        center + vec2(angle.cos(), angle.sin()) * radius
    };
    let points: Vec<Pos2> = (0..=STEPS)
        .map(|i| point(f32::from(i) / f32::from(STEPS)))
        .collect();
    painter.add(Shape::line(points, s));
    let end = point(0.0);
    painter.line_segment([end, end + vec2(-radius * 0.45, -radius * 0.1)], s);
    painter.line_segment([end, end + vec2(radius * 0.05, -radius * 0.45)], s);
}

/// Paints a stroked icon (check, cross, chevrons, arrows) with an explicit stroke width.
pub(crate) fn paint_stroke(painter: &Painter, rect: Rect, icon: Icon, stroke: Stroke) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) / 2.0;
    let at = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    match icon {
        Icon::Check => check(painter, &at, stroke),
        Icon::Ring
        | Icon::Diamond
        | Icon::CheckDot
        | Icon::Pause
        | Icon::Cross
        | Icon::Search
        | Icon::Refresh
        | Icon::Gear
        | Icon::ChevronRight
        | Icon::ChevronDown
        | Icon::ChevronUp
        | Icon::ArrowRight
        | Icon::ArrowLeft
        | Icon::Plus
        | Icon::Warning
        | Icon::Info
        | Icon::Dot
        | Icon::Bar => paint(painter, rect, icon, stroke.color),
    }
}

/// Allocates a square and paints an icon in it (for inline use in layouts).
pub(crate) fn show(
    ui: &mut eframe::egui::Ui,
    icon: Icon,
    size: f32,
    color: Color32,
) -> eframe::egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), eframe::egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, icon, color);
    }
    response
}
