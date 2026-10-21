//! Empty states: a headline, one line of context and the one action that fixes it.

use eframe::egui::{self, RichText};
use prm_domain::auth::SettingsSection;
use prm_service::RefreshPhase;

use crate::{
    actions::UiAction, commands::CommandId, copy, frame::FrameCx, route::ModalKind,
    view::empty::EmptyKind, widgets,
};

struct Content {
    headline: String,
    body: Option<String>,
    action: Option<(&'static str, UiAction)>,
    progress: Option<f32>,
}

fn content(kind: &EmptyKind, scope: &str) -> Content {
    let plain = |headline: &str| Content {
        headline: headline.to_owned(),
        body: None,
        action: None,
        progress: None,
    };
    match kind {
        EmptyKind::Loading {
            phase,
            done,
            total,
            org,
        } => Content {
            headline: copy::EMPTY_LOADING.to_owned(),
            body: (*total > 0 && *phase == Some(RefreshPhase::PullRequests))
                .then(|| copy::loading_body(*done, *total, org)),
            action: None,
            progress: (*total > 0).then(|| *done as f32 / *total as f32),
        },
        EmptyKind::SignedOut => Content {
            body: Some(copy::EMPTY_SIGNED_OUT_BODY.to_owned()),
            action: Some((copy::EMPTY_SIGN_IN, UiAction::Modal(ModalKind::AuthMenu))),
            ..plain(copy::EMPTY_SIGNED_OUT)
        },
        EmptyKind::NothingWatched => settings_fix(
            copy::EMPTY_NOTHING_WATCHED,
            copy::EMPTY_NOTHING_WATCHED_BODY.to_owned(),
            (copy::EMPTY_OPEN_ORGS, SettingsSection::Organizations),
        ),
        EmptyKind::NoActivePrs { everyone: true } => settings_fix(
            copy::EMPTY_NO_PRS,
            copy::no_prs_anywhere_body(scope),
            (copy::EMPTY_CHOOSE_REPOS, SettingsSection::Scope),
        ),
        EmptyKind::NoActivePrs { everyone: false } => settings_fix(
            copy::EMPTY_NO_PRS,
            copy::no_prs_body(scope),
            (copy::EMPTY_EDIT_PEOPLE, SettingsSection::People),
        ),
        EmptyKind::NoMatch { query } => Content {
            action: Some((copy::CLEAR_FILTER, UiAction::ClearFilters)),
            ..plain(&copy::no_match(query))
        },
        EmptyKind::StatusEmpty(status) => Content {
            action: Some((copy::SHOW_ALL, UiAction::Run(CommandId::StatusAll))),
            ..plain(copy::status_empty(*status))
        },
        EmptyKind::NothingChanged => Content {
            action: Some((copy::SHOW_ALL, UiAction::Run(CommandId::ChangedOnly))),
            ..plain(copy::NOTHING_CHANGED)
        },
    }
}

/// A state that a change in settings fixes: the action opens `section`.
fn settings_fix(
    headline: &str,
    body: String,
    (label, section): (&'static str, SettingsSection),
) -> Content {
    Content {
        headline: headline.to_owned(),
        body: Some(body),
        action: Some((label, UiAction::OpenSettings(section))),
        progress: None,
    }
}

/// Centered in the available space.
pub(crate) fn empty_state(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, kind: &EmptyKind, scope: &str) {
    let theme = cx.theme.clone();
    let content = content(kind, scope);
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() / 3.0).max(24.0));
        ui.label(
            RichText::new(&content.headline)
                .font(egui::FontId::new(18.0, theme.fonts.title.family.clone()))
                .color(theme.colors.text_primary),
        );
        ui.add_space(theme.tokens.metrics.space.md);
        if let Some(body) = &content.body {
            ui.label(RichText::new(body).color(theme.colors.text_secondary));
            ui.add_space(theme.tokens.metrics.space.lg);
        }
        if let Some(fraction) = content.progress {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(240.0, 4.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, theme.colors.control_bg);
            widgets::progress_line(ui, rect, fraction, theme.colors.accent_base);
        }
        if let Some((label, action)) = content.action
            && widgets::primary_button(ui, &theme, label).clicked()
        {
            cx.push(action);
        }
    });
}
