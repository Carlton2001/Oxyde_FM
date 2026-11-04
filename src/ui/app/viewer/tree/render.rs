//! Presentation of the Viewer's tree: a summary above a virtualized list of
//! rows with name, file size and date, and the chevron on the right.
use super::{ArchiveInfo, Chevron, Count, Failure, Folder, Level, Row, Totals, Tree};
use crate::ui::{
    app::viewer::Viewer,
    components::tooltip::{NativeTooltipExt, Tooltip},
    format::{format_date, format_number, format_size},
    i18n::{FluentArgs, I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

pub(super) const ROW_HEIGHT: Rems = crate::ui::style::ROW_HEIGHT;
const INDENT: Rems = rems(1.25);
/// The toolbar's and the pane's side margin, shared by the summary and the rows.
const PADDING: Rems = rems(0.5);
/// Space above the first row and below the last, part of the scroll.
pub(in crate::ui::app::viewer) const LIST_PADDING: Rems = rems(0.5);

fn centered(text: SharedString, cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(PADDING)
        .text_color(cx.theme().text_muted)
        .child(text)
        .into_any_element()
}

fn counts(totals: &Totals, cx: &App) -> Vec<SharedString> {
    let language = cx.i18n().language();
    let label = |count: u64, message| {
        let mut args = FluentArgs::new();
        args.set("count", count);
        args.set("number", format_number(count, language));
        cx.i18n().text_with_args(message, &args)
    };
    let content = totals.content;
    let mut parts = vec![
        label(content.folders, Message::StatusFolders),
        label(content.files, Message::StatusFiles),
        format_size(content.bytes, false, true, language).into(),
    ];
    match totals.count {
        Count::Running => parts.push(cx.i18n().text(Message::ViewerTreeCounting)),
        Count::Capped => parts.push(label(super::COUNT_LIMIT, Message::ViewerTreeCountCapped)),
        Count::Done => {}
    }
    parts
}

fn archive_line(info: &ArchiveInfo, cx: &App) -> Option<SharedString> {
    let mut args = FluentArgs::new();
    args.set("percent", info.percent()?);
    Some(cx.i18n().text_with_args(Message::ViewerTreeArchiveRatio, &args))
}

/// Summary lines, computed once per render and drawn as the list's first item.
struct Summary {
    lines: Vec<SharedString>,
    password: Option<SharedString>,
}

fn summary(tree: &Tree, folder: &Folder, cx: &App) -> Summary {
    let mut lines = tree.totals.as_ref().map(|totals| counts(totals, cx)).unwrap_or_default();
    if !folder.complete {
        let mut args = FluentArgs::new();
        args.set("number", format_number(super::READ_PER_FOLDER as u64, cx.i18n().language()));
        lines.push(cx.i18n().text_with_args(Message::ViewerTreeUnsorted, &args));
    }
    lines.extend(tree.archive.as_ref().and_then(|info| archive_line(info, cx)));
    let password = tree.archive.filter(|info| info.password).map(|_| cx.i18n().text(Message::ViewerTreePassword));
    Summary { lines, password }
}

/// A band under the toolbar, with its colors, margins and minimum height.
fn summary_element(summary: &Summary, cx: &App) -> Div {
    let theme = *cx.theme();
    let item = |text: SharedString| div().flex_none().whitespace_nowrap().child(text);
    div()
        .flex_none()
        .w_full()
        .min_h(crate::ui::style::NAV_BAR_HEIGHT)
        .px(PADDING)
        .py(rems(0.25))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(rems(1.0))
        .gap_y(rems(0.125))
        .bg(theme.surface)
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .text_size(rems(0.75))
        .text_color(theme.text_muted)
        .children(summary.lines.iter().cloned().map(item))
        .when_some(summary.password.clone(), |element, text| {
            element.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(rems(0.375))
                    .child(svg().path("icons/key.svg").size(rems(0.875)).flex_none().text_color(theme.text_muted))
                    .child(item(text)),
            )
        })
}

fn note(depth: usize, text: SharedString, cx: &App) -> Div {
    div()
        .w_full()
        .h(ROW_HEIGHT)
        .flex()
        .items_center()
        // Aligned with the names: past the toggle slot of the rows above.
        .pl(PADDING + INDENT * depth as f32 + rems(1.25))
        .pr(PADDING)
        .text_size(rems(0.8125))
        .text_color(cx.theme().text_muted)
        .child(div().min_w_0().truncate().child(text))
}

/// The sidebar tree's toggle, before the icon; it opens on press, like there.
fn toggle(index: usize, path: String, state: Chevron, viewer: WeakEntity<Viewer>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let slot = div().id(("viewer-tree-toggle", index)).w_4().h_4().mr_1().flex_none().flex().items_center().justify_center();
    let icon = match state {
        Chevron::None => return slot.into_any_element(),
        Chevron::Expanded => "icons/chevron-down.svg",
        Chevron::Collapsed | Chevron::Disabled => "icons/chevron-right.svg",
    };
    let enabled = state != Chevron::Disabled;
    slot.child(svg().path(icon).size_full().text_color(theme.text).when(!enabled, |icon| icon.opacity(0.35)))
        // Its presses never reach the row's double-click.
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            if enabled {
                let _ = viewer.update(cx, |viewer, cx| viewer.toggle_tree_folder(path.clone(), cx));
            }
        })
        .when(!enabled, |slot| slot.fixed_tooltip(Tooltip::text(cx.i18n().text(Message::ViewerTreeRowLimit))))
        .into_any_element()
}

