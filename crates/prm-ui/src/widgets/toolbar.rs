//! The toolbar: the filter field, then grouping (and sorting, while the list is cards), refresh
//! and settings as ghost buttons (so the bar reads as text), with the refresh progress line
//! along its bottom edge. In a narrow window the field shrinks first, then the menus show only
//! their icons; nothing wraps or overlaps.

use eframe::egui;
use prm_domain::view::{GroupBy, Sort, SortKey};

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    theme::{UiTheme, icons::Icon},
    view::columns::{COLUMNS, ColumnId},
    widgets::{
        controls, fields,
        progress::{self, ProgressLine},
    },
};

pub(crate) const FILTER_ID: &str = "filter";

pub(crate) const GROUPS: [(GroupBy, &str, CommandId); 6] = [
    (GroupBy::Status, "Status", CommandId::GroupStatus),
    (GroupBy::None, "None", CommandId::GroupNone),
    (GroupBy::Author, "Author", CommandId::GroupAuthor),
    (GroupBy::Repo, "Repository", CommandId::GroupRepo),
    (GroupBy::Project, "Project", CommandId::GroupProject),
    (GroupBy::Org, "Organization", CommandId::GroupOrg),
];

/// The filter field gives up room down to this before the menus drop their text (points).
const SEARCH_MIN: f32 = 160.0;

pub(crate) fn group_label(group: GroupBy) -> &'static str {
    GROUPS
        .iter()
        .find(|(g, _, _)| *g == group)
        .map_or("None", |(_, label, _)| label)
}

/// The sort menu's keys and names: the table's sortable columns (Organization while that column
/// shows), then last activity.
fn sort_keys(show_org: bool) -> impl Iterator<Item = (SortKey, &'static str)> {
    COLUMNS
        .iter()
        .filter(move |c| show_org || c.id != ColumnId::Org)
        .filter_map(|c| Some((c.sort?, c.name)))
        .chain([(SortKey::Activity, copy::LAST_ACTIVITY)])
}

fn sort_label(key: SortKey) -> &'static str {
    sort_keys(true)
        .find(|(k, _)| *k == key)
        .map_or(copy::LAST_ACTIVITY, |(_, label)| label)
}

pub(crate) fn filter_id() -> egui::Id {
    egui::Id::unique(FILTER_ID)
}

/// What the toolbar offers besides the filter.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ToolbarInputs {
    pub group: GroupBy,
    /// The sort, while the list is cards and so has no column headers to sort from.
    pub sort: Option<Sort>,
    /// The organization column is on, so the organization is a sort key too.
    pub show_org: bool,
    /// The background service has stopped, so refreshing does nothing.
    pub stopped: bool,
}

/// Returns whether the filter field has focus.
pub(crate) fn toolbar(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    filter: &mut String,
    inputs: ToolbarInputs,
    line: &mut ProgressLine,
) -> bool {
    let theme = cx.theme.clone();
    let rect = ui.max_rect();
    let mut focused = false;
    ui.horizontal_centered(|ui| {
        // why: the glyph's gap to the window's left edge matches its gap to the divider below.
        let frame_left = rect.left() - ui.ctx().content_rect().left();
        let below = (rect.height() - fields::SEARCH_GLYPH) / 2.0;
        ui.add_space((below - frame_left - fields::SEARCH_GLYPH_INSET).max(0.0));
        let fit = Fit::of(ui, &theme, inputs);
        let id = (filter_id(), copy::FILTER_HINT);
        let response = fields::search_field(ui, &theme, filter, id, (0.0, fit.search));
        if response.changed() {
            cx.push(UiAction::SetFilterText(filter.clone()));
        }
        focused = response.has_focus();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            buttons(ui, cx, inputs, &fit);
        });
    });
    // why: edge to edge like the divider it sits on, not inside the toolbar's margin.
    let full = rect.expand2(egui::vec2(theme.tokens.metrics.space.md, 0.0));
    let bar = egui::Rect::from_min_max(
        egui::pos2(full.left(), full.bottom() - progress::HEIGHT),
        full.max,
    );
    line.show(ui, bar, cx.state.refresh_fraction(), &theme);
    focused
}

/// How the Group and Sort menu buttons show.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MenuLook {
    /// "Group  Status ⌄".
    Labelled,
    /// Only the icon, its label and value in the tooltip, for a toolbar short of room.
    Icon,
}

/// How the toolbar fits its width: the widest the filter field may get, how the menus show,
/// and how wide the buttons at the right are, gaps included.
struct Fit {
    search: f32,
    menus: MenuLook,
    buttons: f32,
}

