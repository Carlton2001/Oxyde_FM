//! A folder of the sidebar's tree, as the main sidebar and the file chooser's places
//! draw it: indented by its depth, a chevron when it holds folders, its icon open
//! while unfolded, dimmed when hidden or system, and the accent bar when selected.
//! The caller adds what a press, a tooltip or a drop does there.
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// The folder rows' height, that of the file list's rows.
pub(crate) fn height() -> f32 {
    super::sidebar_row_h()
}

/// The room before a folder's label at `depth`: its indent, chevron, icon and gaps.
pub(crate) fn chrome(depth: usize) -> f32 {
    indent(depth) + rem_pixels(rems(1.0 + 0.25 + 1.0 + 0.5 + 0.75)).as_f32()
}

fn indent(depth: usize) -> f32 {
    logical(8.0) + depth as f32 * logical(12.0)
}

/// The chevron that unfolds a drive or a folder, or its empty room when there is
/// nothing to unfold; a press there toggles without opening.
pub(crate) fn chevron(expandable: bool, expanded: bool, theme: OxydeTheme, toggle: impl Fn(&mut Window, &mut App) + 'static) -> Div {
    div()
        .w_4().h_4().mr_1()
        .flex().items_center().justify_center().flex_shrink_0()
        .when(expandable, |chevron| chevron
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                toggle(window, cx);
            })
            .child(svg()
                .path(if expanded { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" })
                .size_full()
                .text_color(theme.text)))
}

/// What a folder row shows.
pub(crate) struct FolderRow<'a> {
    pub(crate) path: &'a str,
    pub(crate) name: &'a str,
    pub(crate) depth: usize,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) selected: bool,
    /// Hidden or system.
    pub(crate) dimmed: bool,
    pub(crate) use_system_icons: bool,
}

impl FolderRow<'_> {
    /// The row, its chevron toggling it with `toggle`; the caller sets its width.
    pub(crate) fn render(self, id: impl Into<ElementId>, theme: OxydeTheme, toggle: impl Fn(&mut Window, &mut App) + 'static) -> Stateful<Div> {
        use crate::ui::components::icons::{render_file_icon, IconType};
        let hover = if self.selected { theme.selection_hover } else { theme.hover };
        let icon = if self.expanded && !self.use_system_icons { IconType::FolderOpen } else { IconType::Folder };
        div()
            .id(id)
            .h(px(height()))
            .pl(px(indent(self.depth)))
            .pr_3()
            .flex()
            .items_center()
            .when(self.dimmed, |row| row.opacity(0.75))
            .relative()
            .when(self.selected, |row| row.bg(theme.selected)
                .child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .hover(move |style| style.bg(hover))
            .child(chevron(self.expandable, self.expanded, theme, toggle))
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_shrink_0()
                    .child(render_file_icon(self.path.to_string(), icon, rems(1.0), &theme, self.use_system_icons, false)),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(theme.text).child(self.name.to_string()))
    }
}
