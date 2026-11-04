//! Presentation of the Viewer's tree: a summary above a virtualized list of
//! rows with name, file size and date, and the chevron on the right.
use super::{ArchiveInfo, Chevron, Failure, Folder, Level, Row, Tree};
use crate::ui::{
    app::viewer::Viewer,
    components::tooltip::{NativeTooltipExt, Tooltip},
    format::{format_date, format_number, format_size},
    i18n::{FluentArgs, I18nContext, Message},
    theme::ThemeContext,
};
use gpui::{prelude::*, *};

pub(super) const ROW_HEIGHT: Rems = rems(1.625);
const INDENT: Rems = rems(1.25);
const PADDING: Rems = rems(1.0);

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

fn counts(folder: &Folder, cx: &App) -> SharedString {
    let language = cx.i18n().language();
    let label = |count: usize, message| {
        let mut args = FluentArgs::new();
        args.set("count", count);
        args.set("number", format_number(count as u64, language));
        cx.i18n().text_with_args(message, &args)
    };
    let mut parts = vec![label(folder.folders, Message::StatusFolders), label(folder.files, Message::StatusFiles)];
    if folder.files > 0 {
        parts.push(format_size(folder.file_bytes, false, true, language).into());
    }
    parts.join(" · ").into()
}

fn archive_line(info: &ArchiveInfo, cx: &App) -> SharedString {
    let mut args = FluentArgs::new();
    args.set("size", format_size(info.extracted, false, true, cx.i18n().language()));
    match info.percent() {
        Some(percent) => {
            args.set("percent", percent);
            cx.i18n().text_with_args(Message::ViewerTreeArchiveRatio, &args)
        }
        None => cx.i18n().text_with_args(Message::ViewerTreeArchiveSize, &args),
    }
}

/// Summary lines, computed once per render and drawn as the list's first item.
struct Summary {
    lines: Vec<SharedString>,
    password: Option<SharedString>,
}

fn summary(tree: &Tree, folder: &Folder, cx: &App) -> Summary {
    let mut lines = vec![counts(folder, cx)];
    if !folder.complete {
        let mut args = FluentArgs::new();
        args.set("number", format_number(super::READ_PER_FOLDER as u64, cx.i18n().language()));
        lines.push(cx.i18n().text_with_args(Message::ViewerTreeUnsorted, &args));
    }
    if let Some(info) = &tree.archive {
        lines.push(archive_line(info, cx));
    }
    let password = tree.archive.filter(|info| info.password).map(|_| cx.i18n().text(Message::ViewerTreePassword));
    Summary { lines, password }
}

fn summary_element(summary: &Summary, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    div()
        .w_full()
        .px(PADDING)
        .pb(rems(0.5))
        .mb(rems(0.25))
        .flex()
        .flex_col()
        .gap(rems(0.125))
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .text_color(theme.text_muted)
        .children(summary.lines.iter().map(|line| div().w_full().truncate().child(line.clone())))
        .when_some(summary.password.clone(), |element, text| {
            element.child(
                div()
                    .flex()
                    .items_center()
                    .gap(rems(0.375))
                    .child(svg().path("icons/key.svg").size(rems(0.875)).flex_none().text_color(theme.text_muted))
                    .child(div().min_w_0().truncate().child(text)),
            )
        })
        .into_any_element()
}

fn note(depth: usize, text: SharedString, cx: &App) -> Div {
    div()
        .w_full()
        .h(ROW_HEIGHT)
        .flex()
        .items_center()
        .pl(PADDING + INDENT * depth as f32)
        .pr(PADDING)
        .text_color(cx.theme().text_muted)
        .child(div().min_w_0().truncate().child(text))
}

fn chevron(
    index: usize,
    path: String,
    state: Chevron,
    viewer: WeakEntity<Viewer>,
    cx: &App,
) -> AnyElement {
    let theme = *cx.theme();
    let slot = div().id(("viewer-tree-chevron", index)).flex_none().size(rems(1.25)).flex().items_center().justify_center().rounded_sm();
    let icon = match state {
        Chevron::None => return slot.into_any_element(),
        Chevron::Expanded => "icons/chevron-down.svg",
        Chevron::Collapsed | Chevron::Disabled => "icons/chevron-right.svg",
    };
    let enabled = state != Chevron::Disabled;
    slot.child(svg().path(icon).size_3().text_color(theme.text).when(!enabled, |icon| icon.opacity(0.35)))
        // The chevron alone opens a folder; its presses never reach the row's double-click.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .when(enabled, |slot| {
            slot.hover(move |style| style.bg(theme.control_hover)).on_click(move |_, _, cx| {
                let _ = viewer.update(cx, |viewer, cx| viewer.toggle_tree_folder(path.clone(), cx));
            })
        })
        .when(!enabled, |slot| slot.fixed_tooltip(Tooltip::text(cx.i18n().text(Message::ViewerTreeRowLimit))))
        .into_any_element()
}

fn entry_row(
    index: usize,
    file: &oxyde_lib::models::FileEntry,
    depth: usize,
    state: Chevron,
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
    let reveal = viewer.clone();
    let path = file.path.clone();
    div()
        .id(("viewer-tree-row", index))
        .w_full()
        .h(ROW_HEIGHT)
        .flex()
        .items_center()
        .gap(rems(0.5))
        .pl(PADDING + INDENT * depth as f32)
        .pr(rems(0.5))
        .overflow_hidden()
        .hover(move |style| style.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            if event.click_count == 2 {
                let path = path.clone();
                let _ = reveal.update(cx, |viewer, cx| viewer.reveal_tree_item(path, cx));
            }
        })
        .child(crate::ui::components::icons::render_file_icon(
            file.path.clone(),
            crate::ui::components::icons::entry_icon(file),
            rems(1.0),
            &theme,
            system_icons,
            false,
        ))
        .child(div().flex_1().min_w_0().truncate().text_color(theme.text).child(file.name.clone()))
        .child(div().flex_none().w(rems(5.5)).text_right().truncate().text_color(theme.text_muted).child(size))
        .child(div().flex_none().w(rems(8.5)).truncate().text_color(theme.text_muted).child(date))
        .child(chevron(index, file.path.clone(), state, viewer, cx))
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
        .hover(move |style| style.bg(theme.hover))
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
        let header = summary(tree, folder, cx);
        let rows = tree.rows.clone();
        let viewer = cx.entity().downgrade();
        let system_icons = self.owner.upgrade().is_some_and(|owner| owner.read(cx).use_system_icons());
        list(tree.list.clone(), move |index, _, cx| {
            let viewer = viewer.clone();
            let Some(row) = index.checked_sub(1).map(|index| rows.get(index)) else {
                return summary_element(&header, cx);
            };
            match row {
                Some(Row::Entry { file, depth, chevron }) => entry_row(index, file, *depth, *chevron, system_icons, viewer, cx),
                Some(Row::Loading { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeLoading), cx).into_any_element(),
                Some(Row::Failed { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeFailed), cx).into_any_element(),
                Some(Row::Empty { depth }) => note(*depth, cx.i18n().text(Message::ViewerTreeEmpty), cx).into_any_element(),
                Some(Row::More { folder, count, exact, depth }) => more_row(index, folder.clone(), *count, *exact, *depth, viewer, cx),
                None => div().into_any_element(),
            }
        })
        .size_full()
        .py(rems(1.0))
        .into_any_element()
    }
}
