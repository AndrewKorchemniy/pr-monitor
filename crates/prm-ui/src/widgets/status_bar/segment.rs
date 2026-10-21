//! One run of text in the status bar: an optional glyph, then a label, padded on both sides so
//! hover fills have room. Each kind's look and accessibility come from one profile.

use std::{borrow::Cow, sync::Arc};

use eframe::egui::{self, Color32, Rect, Sense, WidgetInfo, pos2, vec2};

use crate::{
    theme::{
        Tone, UiTheme,
        icons::{self, Icon},
    },
    widgets::controls,
};

/// The active filter's underline, in points.
const UNDERLINE: f32 = 2.0;

/// A painted glyph before a segment's text.
#[derive(Copy, Clone)]
pub(super) struct Glyph {
    pub icon: Icon,
    pub size: f32,
    pub color: Color32,
}

/// What a segment is to a screen reader and how it answers the pointer.
#[derive(Copy, Clone)]
pub(super) enum Kind {
    /// Plain information.
    Info,
    /// A button that opens something.
    Link,
    /// A filter toggle; while `on`, its label and underline take the tone's foreground.
    Toggle { tone: Tone, on: bool },
}

/// How one kind of segment behaves.
#[derive(Copy, Clone)]
struct Profile {
    sense: Sense,
    role: egui::Role,
    /// The toggled state a screen reader announces; `None` for segments that are not toggles.
    selected: Option<bool>,
    /// Hover gives a ghost fill and brighter text.
    interactive: bool,
    /// The tone an active filter's label and underline take.
    accent: Option<Tone>,
}

impl Kind {
    fn profile(self) -> Profile {
        match self {
            Self::Info => Profile {
                sense: Sense::hover(),
                role: egui::Role::Label,
                selected: None,
                interactive: false,
                accent: None,
            },
            Self::Link => Profile {
                sense: Sense::click(),
                role: egui::Role::Button,
                selected: None,
                interactive: true,
                accent: None,
            },
            Self::Toggle { tone, on } => Profile {
                sense: Sense::click(),
                role: egui::Role::Button,
                selected: Some(on),
                interactive: true,
                accent: on.then_some(tone),
            },
        }
    }
}

/// A segment with its text laid out in the bar's font.
pub(super) struct Segment<'a> {
    glyph: Option<Glyph>,
    /// The accessible name, and the visible text unless collapsed.
    text: Cow<'a, str>,
    kind: Kind,
    /// Coloured when painted, after hit testing.
    galley: Arc<egui::Galley>,
    /// Only the glyph shows; the text stays the accessible name.
    pub collapsed: bool,
}

impl<'a> Segment<'a> {
    pub(super) fn new(
        ui: &egui::Ui,
        theme: &UiTheme,
        glyph: Option<Glyph>,
        text: Cow<'a, str>,
        kind: Kind,
    ) -> Self {
        let galley = ui.painter().layout_no_wrap(
            text.clone().into_owned(),
            theme.fonts.small.clone(),
            Color32::PLACEHOLDER,
        );
        Self {
            glyph,
            text,
            kind,
            galley,
            collapsed: false,
        }
    }

    /// Starts with only the glyph showing.
    pub(super) const fn with_collapsed(mut self) -> Self {
        self.collapsed = true;
        self
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// Padding, the glyph, and the text unless collapsed, in points.
    pub(super) fn width(&self, theme: &UiTheme) -> f32 {
        let pad = theme.tokens.metrics.space.sm;
        let glyph = self.glyph.map_or(0.0, |g| g.size + pad);
        let text = if self.collapsed {
            0.0
        } else {
            self.text_width(theme)
        };
        pad + glyph + text
    }

    /// The points the text and its padding add to the glyph.
    pub(super) fn text_width(&self, theme: &UiTheme) -> f32 {
        self.galley.size().x + theme.tokens.metrics.space.sm
    }

    pub(super) fn show(self, ui: &mut egui::Ui, theme: &UiTheme, height: f32) -> egui::Response {
        let profile = self.kind.profile();
        let size = vec2(self.width(theme), height);
        let (rect, response) = ui.allocate_exact_size(size, profile.sense);
        if ui.is_rect_visible(rect) {
            self.paint(ui, theme, (rect, &response), profile);
        }
        let (enabled, text) = (ui.is_enabled(), self.text);
        response.widget_info(|| match profile.selected {
            Some(on) => WidgetInfo::selected(profile.role, enabled, on, &text),
            None => WidgetInfo::labeled(profile.role, enabled, &text),
        });
        response
    }

    fn paint(
        &self,
        ui: &egui::Ui,
        theme: &UiTheme,
        (rect, response): (Rect, &egui::Response),
        profile: Profile,
    ) {
        let pad = theme.tokens.metrics.space.sm;
        if profile.interactive {
            controls::ghost_fill(ui, theme, rect, response);
        }
        let painter = ui.painter();
        let mut x = rect.left() + pad;
        if let Some(glyph) = self.glyph {
            let center = pos2(x + glyph.size / 2.0, rect.center().y);
            let square = Rect::from_center_size(center, vec2(glyph.size, glyph.size));
            icons::paint(painter, square, glyph.icon, glyph.color);
            x += glyph.size + pad;
        }
        if self.collapsed {
            return;
        }
        let top = rect.center().y - self.galley.size().y / 2.0;
        let text = Rect::from_min_size(pos2(x, top), self.galley.size());
        let color = text_color(theme, response, profile);
        painter.galley(text.min, self.galley.clone(), color);
        if let Some(tone) = profile.accent {
            let underline = Rect::from_min_max(
                pos2(rect.left() + pad, text.bottom()),
                pos2(text.right(), text.bottom() + UNDERLINE),
            );
            painter.rect_filled(underline, 0.0, theme.colors.tone(tone).fg);
        }
    }
}

fn text_color(theme: &UiTheme, response: &egui::Response, profile: Profile) -> Color32 {
    let colors = &theme.colors;
    match profile.accent {
        Some(tone) => colors.tone(tone).fg,
        None if profile.interactive && response.hovered() => colors.text_primary,
        None => colors.text_secondary,
    }
}
