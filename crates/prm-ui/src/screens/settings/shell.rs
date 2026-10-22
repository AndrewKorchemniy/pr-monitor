//! The settings window: a large modal over the dashboard with a darker sidebar of pages on the
//! left and the selected page on the right. In a narrow window it fills the window, the sidebar
//! folds into a page menu in the header and the about links end the General page. Unsaved
//! changes show a Save bar at the bottom.

mod about;
mod save_bar;

use eframe::egui::{self, Margin, Rect, RichText, Sense, Stroke, UiBuilder, pos2, vec2};
use prm_domain::auth::SettingsSection;

use super::{
    PAGES, Page, SettingsEnv, SettingsState, appearance, general, notifications, orgs, reveal,
};
use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    theme::{UiTheme, adapter, icons},
    widgets::{self, card, controls, dialog, fit},
};

const NAV_WIDTH: f32 = 232.0;
/// Left and right padding of the page column, the same inset as dialogs.
const PAGE_PAD: i8 = 20;
/// `PAGE_PAD` in points.
const PAGE_INSET: f32 = 20.0;
/// The largest the window gets, in points.
const MAX_SIZE: egui::Vec2 = vec2(1040.0, 780.0);
/// The sidebar shows once the settings window is this wide (points). Narrower, it would leave the
/// page less than a settings row needs to keep its control beside its label
/// (`fit::ROW_STACK_BELOW`), so it folds into a page menu and the page gets the whole width.
const SIDEBAR_FROM: f32 = NAV_WIDTH + 2.0 * PAGE_INSET + fit::ROW_STACK_BELOW;
/// The width of the page menu.
const PAGE_MENU_WIDTH: f32 = 200.0;

/// How the pages are reached.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Nav {
    /// A sidebar of pages, with the about links at its foot.
    Sidebar,
    /// A page menu in the header; the about links end the General page.
    Menu,
}

impl Nav {
    /// The navigation for a settings window `width` points wide.
    fn for_width(width: f32) -> Self {
        if width < SIDEBAR_FROM {
            Self::Menu
        } else {
            Self::Sidebar
        }
    }
}

pub(crate) fn settings_modal(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
) {
    let theme = cx.theme.clone();
    let frame = adapter::frame_dialog(&theme, theme.policy).inner_margin(Margin::ZERO);
    let size = inner_size(ctx.content_rect().size(), frame.total_margin().sum());
    let nav = Nav::for_width(size.x);
    let id = egui::Id::unique("settings");
    let outer = size + frame.total_margin().sum();
    let top = (ctx.content_rect().height() - outer.y) / 2.0;
    let modal = egui::Modal::new(id)
        .area(widgets::modal::dialog_area(ctx, id, outer.x, Some(top)))
        .frame(frame)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            match nav {
                Nav::Sidebar => with_sidebar(ui, cx, st, env, rect),
                Nav::Menu => {
                    let builder = UiBuilder::new().id_salt("settings_page").max_rect(rect);
                    ui.scope_builder(builder, |ui| page_column(ui, cx, st, env, nav));
                }
            }
        });
    if modal.backdrop_response.clicked() && !busy(cx, st) {
        cx.push(UiAction::LeaveSettings);
    }
    orgs::editor(ctx, cx, st, env);
}

/// The settings window's size inside its frame (`chrome` points of margin and stroke) in a
/// window of `window` points.
fn inner_size(window: egui::Vec2, chrome: egui::Vec2) -> egui::Vec2 {
    fit::fit(MAX_SIZE + chrome, window) - chrome
}

