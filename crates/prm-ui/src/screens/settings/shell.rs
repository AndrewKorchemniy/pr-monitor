//! The settings window: a large modal over the dashboard with a darker sidebar of pages on the
//! left and the selected page on the right. Unsaved changes show a Save bar at the bottom.

use eframe::egui::{self, Margin, Rect, RichText, Sense, Stroke, UiBuilder, pos2, vec2};

use super::{
    Editor, PAGES, Page, SettingsEnv, SettingsState, appearance, general, notifications, orgs,
};
use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    theme::{UiTheme, adapter, icons},
    widgets::{self, controls},
};

const NAV_WIDTH: f32 = 232.0;
/// Left and right padding of the page column, the same inset as dialogs.
const PAGE_PAD: i8 = 20;

pub(crate) fn settings_modal(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let screen = ctx.content_rect().size();
    let size = vec2(
        (screen.x - 64.0).clamp(560.0, 1040.0),
        (screen.y - 64.0).clamp(420.0, 780.0),
    );
    let frame = adapter::frame_dialog(&theme, theme.policy).inner_margin(Margin::ZERO);
    let modal = egui::Modal::new(egui::Id::unique("settings"))
        .frame(frame)
        .show(ctx, |ui| {
            let sidebar_fill = ui.painter().add(egui::Shape::Noop);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let nav = Rect::from_min_size(rect.min, vec2(NAV_WIDTH, rect.height()));
            let page = Rect::from_min_max(pos2(nav.right(), rect.top()), rect.max);
            let inner = nav.shrink2(vec2(12.0, 16.0));
            let builder = UiBuilder::new().id_salt("settings_nav").max_rect(inner);
            ui.scope_builder(builder, |ui| sidebar(ui, cx, st, env));
            let builder = UiBuilder::new().id_salt("settings_page").max_rect(page);
            ui.scope_builder(builder, |ui| page_column(ui, cx, st, env));
            // why: painted last, from the window's final height, so it always reaches the bottom.
            let full = Rect::from_min_max(nav.min, pos2(nav.right(), ui.min_rect().bottom()));
            ui.painter().set(sidebar_fill, sidebar_shape(&theme, full));
        });
    if modal.backdrop_response.clicked() && !busy(cx, st) {
        cx.push(UiAction::LeaveSettings);
    }
    orgs::editor(ctx, cx, st, env);
}

/// Something is in progress that a stray click outside settings must not interrupt: a sign-in
/// (often finished in a browser window over this one), a save, or an open dialog.
fn busy(cx: &FrameCx<'_>, st: &SettingsState) -> bool {
    let signing_in = cx
        .state
        .orgs
        .iter()
        .any(|o| matches!(o.auth.state, prm_service::AuthStateView::SigningIn { .. }));
    signing_in || st.saving || st.editing.is_some()
}

/// The darker sidebar fill (rounded on the window's left corners) and its edge.
fn sidebar_shape(theme: &UiTheme, nav: Rect) -> egui::Shape {
    let r = theme.tokens.metrics.radius.lg;
    let corners = egui::CornerRadius {
        nw: r,
        sw: r,
        ne: 0,
        se: 0,
    };
    let edge = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_subtle,
    );
    egui::Shape::Vec(vec![
        egui::Shape::rect_filled(nav, corners, theme.colors.surface_canvas),
        egui::Shape::vline(nav.right(), nav.y_range(), edge),
    ])
}

fn sidebar(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState, env: &SettingsEnv<'_>) {
    let theme = cx.theme.clone();
    ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
    ui.label(
        RichText::new(copy::SETTINGS)
            .font(theme.fonts.label.clone())
            .color(theme.colors.text_muted),
    );
    ui.add_space(theme.tokens.metrics.space.sm);
    for page in PAGES {
        if nav_item(ui, &theme, page, st.page == page).clicked() {
            st.page = page;
        }
    }
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        about_footer(ui, cx, st, env);
    });
}

/// A full-width sidebar entry with its icon; the current page is filled.
fn nav_item(ui: &mut egui::Ui, theme: &UiTheme, page: Page, selected: bool) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = vec2(ui.available_width(), theme.control_height() - m.space.xs);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        Some(theme.colors.control_active)
    } else if response.hovered() {
        Some(theme.colors.control_hover)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, m.radius.md, fill);
    }
    let color = if selected {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };
    let glyph = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(15.0, 15.0));
    icons::paint(ui.painter(), glyph, page.icon(), color);
    let title = page.title();
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), theme.fonts.body.clone(), color);
    let at = pos2(
        glyph.right() + m.space.md + 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, color);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, title));
    response
}

