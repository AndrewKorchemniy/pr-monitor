//! The dialogs over the Organizations page: adding an org, choosing its repositories, editing
//! its people lists, and the third-party licenses. Each fits the window and scrolls inside.

use eframe::egui::{self, RichText};
use prm_service::Command;

use super::{add_org, people_list, saved};
use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::{Editor, SettingsEnv, SettingsState},
    theme::{UiTheme, icons::Icon},
    widgets::{
        self, controls,
        fields::{self, Field},
        modal::{self, ModalOutcome, ModalSpec},
        people_picker::people_picker,
        scope_picker::{CatalogState, scope_picker},
    },
};

/// The open dialog over the page, if any: add an org, choose repositories, or edit people.
pub(crate) fn editor(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let Some(editor) = st.editing else {
        return;
    };
    let outcome = match editor {
        Editor::AddOrg => add_org_dialog(ctx, cx, st),
        Editor::Licenses => licenses_dialog(ctx, &cx.theme.clone(), env),
        Editor::Repositories => repositories_dialog(ctx, cx, st),
        Editor::Tracked | Editor::Reviewers | Editor::Ignored => people_dialog(ctx, cx, st, editor),
    };
    if outcome != ModalOutcome::Open {
        st.editing = None;
    }
}

/// Third-party licenses and the bundled fonts' licenses, in a scrolling dialog.
fn licenses_dialog(ctx: &egui::Context, theme: &UiTheme, env: &SettingsEnv<'_>) -> ModalOutcome {
    let spec = ModalSpec {
        id: "licenses",
        title: copy::LICENSES,
        subtitle: None,
        buttons: &[copy::CLOSE],
        danger: false,
        width: 640.0,
    };
    // why: the dialog's body scrolls, so the long text needs no scroll area of its own.
    modal::modal(ctx, theme, &spec, |ui| {
        for text in [
            env.licenses,
            crate::theme::fonts::OFL_NEXT,
            crate::theme::fonts::OFL_MONO,
        ] {
            ui.label(
                RichText::new(text)
                    .font(theme.fonts.mono.clone())
                    .color(theme.colors.text_secondary),
            );
            controls::row_divider(ui, theme);
        }
    })
}

fn add_org_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
) -> ModalOutcome {
    let theme = &cx.theme.clone();
    let spec = ModalSpec {
        id: "add_org",
        title: copy::ADD_ORG,
        subtitle: None,
        buttons: &[],
        danger: false,
        width: modal::WIDTH,
    };
    let mut added = false;
    let outcome = modal::modal(ctx, theme, &spec, |ui| {
        widgets::field_label(ui, theme, copy::ORG_URL_LABEL);
        ui.add_space(theme.tokens.metrics.space.xs);
        let mut field = Field::new(egui::Id::unique("add_org_url"), copy::ORG_URL_HINT);
        field.error = st.url_error;
        let action = (&field, copy::ADD_ORG, Icon::ArrowRight);
        if fields::action_field(ui, theme, &mut st.new_org_url, action)
            && !st.new_org_url.trim().is_empty()
        {
            added = add_org(st, cx);
        }
        if st.url_error {
            widgets::error_text(ui, theme, copy::BAD_ORG_URL);
        }
    });
    if added {
        ModalOutcome::Button(0)
    } else {
        outcome
    }
}

fn repositories_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
) -> ModalOutcome {
    let theme = cx.theme.clone();
    let org_label = st
        .draft
        .orgs
        .get(st.org)
        .map(|o| o.display_label().to_string());
    let spec = ModalSpec {
        id: "choose_repositories",
        title: copy::STEP_REPOS,
        subtitle: org_label.as_deref(),
        buttons: &[copy::DONE],
        danger: false,
        width: 560.0,
    };
    let Some(org) = st.draft.orgs.get_mut(st.org) else {
        return ModalOutcome::Dismissed;
    };
    let id = org.org_id().cloned();
    let state = cx.state;
    let catalog = CatalogState::of(id.as_ref().and_then(|id| state.org(id)));
    let filter = &mut st.scope_filter;
    let mut reload = false;
    let outcome = modal::modal(ctx, &theme, &spec, |ui| {
        reload = scope_picker(ui, &theme, filter, catalog, &mut org.scope);
    });
    if reload && let Some(id) = id {
        cx.push(UiAction::Send(Command::LoadCatalog {
            org: id,
            force: true,
        }));
    }
    outcome
}

fn people_dialog(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    editor: Editor,
) -> ModalOutcome {
    let theme = cx.theme.clone();
    let saved = saved(st);
    let Some(org) = st.draft.orgs.get_mut(st.org) else {
        return ModalOutcome::Dismissed;
    };
    let id = saved.then(|| org.org_id().cloned()).flatten();
    let (title, picker) = match editor {
        Editor::Reviewers => (copy::REVIEWERS_COUNT, &mut st.pickers.reviewers),
        Editor::Ignored => (copy::IGNORE_FROM, &mut st.pickers.ignored),
        Editor::Tracked | Editor::AddOrg | Editor::Licenses | Editor::Repositories => {
            (copy::TRACK_FROM, &mut st.pickers.tracked)
        }
    };
    let org_label = org.display_label().to_string();
    let spec = ModalSpec {
        id: "edit_people",
        title,
        subtitle: Some(&org_label),
        buttons: &[copy::DONE],
        danger: false,
        width: 480.0,
    };
    let list = people_list(org, editor);
    let outcome = modal::modal(ctx, &theme, &spec, |ui| {
        people_picker(ui, cx, picker, id.as_ref(), list);
    });
    if outcome != ModalOutcome::Open {
        picker.reset();
    }
    outcome
}
