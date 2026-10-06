use super::{ContextMenuContent, ContextMenuInstance, OxydeApp};
use crate::ui::{
    components::{
        dropdown::DropdownItem,
        input::TextInput,
        modal::{action_button, form_row_with_label_width, Modal},
        scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis},
    },
    i18n::{I18nContext, Message},
    theme::ThemeContext,
};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::commands::search::AdvancedSearchOptions;

fn form_row(label: SharedString, value: impl IntoElement, cx: &App) -> impl IntoElement {
    form_row_with_label_width(label, value, rems(9.0), cx)
}

pub fn parse_advanced_path(path: &str) -> Option<(String, AdvancedSearchOptions)> {
    let (kind, root, options): (String, String, AdvancedSearchOptions) =
        serde_json::from_str(path.strip_prefix("search://")?).ok()?;
    (kind == "advanced").then_some((root, options))
}

fn size_in_bytes(value: &str, unit: usize) -> Result<Option<u64>, ()> {
    let value = value.trim().replace(',', ".");
    if value.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((&value, ""));
    let multiplier = 1024u128.checked_pow(unit as u32).ok_or(())?;
    let whole = whole
        .parse::<u128>()
        .map_err(|_| ())?
        .checked_mul(multiplier)
        .ok_or(())?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        let denominator = 10u128.checked_pow(fraction.len() as u32).ok_or(())?;
        fraction
            .parse::<u128>()
            .map_err(|_| ())?
            .checked_mul(multiplier)
            .ok_or(())?
            / denominator
    };
    whole
        .checked_add(fraction)
        .and_then(|v| u64::try_from(v).ok())
        .map(Some)
        .ok_or(())
}

pub struct AdvancedSearchDialog {
    inputs: Vec<Entity<TextInput>>,
    values: Vec<String>,
    flags: [bool; 6], // name regex, content regex, recursive, archives, case, accents
    unit: usize,
    help: bool,
    error: bool,
    focus: FocusHandle,
    pane: String,
    calendar: Option<(usize, chrono::NaiveDate)>,
    controls: Vec<FocusHandle>,
    help_scroll: ScrollHandle,
    help_metrics: (f32, f32),
    help_drag: Option<(String, f32, f32)>,
}

#[cfg(test)]
mod tests {
    use super::size_in_bytes;

    #[test]
    fn size_bounds_accept_decimal_units_and_reject_overflow() {
        assert_eq!(size_in_bytes("1,5", 1), Ok(Some(1536)));
        assert_eq!(size_in_bytes("1.5", 2), Ok(Some(1572864)));
        assert_eq!(size_in_bytes("", 1), Ok(None));
        assert!(size_in_bytes("-1", 1).is_err());
        assert!(size_in_bytes("18446744073709551615", 1).is_err());
    }
}

pub struct AdvancedSearchWindow {
    owner: WeakEntity<OxydeApp>,
    advanced_search: Option<AdvancedSearchDialog>,
    context_menus: Vec<ContextMenuInstance>,
    handle: WeakEntity<Self>,
    scrollbar_origins: std::collections::HashMap<String, Point<f32>>,
    grab_offset: f32,
    _subscriptions: Vec<Subscription>,
}

impl crate::ui::components::scrollbar::ScrollbarState for AdvancedSearchWindow {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.grab_offset = offset;
    }
}

pub(crate) fn close_advanced_search_windows(cx: &mut App) {
    for handle in cx.windows() {
        if let Some(handle) = handle.downcast::<AdvancedSearchWindow>() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}

impl OxydeApp {
    pub(crate) fn open_advanced_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.context_menus.clear();
        self.settings_open = false;
        for handle in cx.windows() {
            if let Some(handle) = handle.downcast::<AdvancedSearchWindow>() {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                });
                cx.notify();
                return;
            }
        }
        let pane = self.state.active_pane_id.clone();
        let Some(tab) = self.state.find_pane(&pane).map(|p| p.active_tab()) else {
            return;
        };
        let (root, old) = super::search::parse_search_path(&tab.path)
            .unwrap_or((tab.path.clone(), String::new()));
        let options = parse_advanced_path(&tab.path)
            .map(|(_, options)| options)
            .unwrap_or(AdvancedSearchOptions {
                query: old,
                recursive: true,
                ..Default::default()
            });
        let owner = cx.entity().downgrade();
        let title = cx.i18n().text(Message::ToolSearch);
        // A new window draws synchronously; defer until the owner update ends.
        cx.defer(move |cx| {
            if owner.upgrade().is_none() {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(160.0), px(140.0)),
                        size(px(760.0), px(520.0)),
                    ))),
                    is_resizable: false,
                    is_minimizable: false,
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| AdvancedSearchWindow::new(owner, pane, root, options, window, cx))
                },
            );
            if let Err(error) = result {
                log::error!("Cannot open advanced search window: {error}");
            }
        });
        cx.notify();
    }
}

