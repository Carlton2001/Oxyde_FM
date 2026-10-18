use crate::ui::components::input::{InputAppearance, TextInputOptions};
use super::{count_label, row, Item};
use crate::ui::app::OxydeApp;
use crate::ui::components::{input::TextInput, tool_window};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
mod scroll;

struct FilesWindow {
    owner: WeakEntity<OxydeApp>,
    _owner_subscription: Subscription,
    operation_id: String,
    total: usize,
    outcomes: Arc<std::collections::HashMap<PathBuf, oxyde_lib::systems::file_ops::ItemOutcome>>,
    items: Arc<Vec<Item>>,
    filtered: Arc<Vec<usize>>,
    query: String,
    input: Entity<TextInput>,
    scroll: UniformListScrollHandle,
    _load_task: Task<()>,
    filter_task: Option<Task<()>>,
    filtering: bool,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    drag: Option<(String, f32, f32)>,
}

impl FilesWindow {
    fn new(
        owner: Entity<OxydeApp>,
        operation_id: String,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        outcomes: std::collections::HashMap<PathBuf, oxyde_lib::systems::file_ops::ItemOutcome>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let total = sources.len();
        let owner_subscription = cx.observe(&owner, |_, _, cx| cx.notify());
        let weak = cx.entity().downgrade();
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |value, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.query = value.to_lowercase();
                        this.filter(cx);
                    });
                },
                |_, _| {},
                |window, cx| {
                    tool_window::close_transients(window, cx);
                    window.remove_window();
                },
                |_, _, _, _, _| {},
                TextInputOptions {
                    appearance: InputAppearance::Embedded,
                    clearable: true,
                    ..Default::default()
                },
                window,
            )
        });
        let focus = input.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        let load_task = cx.spawn(async move |this, cx| {
            let items = cx
                .background_executor()
                .spawn(async move {
                    Arc::new(
                        sources
                            .iter()
                            .map(|path| Item::new(path, destination.as_deref()))
                            .collect::<Vec<_>>(),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.items = items;
                this.filter(cx);
            });
        });
        Self {
            owner: owner.downgrade(),
            _owner_subscription: owner_subscription,
            operation_id,
            total,
            outcomes: Arc::new(outcomes),
            items: Arc::new(Vec::new()),
            filtered: Arc::new(Vec::new()),
            query: String::new(),
            input,
            scroll: UniformListScrollHandle::new(),
            _load_task: load_task,
            filter_task: None,
            filtering: true,
            scrollbar_origins: Default::default(),
            grab_offset: 0.0,
            drag: None,
        }
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        self.filtering = true;
        cx.notify();
        let items = self.items.clone();
        let query = self.query.clone();
        self.filter_task = Some(cx.spawn(async move |this, cx| {
            let filtered = cx
                .background_executor()
                .spawn(async move {
                    Arc::new(
                        items
                            .iter()
                            .enumerate()
                            .filter_map(|(index, item)| {
                                item.search.contains(&query).then_some(index)
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.filtered = filtered;
                this.filtering = false;
                this.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            });
        }));
    }
}

impl Render for FilesWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            window.remove_window();
            return div().into_any_element();
        };
        window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        let title = count_label(Message::OpsAffectedItems, self.total, cx);
        window.set_window_title(&title);
        let placeholder = cx.i18n().text(Message::OpsItemsFilter);
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        let theme = *cx.theme();
        let items = self.items.clone();
        let filtered = self.filtered.clone();
        if let Some(outcomes) = owner
            .read(cx)
            .file_ops
            .operation_item_outcomes(&self.operation_id)
        {
            self.outcomes = Arc::new(outcomes);
        }
        let outcomes = self.outcomes.clone();
        let system = owner.read(cx).use_system_icons();
        let scrollbar = self.render_scrollbar(cx);
        let can_top = self.scroll.0.borrow().base_handle.offset().y < px(0.0);
        let top_action = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            let mut scroll = this.scroll.0.borrow_mut();
            scroll.deferred_scroll_to_item = None;
            let offset = scroll.base_handle.offset();
            scroll.base_handle.set_offset(point(offset.x, px(0.0)));
            cx.notify();
        });
        let header_actions = div().flex().flex_none().mr(rems(0.375)).child(
            super::super::title_actions::scroll_top(can_top, cx, top_action),
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .text_color(theme.text)
            .tab_group()
            .child(tool_window::pointer_drag(cx.entity(),
                |this, event, cx| this.move_scrollbar(event, cx), |this, cx| {
                this.drag = None;
                cx.notify();
            }))
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .capture_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    tool_window::close_transients(window, cx);
                    window.remove_window();
                    cx.stop_propagation();
                } else if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(tool_window::header_with_actions(
                "icons/list.svg",
                title,
                window.is_maximized(),
                true,
                cx,
                header_actions,
                |window, cx| {
                    tool_window::close_transients(window, cx);
                    window.remove_window();
                },
            ))
            .child(
                div()
                    .p(rems(0.75))
                    .bg(theme.header)
                    .border_b(rems(0.0625))
                    .border_color(theme.border)
                    .child(
                        crate::ui::components::input::field_frame(&self.input, window, cx)
                            .child(
                                svg()
                                    .path("icons/search.svg")
                                    .size(rems(1.0))
                                    .ml(rems(0.625))
                                    .text_color(theme.text),
                            )
                            .child(div().flex_1().min_w_0().h_full().child(self.input.clone())),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .px(rems(0.75))
                    .when(self.filtering, |body| {
                        body.child(
                            div()
                                .p(rems(0.5))
                                .text_color(theme.text_muted)
                                .child(cx.i18n().text(Message::OpsItemsLoading)),
                        )
                    })
                    .when(!self.filtering && self.filtered.is_empty(), |body| {
                        body.child(
                            div()
                                .p(rems(0.5))
                                .text_color(theme.text_muted)
                                .child(cx.i18n().text(Message::OpsNoMatchingItems)),
                        )
                    })
                    .when(!self.filtering && !self.filtered.is_empty(), |body| {
                        body.child(
                            uniform_list("operation-items", filtered.len(), move |range, _, cx| {
                                range
                                    .map(|index| {
                                        let item = &items[filtered[index]];
                                        row(item, system, outcomes.get(&item.path).copied(), cx)
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .w_full()
                            .h_full()
                            .track_scroll(&self.scroll),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .bottom_0()
                            .w(rems(0.75))
                            .child(scrollbar),
                    ),
            )
            .child(
                tool_window::footer(cx).child(
                    tool_window::action_button(
                        "operation-items-close",
                        cx.i18n().text(Message::Close),
                        false,
                        cx,
                    )
                    .on_click(|_, window, cx| {
                        tool_window::close_transients(window, cx);
                        window.remove_window();
                    }),
                ),
            )
            .into_any_element()
    }
}

pub(super) fn close_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<FilesWindow>() {
            let _ = handle.update(cx, |_, window, cx| {
                tool_window::close_transients(window, cx);
                window.remove_window();
            });
        }
    }
}

pub(super) fn open(owner: Entity<OxydeApp>, operation_id: String, cx: &mut App) {
    cx.defer(move |cx| {
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<FilesWindow>() {
                let same = handle
                    .read(cx)
                    .is_ok_and(|view| view.operation_id == operation_id);
                if same {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                    return;
                }
            }
        }
        let Some(operation) = owner.read(cx).file_ops.get_operation(&operation_id) else {
            return;
        };
        let title = count_label(Message::OpsAffectedItems, operation.sources.len(), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(crate::ui::app::tool_geometry::restore("icons/list.svg", Bounds::new(
                    point(px(200.0), px(160.0)),
                    size(px(680.0), px(520.0)),
                ), cx)),
                window_min_size: Some(size(px(400.0), px(300.0))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    title: Some(title),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                tool_window::install_icon("icons/list.svg", window);
                cx.new(|cx| {
                    FilesWindow::new(
                        owner,
                        operation_id,
                        operation.sources,
                        operation.destination,
                        operation.item_outcomes,
                        window,
                        cx,
                    )
                })
            },
        ) {
            log::error!("Cannot open operation items: {error}");
        }
    });
}
