mod icons;
mod outcome;
mod window;

use crate::ui::app::OxydeApp;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::FileOperation;
use std::path::Path;

pub(super) fn close_windows(cx: &mut App) {
    window::close_windows(cx);
}

struct Item {
    path: std::path::PathBuf,
    destination: Option<std::path::PathBuf>,
    name: String,
    parent: String,
    search: String,
}

impl Item {
    fn new(path: &Path, destination: Option<&Path>) -> Self {
        let full = path.to_string_lossy();
        Self {
            path: path.to_path_buf(),
            destination: destination
                .and_then(|destination| path.file_name().map(|name| destination.join(name))),
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| full.to_string()),
            parent: path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default(),
            search: full.to_lowercase(),
        }
    }
}

fn count_label(message: Message, count: usize, cx: &App) -> SharedString {
    let mut args = FluentArgs::new();
    args.set("count", count as i64);
    args.set("number", format_number(count as u64, cx.i18n().language()));
    cx.i18n().text_with_args(message, &args)
}

fn row(
    item: &Item,
    system: bool,
    status: Option<oxyde_lib::systems::file_ops::ItemOutcome>,
    cx: &App,
) -> Div {
    let theme = *cx.theme();
    div()
        .w_full()
        .h(rems(3.0))
        .flex_none()
        .flex()
        .items_center()
        .gap(rems(0.625))
        .px(rems(0.5))
        .child(icons::ItemIcon {
            path: item.path.clone(),
            destination: item.destination.clone(),
            system,
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(rems(0.125))
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(item.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(rems(0.75))
                        .text_color(theme.text_muted)
                        .child(item.parent.clone()),
                ),
        )
        .when_some(status, |row, status| row.child(outcome::badge(status, cx)))
}

pub(super) fn preview(
    operation: &FileOperation,
    system: bool,
    app: Entity<OxydeApp>,
    cx: &App,
) -> Div {
    let theme = *cx.theme();
    let mut list = div()
        .p(rems(0.625))
        .rounded(rems(0.5))
        .bg(theme.background)
        .flex()
        .flex_col()
        .gap(rems(0.25))
        .child(
            div()
                .px(rems(0.5))
                .pb(rems(0.25))
                .text_size(rems(0.75))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_muted)
                .child(count_label(
                    Message::OpsAffectedItems,
                    operation.sources.len(),
                    cx,
                )),
        );
    for source in operation.sources.iter().take(5) {
        list = list.child(row(
            &Item::new(source, operation.destination.as_deref()),
            system,
            operation.item_outcomes.get(source).copied(),
            cx,
        ));
    }
    if operation.sources.len() > 5 {
        let id = operation.id.clone();
        list = list.child(
            div().flex().child(
                super::super::tool_window::action_button(
                    format!("operation-files-{id}"),
                    "".into(),
                    false,
                    cx,
                )
                .child(
                    svg()
                        .path("icons/list.svg")
                        .size(rems(0.875))
                        .text_color(theme.icon_muted),
                )
                .child(count_label(
                    Message::OpsViewItems,
                    operation.sources.len(),
                    cx,
                ))
                .on_click(move |_, _, cx| window::open(app.clone(), id.clone(), cx)),
            ),
        );
    }
    list
}
