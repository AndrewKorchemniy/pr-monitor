//! The dialogs over the Organizations page: adding an org, choosing its repositories, editing
//! its people lists, and the third-party licenses. Each fits the window and scrolls inside.

use eframe::egui::{self, RichText};
use prm_service::Command;

use super::{add_org, list_copy, people_list, saved};
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
        nested: true,
    };
    // why: the dialog's body scrolls, so the long text needs no scroll area of its own.
    modal::modal(ctx, theme, &spec, |ui| {
        for text in [
            env.licenses,
            crate::theme::fonts::OFL_NEXT,
            crate::theme::fonts::OFL_MONO,
        ] {
            license(ui, theme, text);
            controls::row_divider(ui, theme);
        }
    })
}

/// One license text, re-flowed to the dialog's width. The texts are fixed, so each is re-flowed
/// once and kept, not on every frame the dialog is open.
fn license(ui: &mut egui::Ui, theme: &UiTheme, text: &'static str) {
    let id = egui::Id::unique(("license", text.as_ptr().addr()));
    let blocks = ui.data_mut(|d| {
        d.get_temp_mut_or_insert_with::<std::sync::Arc<[Block]>>(id, || reflow(text).into())
            .clone()
    });
    for block in blocks.iter() {
        match block {
            Block::Text(text) => {
                ui.label(
                    RichText::new(text.as_str())
                        .font(theme.fonts.mono.clone())
                        .color(theme.colors.text_secondary),
                );
            }
            Block::Rule => controls::row_divider(ui, theme),
        }
    }
}

/// A piece of a license text laid out for the dialog's width.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Block {
    /// A paragraph, its hard-wrapped lines joined so the dialog wraps it; lines that start a
    /// list item or are indented keep their own line.
    Text(String),
    /// A line of only dashes or equals signs.
    Rule,
}

/// Splits `text` into paragraphs and rules. License files are hard-wrapped near 76 columns,
/// which re-wrapped at another width left ragged short lines.
fn reflow(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para = String::new();
    let flush = |para: &mut String, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Text(std::mem::take(para)));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut para, &mut blocks);
        } else if is_rule(trimmed) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Rule);
        } else {
            let keep = keeps_line(line);
            let separator = if para.is_empty() {
                ""
            } else if keep {
                "
"
            } else {
                " "
            };
            para.push_str(separator);
            para.push_str(if keep { line.trim_end() } else { trimmed });
        }
    }
    flush(&mut para, &mut blocks);
    blocks
}

fn is_rule(line: &str) -> bool {
    line.len() >= 3 && (line.chars().all(|c| c == '-') || line.chars().all(|c| c == '='))
}

/// A line that starts its own line: indented, or a list item ("- ", "* ", "1)", "1.").
fn keeps_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let numbered = trimmed
        .split_once([')', '.'])
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    line.starts_with(char::is_whitespace)
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || numbered
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
        nested: true,
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
        nested: true,
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
    let picker = match editor {
        Editor::Reviewers => &mut st.pickers.reviewers,
        Editor::Ignored => &mut st.pickers.ignored,
        Editor::Tracked | Editor::AddOrg | Editor::Licenses | Editor::Repositories => {
            &mut st.pickers.tracked
        }
    };
    let (title, help) = list_copy(editor);
    let org_label = org.display_label().to_string();
    let spec = ModalSpec {
        id: "edit_people",
        title,
        subtitle: Some(&org_label),
        buttons: &[copy::DONE],
        danger: false,
        width: 480.0,
        nested: true,
    };
    let list = people_list(org, editor);
    let outcome = modal::modal(ctx, &theme, &spec, |ui| {
        // why: the dialog covers the section's help, and emptying the tracked list here
        // changes what it means.
        widgets::muted(ui, &theme, help);
        ui.add_space(theme.tokens.metrics.space.sm);
        people_picker(ui, cx, picker, id.as_ref(), list);
    });
    if outcome != ModalOutcome::Open {
        picker.reset();
    }
    outcome
}

#[cfg(test)]
mod tests;