/// A column at the pane's size, aligned by its minimum width but never cut.
fn column(text: String, min_width: Rems, right: bool, cx: &App) -> Div {
    div()
        .flex_none()
        .min_w(min_width)
        .px(rems(0.5))
        .whitespace_nowrap()
        .when(right, |cell| cell.text_right())
        .text_size(rems(0.75))
        .text_color(cx.theme().text_muted)
        .child(text)
}

#[allow(clippy::too_many_arguments)]
fn entry_row(
    index: usize,
    file: &oxyde_lib::models::FileEntry,
    depth: usize,
    state: Chevron,
    subfolders: bool,
    system_icons: bool,
    viewer: WeakEntity<Viewer>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let language = cx.i18n().language();
    let size = (!file.is_dir).then(|| format_size(file.size, false, true, language)).unwrap_or_default();
    let date = (file.modified > 0)
        .then(|| format_date(file.modified, crate::ui::format::date_format(cx)))
        .unwrap_or_default();
    let icon = match crate::ui::components::icons::entry_icon(file) {
        crate::ui::components::icons::IconType::Folder if state == Chevron::Expanded && !system_icons => {
            crate::ui::components::icons::IconType::FolderOpen
        }
        icon => icon,
    };
    let reveal = viewer.clone();
    let path = file.path.clone();
    // The name and the pane's subfolder indicator share the flexible width.
    let name = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .child(div().flex_shrink(1.0).min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(theme.text).child(file.name.clone()))
        .when(subfolders, |name| {
            name.child(div().ml_2().flex_none().child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text)))
        });
    div()
        .id(("viewer-tree-row", index))
        .w_full()
        .h(crate::ui::style::ROW_HEIGHT)
        .flex()
        .items_center()
        .pl(PADDING + INDENT * depth as f32)
        .pr(rems(0.5))
        .text_size(rems(0.8125))
        .hover(move |style| style.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            if event.click_count == 2 {
                let path = path.clone();
                let _ = reveal.update(cx, |viewer, cx| viewer.reveal_tree_item(path, cx));
            }
        })
        .child(toggle(index, file.path.clone(), state, viewer, cx))
        .child(div().flex_none().mr_2().child(crate::ui::components::icons::render_file_icon(
            file.path.clone(),
            icon,
            rems(1.0),
            &theme,
            system_icons,
            false,
        )))
        .child(name)
        .child(column(size, rems(5.5), true, cx))
        .child(column(date, rems(8.0), false, cx))
        .into_any_element()
}

fn more_row(index: usize, folder: String, count: usize, exact: bool, depth: usize, viewer: WeakEntity<Viewer>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let mut args = FluentArgs::new();
    args.set("count", count);
    args.set("number", format_number(count as u64, cx.i18n().language()));
    let text = cx.i18n().text_with_args(if exact { Message::ViewerTreeMore } else { Message::ViewerTreeMoreAtLeast }, &args);
    note(depth, text, cx)
        .id(("viewer-tree-more", index))
        .hover(move |style| style.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            if event.click_count == 2 {
                let folder = folder.clone();
                let _ = viewer.update(cx, |viewer, cx| viewer.open_tree_folder(folder, cx));
            }
        })
        .into_any_element()
}

impl Viewer {
    pub(in crate::ui::app::viewer) fn render_tree(&self, tree: &Tree, cx: &mut Context<Self>) -> AnyElement {
        let folder = match tree.root_level() {
            Some(Level::Ready(folder)) => folder,
            Some(Level::Failed(Failure::Locked)) => return centered(cx.i18n().text(Message::ViewerTreeLocked), cx),
            Some(Level::Failed(Failure::Unreadable)) | None => return centered(cx.i18n().text(Message::ViewerTreeFailed), cx),
            Some(Level::Loading) => return centered(cx.i18n().text(Message::ViewerLoading), cx),
        };
        let header = summary_element(&summary(tree, folder, cx), cx);
        let rows = tree.rows.clone();
        let viewer = cx.entity().downgrade();
        let system_icons = self.owner.upgrade().is_some_and(|owner| owner.read(cx).use_system_icons());
        let body = list(tree.list.clone(), move |index, _, cx| {
            let viewer = viewer.clone();
            match rows.get(index) {
                Some(Row::Entry { file, depth, chevron, subfolders }) => entry_row(index, file, *depth, *chevron, *subfolders, system_icons, viewer, cx),
                Some(Row::Loading { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeLoading), cx).into_any_element(),
                Some(Row::Failed { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeFailed), cx).into_any_element(),
                Some(Row::Empty { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeEmpty), cx).into_any_element(),
                Some(Row::More { folder, count, exact, depth }) => more_row(index, folder.clone(), *count, *exact, *depth, viewer, cx),
                None => div().into_any_element(),
            }
        })
        .size_full()
        .py(LIST_PADDING);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(div().flex_1().min_h_0().child(self.with_scrollbar(body.into_any_element(), cx)))
            .into_any_element()
    }
}