impl AdvancedSearchWindow {
    fn drag_advanced_help(&mut self, position: Point<Pixels>, cx: &mut App) {
        let Some(d) = &mut self.advanced_search else {
            return;
        };
        let Some((track, viewport, content)) = &d.help_drag else {
            return;
        };
        let Some(origin) = self.scrollbar_origins.get(track) else {
            return;
        };
        let thumb =
            (viewport * viewport / content).max(crate::ui::scale::rem_pixels(rems(2.5)).as_f32());
        let travel = (viewport - thumb).max(1.0);
        let offset = ((position.y.as_f32() - origin.y - self.grab_offset) / travel).clamp(0.0, 1.0)
            * (content - viewport);
        d.help_scroll.set_offset(point(px(0.0), px(-offset)));
        if let Some(handle) = self.handle.upgrade() {
            cx.notify(handle.entity_id());
        }
    }
    fn new(
        owner: WeakEntity<OxydeApp>,
        pane: String,
        root: String,
        options: AdvancedSearchOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let unit = if options
            .min_size
            .into_iter()
            .chain(options.max_size)
            .any(|v| v % 1024 != 0)
        {
            0
        } else {
            1
        };
        let divisor = 1024u64.pow(unit as u32);
        let date = |v: Option<u64>| {
            v.and_then(|v| chrono::DateTime::from_timestamp_millis(v as i64))
                .map(|v| {
                    v.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_default()
        };
        let values = vec![
            options.query.clone(),
            root,
            options.content.clone(),
            options
                .min_size
                .map(|v| (v / divisor).to_string())
                .unwrap_or_default(),
            options
                .max_size
                .map(|v| (v / divisor).to_string())
                .unwrap_or_default(),
            date(options.min_date),
            date(options.max_date),
        ];
        let mut inputs = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let change = cx.entity().downgrade();
            let enter = cx.entity().downgrade();
            let escape = cx.entity().downgrade();
            let menu = cx.entity().downgrade();
            inputs.push(cx.new(|cx| {
                TextInput::new(
                    cx,
                    "",
                    value.clone(),
                    move |value, _, cx| {
                        let _ = change.update(cx, |this, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                d.values[index] = value.into();
                                d.error = false;
                            }
                            cx.notify();
                        });
                    },
                    move |window, cx| {
                        let _ =
                            enter.update(cx, |this, cx| this.submit_advanced_search(window, cx));
                    },
                    move |window, cx| {
                        let _ =
                            escape.update(cx, |this, cx| this.close_advanced_search(window, cx));
                    },
                    move |input, selected, position, window, cx| {
                        let _ = menu.update(cx, |this, cx| {
                            this.open_text_context_menu(input, selected, position, window, cx)
                        });
                    },
                    false,
                    true,
                    false,
                    None,
                    window,
                )
            }));
        }
        let focus = inputs[0].read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        let advanced_search = Some(AdvancedSearchDialog {
            inputs,
            values,
            flags: [
                options.regex,
                options.content_regex,
                options.recursive,
                options.archives,
                options.case_sensitive,
                options.ignore_accents,
            ],
            unit,
            help: false,
            error: false,
            focus: cx.focus_handle(),
            pane,
            calendar: None,
            controls: (0..14).map(|_| cx.focus_handle()).collect(),
            help_scroll: ScrollHandle::new(),
            help_metrics: (0.0, 0.0),
            help_drag: None,
        });
        crate::ui::components::tool_window::install_icon("icons/search.svg", window);
        let mut subscriptions = Vec::new();
        if let Some(main) = owner.upgrade() {
            subscriptions.push(
                cx.observe_release_in(&main, window, |_, _, window, _| window.remove_window()),
            );
        }
        Self {
            owner,
            advanced_search,
            context_menus: Vec::new(),
            handle: cx.entity().downgrade(),
            scrollbar_origins: std::collections::HashMap::new(),
            grab_offset: 0.0,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn open_text_context_menu(
        &mut self,
        input: Entity<TextInput>,
        has_selection: bool,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::ui::components::dropdown::{DropdownItem, IconName};

        let Some(handle) = self.handle.upgrade() else {
            return;
        };
        let can_paste = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.text())
            .is_some();

        let mut items = Vec::new();
        for (label, command, is_dimmed) in [
            (Message::Cut, 0u8, !has_selection),
            (Message::Copy, 1u8, !has_selection),
            (Message::Paste, 2u8, !can_paste),
            (Message::SelectAll, 3u8, false),
        ] {
            if command == 3 {
                items.push(DropdownItem::separator());
            }
            let h = handle.downgrade();
            let target_input = input.clone();
            let mut item =
                DropdownItem::new(cx.i18n().text(label).to_string(), move |window, app| {
                    target_input.update(app, |input, cx| match command {
                        0 => input.cut_selected_text(window, cx),
                        1 => input.copy_selected_text(cx),
                        2 => input.paste_from_clipboard(window, cx),
                        _ => input.select_all_text(cx),
                    });
                    let _ = h.update(app, |_this, cx| {
                        cx.notify();
                    });
                });
            item.icon = match command {
                1 => Some(IconName::Copy),
                3 => Some(IconName::ALargeSmall),
                _ => None,
            };
            item.is_dimmed = is_dimmed;
            items.push(item);
        }

        self.context_menus.clear();
        self.context_menus.push(ContextMenuInstance {
            position,
            content: ContextMenuContent::Items(items, None),
            scroll_handle: UniformListScrollHandle::new(),
            width: px(0.0),
            bottom_padding: 0.0,
            open_left: false,
            hovered_item: None,
            native_request_id: None,
        });
        cx.notify();
    }