/// Bottom of the sidebar: the version, and links to the source, problems and licenses.
fn about_footer(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let small = |text: &str| RichText::new(text).font(theme.fonts.small.clone());
    let mut opened = None;
    // why: bottom-up layout, so the last link added sits highest.
    for (label, url) in [
        (
            copy::REPORT_PROBLEM,
            prm_domain::app::DocLink::ReportProblem.url(),
        ),
        (copy::SOURCE_CODE, prm_domain::app::REPO_URL),
    ] {
        if ui.link(small(label)).clicked() {
            opened = url::Url::parse(url).ok();
        }
    }
    if ui.link(small(copy::LICENSES_SHORT)).clicked() {
        st.editing = Some(Editor::Licenses);
    }
    ui.add_space(theme.tokens.metrics.space.xs);
    let build = format!("{} · {}", env.build.git_sha, env.build.target);
    let version = format!("{} {}", copy::APP_NAME, env.build.version);
    widgets::muted(ui, &theme, &version).on_hover_text(build);
    if let Some(url) = opened {
        cx.push(UiAction::Send(prm_service::Command::OpenUrl(url)));
    }
}

/// The page title and close button, the scrolling page, and the Save bar when dirty.
fn page_column(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let area = ui.max_rect();
    let inset = f32::from(PAGE_PAD);
    let header = Rect::from_min_size(
        area.min,
        vec2(area.width(), theme.control_height() + 2.0 * inset),
    );
    let bar = if st.dirty() {
        theme.control_height() + 2.0 * inset + 1.0
    } else {
        0.0
    };
    let body = Rect::from_min_max(
        pos2(area.left(), header.bottom()),
        pos2(area.right(), area.bottom() - bar),
    );
    let builder = UiBuilder::new()
        .id_salt("settings_header")
        .max_rect(header.shrink2(vec2(inset, 0.0)));
    ui.scope_builder(builder, |ui| page_header(ui, cx, st));
    let builder = UiBuilder::new().id_salt("settings_body").max_rect(body);
    ui.scope_builder(builder, |ui| {
        // why: an overlay scrollbar, so rows keep their width when the page starts to scroll
        // (as it does when the Save bar appears).
        ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(PAGE_PAD, 0))
                    .show(ui, |ui| page_body(ui, cx, st, env));
            });
    });
    if bar > 0.0 {
        let rect = Rect::from_min_max(pos2(area.left(), body.bottom()), area.max);
        let builder = UiBuilder::new().id_salt("settings_save_bar").max_rect(rect);
        ui.scope_builder(builder, |ui| save_bar(ui, cx, st));
    }
}

fn page_header(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &SettingsState) {
    let theme = cx.theme.clone();
    ui.horizontal_centered(|ui| {
        if st.page == Page::Organizations {
            orgs::breadcrumb(ui, &theme, st);
        } else {
            widgets::page_title(ui, &theme, st.page.title());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if controls::close_button(ui, &theme, copy::CLOSE).clicked() {
                cx.push(UiAction::LeaveSettings);
            }
        });
    });
}

fn page_body(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    ui.set_width(ui.available_width());
    match st.page {
        Page::General => general::page(ui, cx, &mut st.draft, env.columns),
        Page::Organizations => orgs::page(ui, cx, st),
        Page::Notifications => notifications::page(ui, cx, &mut st.draft),
        Page::Appearance => appearance::page(ui, cx, &mut st.draft, env),
    }
    ui.add_space(cx.theme.tokens.metrics.space.xxl);
}

fn save_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>) {
    let theme = cx.theme.clone();
    if widgets::primary_button(ui, &theme, copy::SAVE_CHANGES).clicked() {
        cx.push(UiAction::SaveSettings);
    }
    if ui.button(copy::DISCARD).clicked() {
        cx.push(UiAction::DiscardSettings);
    }
}

/// "You have unsaved changes" with Discard and Save, under a divider; it fills its rect exactly.
fn save_bar(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &SettingsState) {
    let theme = cx.theme.clone();
    let rect = ui.max_rect();
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    ui.painter().hline(rect.x_range(), rect.top(), stroke);
    let row = rect.shrink(f32::from(PAGE_PAD));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(row).layout(layout), |ui| {
        widgets::muted(ui, &theme, copy::UNSAVED);
        let right = egui::Layout::right_to_left(egui::Align::Center);
        ui.with_layout(right, |ui| {
            ui.add_enabled_ui(!st.saving, |ui| save_buttons(ui, cx));
        });
    });
}
