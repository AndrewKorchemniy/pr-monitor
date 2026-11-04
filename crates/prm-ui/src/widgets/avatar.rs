//! A person's picture as a circle: their Azure DevOps avatar when one has loaded, otherwise
//! their initials on a colour chosen from their id. Reviewer and group chips reuse
//! `paint_avatar` and `face`.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, TextureId, vec2};
use prm_domain::{avatar::AvatarKey, view::ReviewerChip};

use crate::{
    avatars::AvatarStore,
    theme::{UiTheme, adapter::avatar_hue},
};

/// Initials take this share of the diameter as their font size.
const INITIALS_SCALE: f32 = 0.42;

/// Who to draw.
pub(crate) struct AvatarFace<'a> {
    pub key: &'a AvatarKey,
    pub initials: &'a str,
    /// Read out by screen readers.
    pub name: &'a str,
}

impl<'a> AvatarFace<'a> {
    /// A reviewer's face.
    pub(crate) fn of(reviewer: &'a ReviewerChip) -> Self {
        Self {
            key: &reviewer.avatar,
            initials: &reviewer.initials,
            name: &reviewer.name,
        }
    }
}

/// What a person's circle shows right now.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    Image(TextureId),
    Initials,
}

/// The picture if it has loaded, else the initials.
pub(crate) fn face(store: &AvatarStore, key: &AvatarKey) -> Face {
    store
        .texture(key)
        .map_or(Face::Initials, |texture| Face::Image(texture.id()))
}

/// A `diameter` square holding the avatar, labelled with the person's name.
pub(crate) fn avatar(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    store: &AvatarStore,
    who: &AvatarFace<'_>,
    diameter: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(diameter, diameter), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_avatar(ui, theme, store, who, rect);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, who.name));
    response
}

/// Draws the avatar into `rect` (which should be square) without allocating space.
pub(crate) fn paint_avatar(
    ui: &egui::Ui,
    theme: &UiTheme,
    store: &AvatarStore,
    who: &AvatarFace<'_>,
    rect: Rect,
) {
    match face(store, who.key) {
        Face::Image(id) => {
            egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).paint_at(ui, rect);
        }
        Face::Initials => paint_initials(ui, theme, who, rect),
    }
}

fn paint_initials(ui: &egui::Ui, theme: &UiTheme, who: &AvatarFace<'_>, rect: Rect) {
    let painter = ui.painter();
    let tint = theme.avatar_tints[avatar_hue(who.key.id)];
    let center: Pos2 = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    painter.circle_filled(center, radius, tint.bg);
    let font = FontId::new(
        radius * 2.0 * INITIALS_SCALE,
        theme.fonts.body_strong.family.clone(),
    );
    painter.text(center, Align2::CENTER_CENTER, who.initials, font, tint.text);
    let hairline = theme.tokens.metrics.stroke.hairline;
    painter.circle_stroke(
        center,
        radius - hairline / 2.0,
        Stroke::new(hairline, theme.colors.border_subtle),
    );
}

#[cfg(test)]
mod tests;
