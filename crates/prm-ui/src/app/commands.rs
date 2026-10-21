//! Running a `CommandId` (from keys, the palette, buttons, the context menu or the tray).

use eframe::egui;
use prm_domain::{
    auth::SettingsSection,
    view::{GroupBy, SortKey, StatusFilter},
};
use prm_service::Command;

use super::PrMonitorApp;
use crate::{
    actions::UiAction,
    commands::{CommandId as C, PAUSE},
    copy,
    format::markdown_link,
    route::{ModalKind, Overlay},
    widgets::toolbar::{GROUPS, filter_id},
};

const ZOOM_STEP: f32 = 0.1;

/// Commands on the selected PR.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PrCommand {
    Open,
    Peek,
    CopyLink,
    CopyMarkdown,
    CopyTitle,
    FilterAuthor,
    FilterRepo,
    MarkSeen,
}

impl PrMonitorApp {
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per command keeps the registry exhaustive"
    )]
    pub(super) fn run_command(&mut self, ctx: &egui::Context, command: C) {
        self.palette.recent.retain(|c| *c != command);
        self.palette.recent.insert(0, command);
        self.palette.recent.truncate(8);
        let now = self.clock.now();
        let view = &mut self.dash.view;
        match command {
            C::SortStatus => view.sort_by(SortKey::Status),
            C::SortNumber => view.sort_by(SortKey::Number),
            C::SortTitle => view.sort_by(SortKey::Title),
            C::SortRepo => view.sort_by(SortKey::Repo),
            C::SortAuthor => view.sort_by(SortKey::Author),
            C::SortAge => view.sort_by(SortKey::Age),
            C::SortActivity => view.sort_by(SortKey::Activity),
            C::SortComments => view.sort_by(SortKey::Comments),
            C::GroupNone => view.set_group(GroupBy::None),
            C::GroupStatus => view.set_group(GroupBy::Status),
            C::GroupAuthor => view.set_group(GroupBy::Author),
            C::GroupRepo => view.set_group(GroupBy::Repo),
            C::GroupOrg => view.set_group(GroupBy::Org),
            C::GroupProject => view.set_group(GroupBy::Project),
            C::GroupCycle => {
                let at = GROUPS
                    .iter()
                    .position(|(g, _, _)| *g == view.group)
                    .unwrap_or(0);
                view.set_group(GROUPS[(at + 1) % GROUPS.len()].0);
            }
            C::StatusNeedsReview => view.set_status(StatusFilter::NeedsReview),
            C::StatusActiveComments => view.set_status(StatusFilter::ActiveComments),
            C::StatusReviewed => view.set_status(StatusFilter::Reviewed),
            C::StatusAll => {
                if view.query.status != StatusFilter::All {
                    view.set_status(view.query.status);
                }
            }
            C::ChangedOnly => view.toggle_unread_only(),
            C::HideDrafts => view.toggle_hide_drafts(),
            C::NavDown => view.nav(1),
            C::NavUp => view.nav(-1),
            C::NavFirst => view.nav_edge(false),
            C::NavLast => view.nav_edge(true),
            C::NavPageDown => view.nav_page(true),
            C::NavPageUp => view.nav_page(false),
            C::NavNextGroup => view.nav_group(true),
            C::NavPrevGroup => view.nav_group(false),
            C::GroupCollapse => view.set_selected_group_collapsed(true),
            C::GroupExpand => view.set_selected_group_collapsed(false),
            C::ColumnsReset => view.reset_columns(),
            C::FilterFocus => ctx.memory_mut(|m| m.request_focus(filter_id())),
            C::PrOpen => self.run_pr(ctx, PrCommand::Open),
            C::PrPeek => self.run_pr(ctx, PrCommand::Peek),
            C::PrCopyLink => self.run_pr(ctx, PrCommand::CopyLink),
            C::PrCopyMarkdown => self.run_pr(ctx, PrCommand::CopyMarkdown),
            C::PrCopyTitle => self.run_pr(ctx, PrCommand::CopyTitle),
            C::PrFilterAuthor => self.run_pr(ctx, PrCommand::FilterAuthor),
            C::PrFilterRepo => self.run_pr(ctx, PrCommand::FilterRepo),
            C::PrMarkSeen => self.run_pr(ctx, PrCommand::MarkSeen),
            C::PrMarkAllSeen => {
                self.notices
                    .info(&copy::marked_seen(self.state.summary.unread as usize), now);
                self.link.send(Command::MarkAllSeen);
            }
            C::PaletteOpen => {
                self.palette.query.clear();
                self.palette.selected = 0;
                self.overlay = Overlay::Palette;
            }
            C::RefreshNow => self.link.send(Command::RefreshNow),
            C::SettingsOpen => {
                self.open_settings(self.state.ui_state.settings_section.unwrap_or_default());
            }
            C::AboutOpen => self.open_settings(SettingsSection::About),
            C::SettingsSave => self.actions.push(UiAction::SaveSettings),
            C::ShortcutsShow => self.overlay = Overlay::Shortcuts,
            C::NotificationsPause => {
                let until = Some(now.saturating_add(PAUSE));
                self.link.send(Command::SetNotificationsPaused { until });
            }
            C::NotificationsResume => self
                .link
                .send(Command::SetNotificationsPaused { until: None }),
            C::NotificationsTest => self.link.send(Command::TestNotification),
            C::AuthMenu => {
                self.window.show(ctx);
                self.overlay = Overlay::Modal(ModalKind::AuthMenu);
            }
            C::WindowShow => self.window.show(ctx),
            C::WindowHide => self.hide(ctx),
            C::AppQuit => self.quit(ctx),
            C::ZoomIn => self.zoom(Some(ZOOM_STEP)),
            C::ZoomOut => self.zoom(Some(-ZOOM_STEP)),
            C::ZoomReset => self.zoom(None),
            C::ThemeCycle => self.cycle_theme(),
        }
    }

    fn run_pr(&mut self, ctx: &egui::Context, command: PrCommand) {
        let Some(row) = self.dash.view.selected_row().cloned() else {
            return;
        };
        let now = self.clock.now();
        let first_word = |s: &str| s.split_whitespace().next().unwrap_or_default().to_owned();
        let mut copy_text = |text: String, notice: &str| {
            ctx.copy_text(text);
            self.notices.info(notice, now);
        };
        match command {
            PrCommand::CopyLink => copy_text(row.web_url.to_string(), copy::LINK_COPIED),
            PrCommand::CopyMarkdown => {
                let title = format!("{} {}", row.number_text, row.title);
                copy_text(markdown_link(&title, &row.web_url), copy::MARKDOWN_COPIED);
            }
            PrCommand::CopyTitle => copy_text(row.title.to_string(), copy::TITLE_COPIED),
            PrCommand::Open => self.open_pr(row.key.clone()),
            PrCommand::Peek => self.dash.peek.open = !self.dash.peek.open,
            PrCommand::FilterAuthor => {
                self.set_filter_text(format!("@{}", first_word(&row.author_key)));
            }
            PrCommand::FilterRepo => {
                self.set_filter_text(format!("repo:{}", first_word(&row.repo_key)));
            }
            PrCommand::MarkSeen => {
                self.changes.forget(&row.key);
                self.link.send(Command::MarkSeen(vec![row.key.clone()]));
            }
        }
    }

    /// Zoom is the text-size preference, applied through the service like any setting.
    fn zoom(&self, step: Option<f32>) {
        let mut config = (*self.state.config).clone();
        let range = prm_config::limits::FONT_SCALE;
        let scale = step.map_or(prm_config::limits::DEFAULT_FONT_SCALE, |s| {
            config.ui.font_scale + s
        });
        config.ui.font_scale = ((scale * 10.0).round() / 10.0).clamp(*range.start(), *range.end());
        self.link.send(Command::ApplyConfig(Box::new(config)));
    }

    fn cycle_theme(&self) {
        let choices = self.theme.choices();
        let mut config = (*self.state.config).clone();
        let next = choices
            .iter()
            .position(|(id, _)| *id == config.ui.theme)
            .map_or(0, |i| (i + 1) % choices.len());
        if let Some((id, _)) = choices.get(next) {
            config.ui.theme.clone_from(id);
            self.link.send(Command::ApplyConfig(Box::new(config)));
        }
    }

    pub(super) fn tray_menu(&mut self, ctx: &egui::Context, key: &str) {
        let Some(command) = C::from_key(key) else {
            return;
        };
        let paused = self.tray.as_ref().is_some_and(|(tray, _)| tray.paused());
        let command = if command == C::NotificationsPause && paused {
            C::NotificationsResume
        } else {
            command
        };
        if matches!(command, C::SettingsOpen | C::AuthMenu) {
            self.window.show(ctx);
        }
        self.run_command(ctx, command);
    }
}
