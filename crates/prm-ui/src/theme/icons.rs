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
    Plus,
    /// Notifications.
    Bell,
    /// An organization: a building.
    Building,
    /// Appearance: a half-filled circle.
    Contrast,
    /// Merge conflicts or warnings: a triangle with a bar.
    Warning,
    /// A draft pull request: a pencil.
    Pencil,
    /// Auto-complete is set: a lightning bolt.
    Bolt,
    Info,
    /// A filled dot (status-bar auth indicator).
    Dot,
    /// Changed since last looked: a short rounded bar, like a changed row's edge.
    Bar,
    /// Grouping: a heading line over two indented rows.
    Group,
    /// Sorting: an arrow up beside an arrow down.
    Sort,
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
            // why: filled bars snapped to whole pixels keep their full colour; thin strokes
            // anti-aliased to a faint mark at small sizes.
            let bar = |x: f32| {
                let r = Rect::from_min_max(at(x - 0.17, -0.7), at(x + 0.17, 0.7));
                eframe::emath::GuiRounding::round_to_pixels(r, painter.pixels_per_point())
            };
            painter.rect_filled(bar(-0.38), 1.0, color);
            painter.rect_filled(bar(0.38), 1.0, color);
        }
        Icon::Cross => {
            painter.line_segment([at(-0.6, -0.6), at(0.6, 0.6)], s);
            painter.line_segment([at(0.6, -0.6), at(-0.6, 0.6)], s);
        }
        Icon::Search => {
            painter.circle_stroke(at(-0.2, -0.2), r * 0.55, s);
            painter.line_segment([at(0.2, 0.2), at(0.8, 0.8)], s);
        }
        Icon::Refresh => arc_with_head(painter, (c, r * 0.62), s, 0.0),
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
        Icon::Plus => {
            painter.line_segment([at(-0.7, 0.0), at(0.7, 0.0)], s);
            painter.line_segment([at(0.0, -0.7), at(0.0, 0.7)], s);
        }
        Icon::Bell => bell(painter, &at, s),
        Icon::Building => {
            let outline = vec![
                at(-0.55, 0.8),
                at(-0.55, -0.75),
                at(0.55, -0.75),
                at(0.55, 0.8),
            ];
            painter.add(Shape::closed_line(outline, s));
            for y in [-0.35, 0.05] {
                painter.line_segment([at(-0.25, y), at(0.25, y)], s);
            }
            painter.line_segment([at(-0.15, 0.8), at(-0.15, 0.45)], s);
            painter.line_segment([at(0.15, 0.45), at(0.15, 0.8)], s);
        }
        Icon::Contrast => {
            painter.circle_stroke(c, r * 0.8, s);
            let half: Vec<Pos2> = (0_u8..=12)
                .map(|i| {
                    let angle = TAU / 4.0 + TAU / 2.0 * f32::from(i) / 12.0;
                    c + vec2(angle.cos(), angle.sin()) * r * 0.8
                })
                .collect();
            painter.add(Shape::convex_polygon(half, color, Stroke::NONE));
        }
        Icon::Warning => {
            let triangle = vec![at(0.0, -0.85), at(0.9, 0.75), at(-0.9, 0.75)];
            painter.add(Shape::closed_line(triangle, s));
            painter.line_segment([at(0.0, -0.3), at(0.0, 0.25)], s);
            painter.circle_filled(at(0.0, 0.5), s.width * 0.6, color);
        }
        Icon::Pencil => pencil(painter, &at, s),
        Icon::Bolt => {
            // why: the bolt is concave, so it is two convex halves that overlap at the waist.
            let upper = vec![at(0.35, -0.95), at(0.15, 0.15), at(-0.6, 0.15)];
            let lower = vec![at(-0.15, -0.15), at(0.6, -0.15), at(-0.35, 0.95)];
            for half in [upper, lower] {
                painter.add(Shape::convex_polygon(half, color, Stroke::NONE));
            }
        }
        Icon::Info => {
            painter.circle_stroke(c, r * 0.85, s);
            painter.line_segment([at(0.0, -0.1), at(0.0, 0.5)], s);
            painter.circle_filled(at(0.0, -0.4), s.width * 0.6, color);
        }
        Icon::Bar => {
            // why: wide and rounded enough that it never reads as a hairline divider.
            let size = vec2(r * 0.6, r * 1.6);
            painter.rect_filled(Rect::from_center_size(c, size), size.x / 2.0, color);
        }
        Icon::Group => {
            painter.line_segment([at(-0.75, -0.6), at(0.75, -0.6)], s);
            for y in [0.0, 0.6] {
                painter.line_segment([at(-0.3, y), at(0.75, y)], s);
            }
        }
        Icon::Sort => {
            for (x, dir) in [(-0.4, -1.0), (0.4, 1.0)] {
                let tip = at(x, 0.75 * dir);
                painter.line_segment([at(x, -0.75 * dir), tip], s);
                painter.line_segment([tip, at(x - 0.3, 0.4 * dir)], s);
                painter.line_segment([tip, at(x + 0.3, 0.4 * dir)], s);
            }
        }
    }
}

