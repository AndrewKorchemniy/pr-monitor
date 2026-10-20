//! Count chips: an icon, a count and a label in a pill; clickable as a filter toggle.

use eframe::egui::{self, Sense, Stroke, StrokeKind, vec2};

use crate::theme::{Tone, UiTheme, icons::Icon};

pub(crate) struct Chip<'a> {
    pub icon: Option<Icon>,
    pub text: &'a str,
    pub tone: Tone,
    pub selected: bool,
}

const ICON: f32 = 10.0;
const PAD_X: f32 = 8.0;
const GAP: f32 = 5.0;

impl Chip<'_> {
    pub(crate) fn show(self, ui: &mut egui::Ui, theme: &UiTheme) -> egui::Response {
        let colors = theme.colors.tone(self.tone);
        let font = theme.fonts.label.clone();
        let text_color = if self.selected {
            colors.fg
        } else {
            theme.colors.text_secondary
        };
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_owned(), font, text_color);
        let icon_w = if self.icon.is_some() { ICON + GAP } else { 0.0 };
        let height = galley.size().y + 6.0;
        let size = vec2(PAD_X * 2.0 + icon_w + galley.size().x, height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        if ui.is_rect_visible(rect) {
            let radius = egui::CornerRadius::same(theme.tokens.metrics.radius.pill);
            let fill = if self.selected {
                colors.bg
            } else if response.hovered() {
                theme.colors.control_hover
            } else {
                theme.colors.control_bg
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            if self.selected {
                painter.rect_stroke(
                    rect,
                    radius,
                    Stroke::new(1.0, colors.solid),
                    StrokeKind::Inside,
                );
            }
            let mut x = rect.left() + PAD_X;
            if let Some(icon) = self.icon {
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(x + ICON / 2.0, rect.center().y),
                    vec2(ICON, ICON),
                );
                crate::theme::icons::paint(painter, icon_rect, icon, colors.solid);
                x += ICON + GAP;
            }
            let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, text_color);
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, self.selected, self.text)
        });
        response
    }
}