    fn close_advanced_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.context_menus.is_empty() {
            self.context_menus.clear();
        } else if self
            .advanced_search
            .as_mut()
            .is_some_and(|d| d.calendar.take().is_some())
        {
            // Escape dismisses the date picker before closing the tool window.
        } else {
            window.remove_window();
        }
        cx.notify();
    }

    fn submit_advanced_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.advanced_search else {
            return;
        };
        let parse = || -> Result<AdvancedSearchOptions, ()> {
            let size = |index: usize| -> Result<Option<u64>, ()> {
                size_in_bytes(&d.values[index], d.unit)
            };
            let date = |index: usize| -> Result<Option<u64>, ()> {
                let v = d.values[index].trim();
                if v.is_empty() {
                    return Ok(None);
                }
                let day = chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").map_err(|_| ())?;
                let time = day
                    .and_hms_opt(
                        if index == 6 { 23 } else { 0 },
                        if index == 6 { 59 } else { 0 },
                        if index == 6 { 59 } else { 0 },
                    )
                    .ok_or(())?;
                use chrono::TimeZone;
                let timestamp = chrono::Local
                    .from_local_datetime(&time)
                    .earliest()
                    .ok_or(())?
                    .timestamp_millis();
                u64::try_from(timestamp + if index == 6 { 999 } else { 0 })
                    .map(Some)
                    .map_err(|_| ())
            };
            let o = AdvancedSearchOptions {
                query: d.values[0].trim().into(),
                content: d.values[2].trim().into(),
                regex: d.flags[0],
                content_regex: d.flags[1],
                recursive: d.flags[2],
                archives: d.flags[3],
                case_sensitive: d.flags[4],
                ignore_accents: d.flags[5],
                min_size: size(3)?,
                max_size: size(4)?,
                min_date: date(5)?,
                max_date: date(6)?,
            };
            o.validate().map_err(|_| ())?;
            if d.values[1].trim().is_empty() {
                return Err(());
            }
            Ok(o)
        };
        let Ok(options) = parse() else {
            self.advanced_search.as_mut().unwrap().error = true;
            cx.notify();
            return;
        };
        let pane = d.pane.clone();
        let root = d.values[1].trim().to_string();
        let path = format!(
            "search://{}",
            serde_json::to_string(&("advanced", root, options)).expect("search options")
        );
        self.advanced_search.as_mut().unwrap().error = false;
        cx.notify();
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |owner, cx| {
                owner.navigate_to(&pane, path, cx);
                for handle in cx.windows() {
                    if let Some(handle) = handle.downcast::<OxydeApp>() {
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, _| window.activate_window());
                        });
                    }
                }
            });
        });
    }

    pub(crate) fn render_advanced_search(
        &self,
        handle: Entity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = self.advanced_search.as_ref().unwrap();
        let theme = *cx.theme();
        let units = match cx.i18n().language() {
            crate::ui::i18n::Language::French => ["o", "Ko", "Mo", "Go", "To"],
            _ => ["B", "KB", "MB", "GB", "TB"],
        };
        let mut body = div()
            .flex()
            .flex_col()
            .gap(rems(0.75))
            .when(d.help, |body| body.h_full().min_h_0());
        let mut tabs = div()
            .flex()
            .flex_none()
            .items_center()
            .px(rems(1.25))
            .bg(theme.dialog_tabs)
            .gap(rems(0.25))
            .border_b(rems(0.0625))
            .border_color(theme.border);
        for (help, message) in [
            (false, Message::AdvancedSettings),
            (true, Message::AdvancedHelp),
        ] {
            let h = handle.clone();
            tabs = tabs.child(
                div().id(if help { "advanced-help" } else { "advanced-settings" })
                .px(rems(1.0))
                .py(rems(0.75))
                .text_size(rems(0.8125))
                .font_weight(FontWeight::MEDIUM)
                .flex()
                .items_center()
                .justify_center()
                .when(d.help != help, |tab| tab.hover(move |style| style.bg(theme.hover).text_color(theme.text)))
                .gap(rems(0.5))
                .child(
                    svg()
                        .path(if help {
                            "icons/regex.svg"
                        } else {
                            "icons/target.svg"
                        })
                        .size(rems(0.875))
                        .text_color(if d.help == help {
                            theme.accent
                        } else {
                            theme.text_muted
                        }),
                )
                .track_focus(&d.controls[if help { 7 } else { 6 }])
                .rounded_none()
                .border_b(rems(0.0625))
                .border_color(if d.help == help {
                    theme.accent
                } else {
                    transparent_black()
                })
                .text_color(if d.help == help {
                    theme.accent
                } else {
                    theme.text_muted
                })
                .child(cx.i18n().text(message))
                .on_click(move |_, _, cx| {
                    h.update(cx, |this, cx| {
                        if let Some(d) = &mut this.advanced_search {
                            d.help = help;
                        }
                        cx.notify();
                    });
                }),
            );
        }
        if d.help {
            let patterns = [
                ".", "\\d", "\\w", "^", "$", "+", "?", "|", "[abc]", "[^abc]", "()", "{n}",
            ];
            let labels = [
                Message::AdvancedRegexAnyChar,
                Message::AdvancedRegexDigit,
                Message::AdvancedRegexWordChar,
                Message::AdvancedRegexStartLine,
                Message::AdvancedRegexEndLine,
                Message::AdvancedRegexOneOrMore,
                Message::AdvancedRegexOptional,
                Message::AdvancedRegexEitherOr,
                Message::AdvancedRegexRange,
                Message::AdvancedRegexNegatedSet,
                Message::AdvancedRegexGroup,
                Message::AdvancedRegexCountExact,
            ];
            let descriptions = [
                Message::AdvancedRegexAnyCharDesc,
                Message::AdvancedRegexDigitDesc,
                Message::AdvancedRegexWordCharDesc,
                Message::AdvancedRegexStartLineDesc,
                Message::AdvancedRegexEndLineDesc,
                Message::AdvancedRegexOneOrMoreDesc,
                Message::AdvancedRegexOptionalDesc,
                Message::AdvancedRegexEitherOrDesc,
                Message::AdvancedRegexRangeDesc,
                Message::AdvancedRegexNegatedSetDesc,
                Message::AdvancedRegexGroupDesc,
                Message::AdvancedRegexCountExactDesc,
            ];
            body = body.child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(rems(0.5))
                            .child(
                                svg()
                                    .path("icons/code.svg")
                                    .size(rems(0.875))
                                    .text_color(theme.text_muted),
                            )
                            .child(cx.i18n().text(Message::AdvancedRegexHelpTitle)),
                    )
                    .child(
                        div()
                            .id("regex-link")
                            .flex()
                            .items_center()
                            .gap(rems(0.375))
                            .text_color(theme.accent)
                            .child("regex101.com")
                            .child(
                                svg()
                                    .path("icons/external-link.svg")
                                    .size(rems(0.625))
                                    .text_color(theme.accent),
                            )
                            .on_click(|_, _, cx| {
                                cx.background_spawn(async {
                                    let _ = oxyde_lib::platform::platform()
                                        .open_item("https://regex101.com");
                                })
                                .detach();
                            }),
                    ),
            );
            let refresh = handle.clone();
            let mut rows = div()
                .id("advanced-help-scroll")
                .h_full()
                .w_full()
                .pr(rems(0.75))
                .track_scroll(&d.help_scroll)
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .on_scroll_wheel(move |_, _, cx| refresh.update(cx, |_, cx| cx.notify()));
            for i in 0..patterns.len() {
                rows = rows.child(
                    div()
                        .flex()
                        .flex_none()
                        .w_full()
                        .min_w_0()
                        .items_center()
                        .gap(rems(0.75))
                        .py(rems(0.625))
                        .border_b(rems(0.0625))
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(rems(4.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(theme.header)
                                .border(rems(0.0625))
                                .border_color(theme.border)
                                .py(rems(0.25))
                                .text_color(theme.accent)
                                .child(patterns[i]),
                        )
                        .child(
                            div()
                                .w(rems(10.0))
                                .flex_none()
                                .child(cx.i18n().text(labels[i])),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .text_color(theme.text_muted)
                                .child(cx.i18n().text(descriptions[i])),
                        ),
                );
            }
            let measure = handle.clone();
            let mouse = handle.clone();
            let viewport = d.help_metrics.0;
            let content = d.help_metrics.1;
            body = body.child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .min_w_0()
                    .child(rows)
                    .child(render_scrollbar_with_pointer(
                        theme,
                        handle.clone(),
                        ScrollbarAxis::Vertical,
                        viewport,
                        content,
                        d.help_scroll.offset().y.as_f32().abs(),
                        d.help_drag.is_some(),
                        "advanced-help".into(),
                        None,
                        |this, track, viewport, content, position, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                d.help_drag = Some((track, viewport, content));
                            }
                            this.drag_advanced_help(position, cx);
                        },
                    ))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, cx| {
                                measure.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.advanced_search {
                                        let viewport = d.help_scroll.bounds().size.height.as_f32();
                                        let metrics = (
                                            viewport,
                                            viewport + d.help_scroll.max_offset().y.as_f32(),
                                        );
                                        if d.help_metrics != metrics {
                                            d.help_metrics = metrics;
                                            cx.notify();
                                        }
                                    }
                                });
                                let movement = mouse.clone();
                                window.on_mouse_event(
                                    move |event: &MouseMoveEvent, phase, _, cx| {
                                        if phase == DispatchPhase::Capture {
                                            movement.update(cx, |this, cx| {
                                                if this
                                                    .advanced_search
                                                    .as_ref()
                                                    .is_some_and(|d| d.help_drag.is_some())
                                                {
                                                    if event.dragging() {
                                                        this.drag_advanced_help(event.position, cx);
                                                    } else if let Some(d) =
                                                        &mut this.advanced_search
                                                    {
                                                        d.help_drag = None;
                                                        cx.notify();
                                                    }
                                                }
                                            });
                                        }
                                    },
                                );
                                let release = mouse.clone();
                                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                                    if phase == DispatchPhase::Capture
                                        && event.button == MouseButton::Left
                                    {
                                        release.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.advanced_search {
                                                if d.help_drag.take().is_some() {
                                                    cx.notify();
                                                }
                                            }
                                        });
                                    }
                                });
                            },
                        )
                        .absolute()
                        .size_0(),
                    ),
            );
        } else {
            for (index, label, placeholder) in [
                (
                    0,
                    Message::AdvancedSearchFor,
                    Message::AdvancedSearchForPlaceholder,
                ),
                (1, Message::AdvancedSearchIn, Message::PathPlaceholder),
                (
                    2,
                    Message::AdvancedFindText,
                    Message::AdvancedFindTextPlaceholder,
                ),
            ] {
                d.inputs[index].update(cx, |input, cx| {
                    input.set_placeholder(cx.i18n().text(placeholder), cx)
                });
                let mut field = div()
                    .flex()
                    .items_center()
                    .gap(rems(0.0))
                    .h(rems(2.25))
                    .rounded_md()
                    .overflow_hidden()
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.background);
                if index != 1 {
                    field = field.child(self.advanced_toggle(
                        if index == 0 { 0 } else { 1 },
                        Message::AdvancedUseRegex,
                        true,
                        handle.clone(),
                        cx,
                    ));
                } else {
                    field = field.child(
                        svg()
                            .path("icons/folder.svg")
                            .size(rems(1.0))
                            .ml(rems(0.625))
                            .text_color(theme.text_muted),
                    );
                }
                let regex_enabled = match index {
                    0 => d.flags[0],
                    2 => d.flags[1],
                    _ => false,
                };
                field = field.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .font_family(if regex_enabled {
                            "JetBrains Mono"
                        } else {
                            "Inter"
                        })
                        .child(d.inputs[index].clone()),
                );
                body = body.child(form_row(cx.i18n().text(label), field, cx));
                if index == 1 {
                    body = body.child(form_row(
                        "".into(),
                        div()
                            .flex()
                            .gap(rems(0.75))
                            .child(self.advanced_toggle(
                                2,
                                Message::AdvancedSearchSubfolders,
                                false,
                                handle.clone(),
                                cx,
                            ))
                            .child(self.advanced_toggle(
                                3,
                                Message::AdvancedSearchInArchives,
                                false,
                                handle.clone(),
                                cx,
                            )),
                        cx,
                    ));
                }
            }
            let unit_handle = handle.clone();
            let mut unit = Some(
                action_button("advanced-unit")
                    .h(rems(2.25))
                    .min_w(rems(5.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(rems(0.5))
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.background)
                    .track_focus(&d.controls[8])
                    .child(units[d.unit])
                    .child(
                        svg()
                            .path("icons/chevron-down.svg")
                            .size(rems(0.75))
                            .text_color(theme.text_muted),
                    )
                    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                        unit_handle.update(cx, |this, cx| {
                            let items = units
                                .into_iter()
                                .enumerate()
                                .map(|(index, label)| {
                                    let h = unit_handle.downgrade();
                                    DropdownItem::new(label, move |_, cx| {
                                        let _ = h.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.advanced_search {
                                                d.unit = index;
                                            }
                                            cx.notify();
                                        });
                                    })
                                })
                                .collect();
                            this.context_menus.clear();
                            this.context_menus.push(ContextMenuInstance {
                                position: event.position,
                                content: ContextMenuContent::Items(items, None),
                                scroll_handle: UniformListScrollHandle::new(),
                                width: px(0.0),
                                bottom_padding: 0.0,
                                open_left: false,
                                hovered_item: None,
                                native_request_id: None,
                            });
                            cx.notify();
                        });
                    }),
            );
            for (start, label) in [(3, Message::AdvancedSize), (5, Message::AdvancedDate)] {
                for (index, placeholder) in [
                    (start, Message::AdvancedMin),
                    (start + 1, Message::AdvancedMax),
                ] {
                    d.inputs[index].update(cx, |input, cx| {
                        input.set_placeholder(
                            if start == 5 {
                                cx.i18n().text(Message::AdvancedDatePlaceholder)
                            } else {
                                cx.i18n().text(placeholder)
                            },
                            cx,
                        )
                    });
                }
                let mut fields = div().flex().items_center().gap(rems(0.5));
                for index in start..=start + 1 {
                    if index != start {
                        fields = fields.child("—");
                    }
                    let mut field = div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .h(rems(2.25))
                        .rounded_md()
                        .border(rems(0.0625))
                        .border_color(theme.border)
                        .bg(theme.background)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(d.inputs[index].clone()),
                        );
                    if start == 5 {
                        let h = handle.clone();
                        field = field.child(
                            div()
                                .id(SharedString::from(format!("advanced-calendar-{index}")))
                                .size(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .track_focus(&d.controls[if index == 5 { 9 } else { 10 }])
                                .child(
                                    svg()
                                        .path("icons/calendar-days.svg")
                                        .size(rems(1.0))
                                        .text_color(theme.text_muted),
                                )
                                .on_click(move |_, window, cx| {
                                    h.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.advanced_search {
                                            d.calendar = Some((
                                                index,
                                                chrono::NaiveDate::parse_from_str(
                                                    &d.values[index],
                                                    "%Y-%m-%d",
                                                )
                                                .unwrap_or_else(|_| {
                                                    chrono::Local::now().date_naive()
                                                }),
                                            ));
                                            window.focus(&d.focus, cx);
                                        }
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    fields = fields.child(field);
                }
                if start == 3 {
                    fields = fields.child(unit.take().unwrap());
                }
                body = body.child(form_row(cx.i18n().text(label), fields, cx));
            }
            body = body.child(form_row(
                cx.i18n().text(Message::AdvancedGlobalOptions),
                div()
                    .flex()
                    .gap(rems(0.75))
                    .child(self.advanced_toggle(
                        4,
                        Message::AdvancedCaseSensitive,
                        false,
                        handle.clone(),
                        cx,
                    ))
                    .child(self.advanced_toggle(
                        5,
                        Message::AdvancedIgnoreAccents,
                        false,
                        handle.clone(),
                        cx,
                    )),
                cx,
            ));
        }
        if d.error {
            body = body.child(
                div()
                    .text_color(theme.error)
                    .child(cx.i18n().text(Message::AdvancedInvalid)),
            );
        }
        let reset = handle.clone();
        let cancel = handle.clone();
        let submit = handle.clone();
        let keyboard = handle.clone();
        let frame = div()
            .relative()
            .w_full()
            .when(d.help, |frame| frame.h_full())
            .when(!d.help, |frame| {
                frame.child(
                    canvas(
                        |bounds, _, _| bounds,
                        |bounds, _, window, _| {
                            // Fit the native content size to the form, not a saved oversized viewport.
                            if (window.viewport_size().height - bounds.size.height).abs() > px(1.0)
                            {
                                window.on_next_frame(move |window, _| window.resize(bounds.size));
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
            .flex()
            .flex_col()
            .child(crate::ui::components::tool_window::header(
                "icons/search.svg",
                cx.i18n().text(Message::ToolSearch),
                window.is_maximized(),
                false,
                cx,
                |window, _| window.remove_window(),
            ))
            .child(tabs)
            .child(
                div()
                    .id("advanced-form-scroll")
                    .when(d.help, |body| body.flex_1().min_h_0())
                    .when(!d.help, |body| body.flex_none())
                    .overflow_y_scroll()
                    .px(rems(1.0))
                    .py(rems(1.25))
                    .child(body),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(rems(0.625))
                    .border_t(rems(0.0625))
                    .border_color(theme.border)
                    .p(rems(0.625))
                    .child(
                        div().flex_1().flex().child(
                            crate::ui::components::tool_window::action_button(
                                "advanced-reset",
                                "".into(),
                                false,
                                cx,
                            )
                            .track_focus(&d.controls[11])
                            .child(cx.i18n().text(Message::AdvancedReset))
                            .on_click(move |_, window, cx| {
                                reset.update(cx, |this, cx| {
                                    let Some(d) = &mut this.advanced_search else {
                                        return;
                                    };
                                    d.flags = [false, false, true, false, false, false];
                                    d.unit = 1;
                                    d.error = false;
                                    for i in [0, 2, 3, 4, 5, 6] {
                                        d.values[i].clear();
                                        d.inputs[i].update(cx, |input, cx| {
                                            input.set_content(String::new(), cx)
                                        });
                                    }
                                    let focus = d.inputs[0].read(cx).focus_handle.clone();
                                    window.focus(&focus, cx);
                                    cx.notify();
                                });
                            }),
                        ),
                    )
                    .child(
                        crate::ui::components::tool_window::action_button(
                            "advanced-cancel",
                            "".into(),
                            false,
                            cx,
                        )
                        .track_focus(&d.controls[12])
                        .child(cx.i18n().text(Message::Cancel))
                        .on_click(move |_, window, cx| {
                            cancel.update(cx, |this, cx| this.close_advanced_search(window, cx));
                        }),
                    )
                    .child(
                        crate::ui::components::tool_window::action_button(
                            "advanced-submit",
                            "".into(),
                            true,
                            cx,
                        )
                        .track_focus(&d.controls[13])
                        .child(
                            svg()
                                .path("icons/search.svg")
                                .size(rems(0.875))
                                .text_color(theme.text_on_accent),
                        )
                        .child(cx.i18n().text(Message::AdvancedSearch))
                        .on_click(move |_, window, cx| {
                            submit.update(cx, |this, cx| this.submit_advanced_search(window, cx));
                        }),
                    ),
            );
        let mut layer = div()
            .relative()
            .size_full()
            .track_focus(&d.focus)
            // Block file actions at the modal boundary while allowing TextInput
            // actions and unhandled keystrokes to reach its text input handler.
            .on_action(|_: &super::Ouvrir, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Copier, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Couper, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Coller, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Renommer, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Supprimer, _, cx| cx.stop_propagation())
            .on_action(|_: &super::ToutSelectionner, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Annuler, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Refaire, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Actualiser, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                keyboard.update(cx, |this, cx| {
                    if event.keystroke.key == "escape" && !this.context_menus.is_empty() {
                        this.context_menus.clear();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if let Some(d) = &mut this.advanced_search {
                        if let Some((index, date)) = d.calendar {
                            let delta = match event.keystroke.key.as_str() {
                                "left" => -1,
                                "right" => 1,
                                "up" => -7,
                                "down" => 7,
                                _ => 0,
                            };
                            if delta != 0 {
                                d.calendar = Some((index, date + chrono::Duration::days(delta)));
                            } else if event.keystroke.key == "enter" {
                                let value = date.format("%Y-%m-%d").to_string();
                                d.values[index] = value.clone();
                                d.inputs[index]
                                    .update(cx, |input, cx| input.set_content(value, cx));
                                d.calendar = None;
                            } else if event.keystroke.key == "escape" {
                                d.calendar = None;
                            }
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                    }
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            if this
                                .advanced_search
                                .as_ref()
                                .is_some_and(|d| d.calendar.is_some())
                            {
                                this.advanced_search.as_mut().unwrap().calendar = None;
                                cx.notify();
                            } else {
                                this.close_advanced_search(window, cx);
                            }
                        }
                        "space" | "enter"
                            if this.advanced_search.as_ref().is_some_and(|d| {
                                d.controls.iter().any(|f| f.is_focused(window))
                            }) =>
                        {
                            let d = this.advanced_search.as_mut().unwrap();
                            let control = d
                                .controls
                                .iter()
                                .position(|f| f.is_focused(window))
                                .unwrap();
                            match control {
                                0..=5 => d.flags[control] = !d.flags[control],
                                6 | 7 => d.help = control == 7,
                                8 => d.unit = (d.unit + 1) % 5,
                                9 | 10 => {
                                    d.calendar =
                                        Some((control - 4, chrono::Local::now().date_naive()))
                                }
                                11 => {
                                    d.flags = [false, false, true, false, false, false];
                                    d.unit = 1;
                                    d.error = false;
                                    for i in [0, 2, 3, 4, 5, 6] {
                                        d.values[i].clear();
                                        d.inputs[i].update(cx, |input, cx| {
                                            input.set_content(String::new(), cx)
                                        });
                                    }
                                }
                                12 => this.close_advanced_search(window, cx),
                                13 => this.submit_advanced_search(window, cx),
                                _ => {}
                            }
                            cx.notify();
                        }
                        "enter" => this.submit_advanced_search(window, cx),
                        "tab" => {
                            if let Some(d) = &this.advanced_search {
                                let mut order = vec![d.controls[6].clone(), d.controls[7].clone()];
                                if !d.help {
                                    order.extend([
                                        d.controls[0].clone(),
                                        d.inputs[0].read(cx).focus_handle.clone(),
                                        d.inputs[1].read(cx).focus_handle.clone(),
                                        d.controls[2].clone(),
                                        d.controls[3].clone(),
                                        d.controls[1].clone(),
                                        d.inputs[2].read(cx).focus_handle.clone(),
                                        d.inputs[3].read(cx).focus_handle.clone(),
                                        d.inputs[4].read(cx).focus_handle.clone(),
                                        d.controls[8].clone(),
                                        d.inputs[5].read(cx).focus_handle.clone(),
                                        d.controls[9].clone(),
                                        d.inputs[6].read(cx).focus_handle.clone(),
                                        d.controls[10].clone(),
                                        d.controls[4].clone(),
                                        d.controls[5].clone(),
                                    ]);
                                }
                                order.extend(d.controls[11..].iter().cloned());
                                let current = order.iter().position(|f| f.is_focused(window));
                                let next = if event.keystroke.modifiers.shift {
                                    current.map_or(order.len() - 1, |i| {
                                        (i + order.len() - 1) % order.len()
                                    })
                                } else {
                                    current.map_or(0, |i| (i + 1) % order.len())
                                };
                                window.focus(&order[next], cx);
                            }
                        }
                        _ => return,
                    }
                    cx.stop_propagation();
                });
            })
            .child(frame);
        if let Some((index, month)) = d.calendar {
            use chrono::Datelike;
            let first = month.with_day(1).unwrap();
            let mut calendar = div().flex().flex_col().gap(rems(0.5));
            let mut navigation = div().flex().items_center().justify_between();
            for delta in [-1, 1] {
                let h = handle.clone();
                navigation = navigation.child(
                    action_button(if delta < 0 {
                        "calendar-prev"
                    } else {
                        "calendar-next"
                    })
                    .child(
                        svg()
                            .path(if delta < 0 {
                                "icons/chevron-left.svg"
                            } else {
                                "icons/chevron-right.svg"
                            })
                            .size(rems(1.0))
                            .text_color(theme.text_muted),
                    )
                    .on_click(move |_, _, cx| {
                        h.update(cx, |this, cx| {
                            if let Some(d) = &mut this.advanced_search {
                                if let Some((_, date)) = &mut d.calendar {
                                    let months = chrono::Months::new(1);
                                    *date = if delta < 0 {
                                        date.checked_sub_months(months)
                                    } else {
                                        date.checked_add_months(months)
                                    }
                                    .unwrap_or(*date);
                                }
                            }
                            cx.notify();
                        });
                    }),
                );
                if delta < 0 {
                    navigation = navigation.child(month.format("%Y-%m").to_string());
                }
            }
            calendar = calendar.child(navigation);
            let mut weekdays = div().flex().gap(rems(0.125));
            for day in cx.i18n().text(Message::AdvancedWeekdays).split_whitespace() {
                weekdays = weekdays.child(
                    div()
                        .w(rems(2.1))
                        .text_color(theme.text_muted)
                        .child(day.to_string()),
                );
            }
            calendar = calendar.child(weekdays);
            let offset = first.weekday().num_days_from_monday() as i64;
            for week in 0..6 {
                let mut row = div().flex().gap(rems(0.125));
                for day in 0..7 {
                    let date = first + chrono::Duration::days(week * 7 + day - offset);
                    let h = handle.clone();
                    row = row.child(
                        action_button(SharedString::from(format!("calendar-day-{date}")))
                            .w(rems(2.1))
                            .px(rems(0.25))
                            .text_color(if date.month() == month.month() {
                                theme.text
                            } else {
                                theme.text_muted
                            })
                            .bg(if date == month {
                                theme.selected
                            } else {
                                theme.surface
                            })
                            .hover(move |s| s.bg(theme.control_hover))
                            .child(date.day().to_string())
                            .on_click(move |_, _, cx| {
                                h.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.advanced_search {
                                        let value = date.format("%Y-%m-%d").to_string();
                                        d.values[index] = value.clone();
                                        d.inputs[index]
                                            .update(cx, |input, cx| input.set_content(value, cx));
                                        d.calendar = None;
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                }
                calendar = calendar.child(row);
            }
            let h = handle.clone();
            layer = layer.child(
                Modal::new(
                    "advanced-date-picker",
                    cx.i18n().text(Message::AdvancedDate),
                    calendar,
                )
                .title_icon("icons/calendar-days.svg")
                .width(rems(18.0))
                .action(
                    action_button("calendar-cancel")
                        .child(cx.i18n().text(Message::Cancel))
                        .on_click(move |_, _, cx| {
                            h.update(cx, |this, cx| {
                                if let Some(d) = &mut this.advanced_search {
                                    d.calendar = None;
                                }
                                cx.notify();
                            });
                        }),
                ),
            );
        }
        layer.into_any_element()
    }

    fn advanced_toggle(
        &self,
        index: usize,
        message: Message,
        regex: bool,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.theme();
        let checked = self.advanced_search.as_ref().unwrap().flags[index];
        if regex {
            return div()
                .id(SharedString::from(format!("advanced-flag-{index}")))
                .track_focus(&self.advanced_search.as_ref().unwrap().controls[index])
                .flex_none()
                .w(rems(2.375))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(if checked { theme.accent } else { theme.header })
                .border_r(rems(0.0625))
                .border_color(theme.border)
                .focus(move |style| style.border_color(theme.accent))
                .child(
                    svg()
                        .path("icons/regex.svg")
                        .size(rems(0.875))
                        .text_color(if checked {
                            theme.text_on_accent
                        } else {
                            theme.text_muted
                        }),
                )
                .tooltip(crate::ui::components::tooltip::Tooltip::text(
                    cx.i18n().text(message),
                ))
                .on_click(move |_, _, cx| {
                    handle.update(cx, |this, cx| {
                        if let Some(d) = &mut this.advanced_search {
                            d.flags[index] = !d.flags[index];
                        }
                        cx.notify();
                    });
                })
                .into_any_element();
        }
        div()
            .id(SharedString::from(format!("advanced-flag-{index}")))
            .flex()
            .items_center()
            .gap(rems(0.375))
            .track_focus(&self.advanced_search.as_ref().unwrap().controls[index])
            .focus(move |style| style.bg(theme.control_hover))
            .child(
                div()
                    .size(rems(if regex { 1.75 } else { 1.0 }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(if checked { theme.accent } else { theme.header })
                    .text_color(if checked {
                        theme.text_on_accent
                    } else {
                        theme.text_muted
                    })
                    .when(regex || checked, |e| {
                        e.child(
                            svg()
                                .path(if regex {
                                    "icons/regex.svg"
                                } else {
                                    "icons/check.svg"
                                })
                                .size(rems(if regex { 0.875 } else { 0.75 }))
                                .text_color(if checked {
                                    theme.text_on_accent
                                } else {
                                    theme.text_muted
                                }),
                        )
                    }),
            )
            .when(!regex, |e| e.child(cx.i18n().text(message)))
            .tooltip(crate::ui::components::tooltip::Tooltip::text(
                cx.i18n().text(message),
            ))
            .on_click(move |_, _, cx| {
                handle.update(cx, |this, cx| {
                    if let Some(d) = &mut this.advanced_search {
                        d.flags[index] = !d.flags[index];
                    }
                    cx.notify();
                });
            })
            .into_any_element()
    }
}

impl Render for AdvancedSearchWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        window.set_window_title(cx.i18n().text(Message::ToolSearch).as_ref());
        if let Some(owner) = self.owner.upgrade() {
            window.set_rem_size(px(owner.read(cx).ui_rem_size() as f32));
        }
        let handle = cx.entity();
        let mut root = div()
            .size_full()
            .relative()
            .bg(theme.background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25));
        if self.advanced_search.is_some() {
            root = root.child(self.render_advanced_search(handle.clone(), window, cx));
        }
        // These menus belong to this window, including its text input actions.
        {
            for (index, menu) in self.context_menus.iter().enumerate() {
                let ContextMenuContent::Items(items, _) = &menu.content else {
                    continue;
                };
                let close = handle.clone();
                let width = crate::ui::scale::rem_pixels(rems(13.0));
                let height = crate::ui::scale::rem_pixels(rems(items.len() as f32 * 1.625 + 1.0));
                let viewport = window.viewport_size();
                let position = point(
                    menu.position.x.min((viewport.width - width).max(px(0.0))),
                    menu.position.y.min((viewport.height - height).max(px(0.0))),
                );
                root = root.child(deferred(
                    div().absolute().left(position.x).top(position.y).child(
                        crate::ui::components::dropdown::GenericDropdown {
                            id: ("advanced-menu", index).into(),
                            items: items.clone(),
                            reserve_icon_space: true,
                            is_context_menu: true,
                            bottom_padding: 4.0,
                            scroll_handle: menu.scroll_handle.clone(),
                            h_app: handle.clone(),
                            on_drag: std::rc::Rc::new(|_, _, _, _, _| {}),
                            on_close: std::rc::Rc::new(move |_, cx| {
                                close.update(cx, |this, cx| {
                                    this.context_menus.clear();
                                    cx.notify();
                                });
                            }),
                            on_outside: None,
                            is_dragging: false,
                            width: Some(width.as_f32()),
                            max_height: viewport.height.as_f32(),
                            header: None,
                            on_item_hover: None,
                            on_width_calculated: None,
                            on_hover_change: None,
                        },
                    ),
                ));
            }
        }
        root
    }
}