impl Fit {
    fn of(ui: &egui::Ui, theme: &UiTheme, inputs: ToolbarInputs) -> Self {
        let side = theme.control_height();
        let gap = theme.tokens.metrics.space.sm;
        let group = controls::menu_button_width(ui, theme, copy::GROUP, group_label(inputs.group));
        let sort = inputs.sort.map_or(0.0, |sort| {
            controls::menu_button_width(ui, theme, copy::SORT, sort_label(sort.key))
        });
        let menus = if inputs.sort.is_some() { 2.0 } else { 1.0 };
        // why: settings and refresh, the gaps between all the buttons, and the gap to the field.
        let fixed = 2.0 * side + (menus + 1.0) * gap + ui.spacing().item_spacing.x;
        let room = ui.available_width();
        let (look, menus_width) = if room - (group + sort + fixed) < SEARCH_MIN {
            (MenuLook::Icon, menus * side)
        } else {
            (MenuLook::Labelled, group + sort)
        };
        Self {
            search: (room - fixed - menus_width).max(0.0),
            menus: look,
            buttons: fixed - ui.spacing().item_spacing.x + menus_width,
        }
    }
}

/// Group (and Sort), refresh and settings at the right end of the toolbar, added in the order
/// they read, since Tab follows the order controls are added.
fn buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, inputs: ToolbarInputs, fit: &Fit) {
    let theme = cx.theme.clone();
    let size = egui::vec2(fit.buttons, ui.available_height());
    let row = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, row, |ui| {
        ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.sm;
        group_menu(ui, cx, inputs.group, fit.menus);
        if let Some(sort) = inputs.sort {
            sort_menu(ui, cx, (sort, inputs.show_org), fit.menus);
        }
        refresh_button(ui, cx, inputs.stopped);
        icon_button(ui, cx, Icon::Gear, CommandId::SettingsOpen);
    });
}

/// Refresh, or while a refresh runs, the same button turned off with its arrow spinning.
fn refresh_button(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, stopped: bool) {
    if stopped || !cx.state.refresh.in_progress {
        ui.add_enabled_ui(!stopped, |ui| {
            icon_button(ui, cx, Icon::Refresh, CommandId::RefreshNow);
        });
        return;
    }
    let theme = cx.theme.clone();
    controls::refreshing_button(ui, &theme, CommandId::RefreshNow.title())
        .on_hover_text(copy::REFRESHING);
}

fn icon_button(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, icon: Icon, command: CommandId) {
    let theme = cx.theme.clone();
    let title = cx.titled(command);
    let response =
        controls::ghost_icon_button(ui, &theme, icon, command.title()).on_hover_text(&title);
    if response.clicked() {
        cx.run(command);
    }
}

/// "Label  Value ⌄", or only `icon` when the toolbar is short of room.
fn menu_opener(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (icon, label, value): (Icon, &str, &str),
    look: MenuLook,
) -> egui::Response {
    match look {
        MenuLook::Icon => controls::menu_icon_button(ui, theme, icon, (label, value)),
        MenuLook::Labelled => controls::menu_button(ui, theme, label, value),
    }
}

fn menu_popup(response: &egui::Response, theme: &UiTheme) -> (egui::Popup<'static>, f32) {
    let width = response.rect.width().max(controls::MENU_WIDTH);
    let popup = egui::Popup::menu(response)
        .frame(controls::menu_frame(theme))
        .gap(controls::MENU_GAP)
        .width(width);
    (popup, width)
}

fn group_menu(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, group: GroupBy, look: MenuLook) {
    let theme = cx.theme.clone();
    let opener = (Icon::Group, copy::GROUP, group_label(group));
    let response = menu_opener(ui, &theme, opener, look);
    let (popup, width) = menu_popup(&response, &theme);
    popup.show(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (value, label, command) in GROUPS {
            if controls::menu_item(ui, &theme, label, value == group).clicked() {
                cx.run(command);
                ui.close();
            }
        }
    });
}

/// The keys the column headers sort by (Organization while `show_org`); the current one says
/// which way, and picking it again reverses it.
fn sort_menu(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (sort, show_org): (Sort, bool),
    look: MenuLook,
) {
    let theme = cx.theme.clone();
    let opener = (Icon::Sort, copy::SORT, sort_label(sort.key));
    let response = menu_opener(ui, &theme, opener, look);
    let (popup, width) = menu_popup(&response, &theme);
    let direction = if sort.descending {
        copy::DESCENDING
    } else {
        copy::ASCENDING
    };
    popup.show(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (key, label) in sort_keys(show_org) {
            let selected = key == sort.key;
            let item = (label, selected.then_some(direction));
            let mark = controls::MenuMark::Pointer { selected };
            if controls::menu_item_detail(ui, &theme, item, mark).clicked() {
                cx.push(UiAction::Sort(key));
                ui.close();
            }
        }
    });
}