fn bell(painter: &Painter, at: &impl Fn(f32, f32) -> Pos2, s: Stroke) {
    let outline = vec![
        at(-0.65, 0.45),
        at(-0.5, 0.3),
        at(-0.5, -0.1),
        at(-0.4, -0.45),
        at(-0.2, -0.66),
        at(0.0, -0.72),
        at(0.2, -0.66),
        at(0.4, -0.45),
        at(0.5, -0.1),
        at(0.5, 0.3),
        at(0.65, 0.45),
    ];
    painter.add(Shape::closed_line(outline, s));
    painter.line_segment([at(-0.15, 0.68), at(0.15, 0.68)], s);
}

/// A pencil at 45°, tip at the bottom left, with the band below its end.
fn pencil(painter: &Painter, at: &impl Fn(f32, f32) -> Pos2, s: Stroke) {
    let outline = vec![
        at(-0.8, 0.8),
        at(-0.27, 0.63),
        at(0.88, -0.52),
        at(0.52, -0.88),
        at(-0.63, 0.27),
    ];
    painter.add(Shape::closed_line(outline, s));
    painter.line_segment([at(0.17, -0.53), at(0.53, -0.17)], s);
}

fn check(painter: &Painter, at: &impl Fn(f32, f32) -> Pos2, s: Stroke) {
    painter.line_segment([at(-0.7, 0.05), at(-0.2, 0.55)], s);
    painter.line_segment([at(-0.2, 0.55), at(0.75, -0.55)], s);
}

/// A clockwise arc open at the top right, ending at the top in a solid arrowhead.
/// The refresh arrow turned clockwise by `turn` radians, for a refresh in progress.
pub(crate) fn paint_refresh_turned(painter: &Painter, rect: Rect, color: Color32, turn: f32) {
    let r = rect.width().min(rect.height()) / 2.0;
    arc_with_head(
        painter,
        (rect.center(), r * 0.62),
        stroke(color, rect),
        turn,
    );
}

fn arc_with_head(painter: &Painter, (center, radius): (Pos2, f32), s: Stroke, turn: f32) {
    const STEPS: u8 = 24;
    // Screen angles grow clockwise (y points down): start just above the right, end at the top.
    let (start, sweep) = (turn - TAU / 12.0, TAU * 5.0 / 6.0 - TAU / 12.0);
    let point = |angle: f32| center + vec2(angle.cos(), angle.sin()) * radius;
    let points: Vec<Pos2> = (0..=STEPS)
        .map(|i| point(start + sweep * f32::from(i) / f32::from(STEPS)))
        .collect();
    painter.add(Shape::line(points, s));
    let end_angle = start + sweep;
    let end = point(end_angle);
    let along = vec2(-end_angle.sin(), end_angle.cos());
    let out = vec2(end_angle.cos(), end_angle.sin());
    let (length, half) = (radius * 0.55, radius * 0.42);
    let head = vec![end + along * length, end + out * half, end - out * half];
    painter.add(Shape::convex_polygon(head, s.color, Stroke::NONE));
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
        | Icon::Plus
        | Icon::Bell
        | Icon::Building
        | Icon::Contrast
        | Icon::Warning
        | Icon::Pencil
        | Icon::Bolt
        | Icon::Info
        | Icon::Dot
        | Icon::Bar
        | Icon::Group
        | Icon::Sort => paint(painter, rect, icon, stroke.color),
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