/// The sidebar in `rect`'s left edge and the page beside it.
fn with_sidebar(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
    rect: Rect,
) {
    let theme = cx.theme.clone();
    let sidebar_fill = ui.painter().add(egui::Shape::Noop);
    let nav = Rect::from_min_size(rect.min, vec2(NAV_WIDTH, rect.height()));
    let page = Rect::from_min_max(pos2(nav.right(), rect.top()), rect.max);
    let inner = nav.shrink2(vec2(12.0, 16.0));
    let builder = UiBuilder::new().id_salt("settings_nav").max_rect(inner);
    ui.scope_builder(builder, |ui| sidebar(ui, cx, st, env));
    let builder = UiBuilder::new().id_salt("settings_page").max_rect(page);
    ui.scope_builder(builder, |ui| page_column(ui, cx, st, env, Nav::Sidebar));
    // why: painted last, from the window's final height, so it always reaches the bottom.
    let full = Rect::from_min_max(nav.min, pos2(nav.right(), ui.min_rect().bottom()));
    ui.painter().set(sidebar_fill, sidebar_shape(&theme, full));
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
        about::footer(ui, cx, st, env);
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

/// The page title and close button, the scrolling page, and the Save bar when dirty.
fn page_column(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
    nav: Nav,
) {
    let theme = cx.theme.clone();
    let area = ui.max_rect();
    let header = Rect::from_min_size(
        area.min,
        vec2(area.width(), theme.control_height() + 2.0 * PAGE_INSET),
    );
    let bar = st
        .save_bar_shown()
        .then(|| save_bar::layout(ui, &theme, st, area.width() - 2.0 * PAGE_INSET));
    let bar_height = bar.as_ref().map_or(0.0, |b| b.height(&theme, PAGE_INSET));
    let body = Rect::from_min_max(
        pos2(area.left(), header.bottom()),
        pos2(area.right(), area.bottom() - bar_height),
    );
    let builder = UiBuilder::new()
        .id_salt("settings_header")
        .max_rect(header.shrink2(vec2(PAGE_INSET, 0.0)));
    ui.scope_builder(builder, |ui| page_header(ui, cx, st, nav));
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
                    .show(ui, |ui| page_body(ui, cx, st, env, nav));
            });
    });
    if let Some(bar) = bar {
        let rect = Rect::from_min_max(pos2(area.left(), body.bottom()), area.max);
        let builder = UiBuilder::new().id_salt("settings_save_bar").max_rect(rect);
        ui.scope_builder(builder, |ui| save_bar::show(ui, cx, st, (&bar, PAGE_INSET)));
    }
}

/// The page's title (or the page menu) and the close button, which keeps its place however
/// long the title gets.
fn page_header(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, st: &mut SettingsState, nav: Nav) {
    let theme = cx.theme.clone();
    if dialog::with_close(ui, &theme, None, |ui| page_title(ui, &theme, st, nav)) {
        cx.push(UiAction::LeaveSettings);
    }
}

/// The page's title, its breadcrumb on Organizations, or the page menu in a narrow window.
fn page_title(ui: &mut egui::Ui, theme: &UiTheme, st: &mut SettingsState, nav: Nav) {
    match nav {
        Nav::Menu => page_menu(ui, theme, st),
        Nav::Sidebar if st.page == Page::Organizations => orgs::breadcrumb(ui, theme, st),
        Nav::Sidebar => {
            widgets::page_title(ui, theme, st.page.title());
        }
    }
}

/// The sidebar folded into a ghost menu button: "Settings  General ⌄", then a menu of pages.
fn page_menu(ui: &mut egui::Ui, theme: &UiTheme, st: &mut SettingsState) {
    // why: a ghost button's text sits its padding in from its edge; start the button that far
    // into the margin so the text lines up with the page below and the fill hangs outside.
    ui.add_space(-theme.tokens.metrics.space.lg);
    let response = controls::menu_button(ui, theme, copy::SETTINGS, st.page.title());
    egui::Popup::menu(&response)
        .frame(controls::menu_frame(theme))
        .gap(controls::MENU_GAP)
        .width(PAGE_MENU_WIDTH)
        .show(|ui| {
            ui.set_width(PAGE_MENU_WIDTH);
            ui.spacing_mut().item_spacing.y = 0.0;
            for page in PAGES {
                if controls::menu_item(ui, theme, page.title(), st.page == page).clicked() {
                    st.page = page;
                }
            }
        });
}

fn page_body(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    st: &mut SettingsState,
    env: &SettingsEnv<'_>,
    nav: Nav,
) {
    let theme = cx.theme.clone();
    ui.set_width(ui.available_width());
    match st.page {
        Page::General => general::page(ui, cx, &mut st.draft, env.columns),
        Page::Organizations => orgs::page(ui, cx, st),
        Page::Notifications => {
            let test = (&mut st.test_result, &mut st.reveal_test);
            notifications::page(ui, cx, &mut st.draft, test);
        }
        Page::Appearance => appearance::page(ui, cx, &mut st.draft, env),
    }
    if nav == Nav::Menu && st.page == Page::General {
        reveal(ui, st, &[SettingsSection::About]);
        let title = copy::settings_section(SettingsSection::About);
        card::section(ui, &theme, title, None, |ui| {
            about::section(ui, cx, st, env);
        });
    }
    ui.add_space(theme.tokens.metrics.space.xxl);
}

#[cfg(test)]
mod tests;
