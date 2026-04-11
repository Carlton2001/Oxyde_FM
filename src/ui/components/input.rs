use gpui::*;
use gpui::prelude::*;
use unicode_segmentation::*;

actions!(input, [Backspace, Delete, Left, Right, Home, End, SelectAll, Enter, Escape]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("enter", Enter, Some("TextInput")),
        KeyBinding::new("escape", Escape, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
    ]);
}
pub struct TextInput {
    pub focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    selected_range: std::ops::Range<usize>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    on_change: Box<dyn Fn(&str, &mut Window, &mut App)>,
    on_enter: Box<dyn Fn(&mut Window, &mut App)>,
    on_escape: Box<dyn Fn(&mut Window, &mut App)>,
    cursor_visible: bool,
    blink_epoch: usize,
    pub minimal: bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>, initial_text: String, on_change: impl Fn(&str, &mut Window, &mut App) + 'static, on_enter: impl Fn(&mut Window, &mut App) + 'static, on_escape: impl Fn(&mut Window, &mut App) + 'static, minimal: bool, initial_selection: Option<std::ops::Range<usize>>, window: &mut Window) -> Self {
        let len = initial_text.len();
        let focus_handle = cx.focus_handle();
        
        cx.on_focus(&focus_handle, window, |this, _, cx| {
            this.reset_blinking(cx);
        }).detach();

        cx.on_focus_out(&focus_handle, window, |this, _, _, _| {
            this.blink_epoch += 1;
            this.cursor_visible = true;
        }).detach();

        Self {
            focus_handle,
            content: initial_text,
            placeholder: placeholder.into(),
            selected_range: initial_selection.unwrap_or(len..len),
            last_layout: None,
            last_bounds: None,
            on_change: Box::new(on_change),
            on_enter: Box::new(on_enter),
            on_escape: Box::new(on_escape),
            cursor_visible: true,
            blink_epoch: 0,
            minimal,
        }
    }

    fn blink_cursors(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if epoch == self.blink_epoch {
            let this_weak = cx.weak_entity();
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                    let _ = this_weak.update(&mut cx, |this, cx| {
                        if epoch == this.blink_epoch {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                            this.blink_cursors(epoch, cx);
                        }
                    });
                }
            }).detach();
        }
    }

    fn reset_blinking(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch += 1;
        self.cursor_visible = true; // Rétablit la visibilité immédiate
        cx.notify();
        let epoch = self.blink_epoch;
        self.blink_cursors(epoch, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.selected_range.start > 0 {
            let prev = self.content.grapheme_indices(true).rev().find(|(i, _)| *i < self.selected_range.start).map(|(i, _)| i).unwrap_or(0);
            self.content.replace_range(prev..self.selected_range.start, "");
            self.selected_range = prev..prev;
        } else {
            self.content.replace_range(self.selected_range.clone(), "");
            self.selected_range = self.selected_range.start..self.selected_range.start;
        }
        (self.on_change)(&self.content, window, cx);
        self.reset_blinking(cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.selected_range.end < self.content.len() {
            let next = self.content.grapheme_indices(true).find(|(i, _)| *i > self.selected_range.start).map(|(i, _)| i).unwrap_or(self.content.len());
            self.content.replace_range(self.selected_range.start..next, "");
        } else {
             self.content.replace_range(self.selected_range.clone(), "");
             self.selected_range = self.selected_range.start..self.selected_range.start;
        }
        (self.on_change)(&self.content, window, cx);
        self.reset_blinking(cx);
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_enter)(window, cx);
    }

    fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_escape)(window, cx);
    }

    fn move_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.selected_range = self.selected_range.start..self.selected_range.start;
            self.reset_blinking(cx);
        } else if self.selected_range.start > 0 {
            let prev = self.content.grapheme_indices(true).rev().find(|(i, _)| *i < self.selected_range.start).map(|(i, _)| i).unwrap_or(0);
            self.selected_range = prev..prev;
            self.reset_blinking(cx);
        }
    }

    fn move_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.selected_range = self.selected_range.end..self.selected_range.end;
            self.reset_blinking(cx);
        } else if self.selected_range.end < self.content.len() {
            let next = self.content.grapheme_indices(true).find(|(i, _)| *i > self.selected_range.start).map(|(i, _)| i).unwrap_or(self.content.len());
            self.selected_range = next..next;
            self.reset_blinking(cx);
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(&mut self, range: std::ops::Range<usize>, _: &mut Option<std::ops::Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        self.content.get(range).map(|s| s.to_string())
    }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.selected_range.clone(), reversed: false })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<std::ops::Range<usize>> { None }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) { }
    fn replace_text_in_range(&mut self, range: Option<std::ops::Range<usize>>, new_text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let range = range.unwrap_or(self.selected_range.clone());
        self.content.replace_range(range.clone(), new_text);
        let new_pos = range.start + new_text.len();
        self.selected_range = new_pos..new_pos;
        (self.on_change)(&self.content, window, cx);
        self.reset_blinking(cx);
    }
    fn replace_and_mark_text_in_range(&mut self, range: Option<std::ops::Range<usize>>, new_text: &str, _: Option<std::ops::Range<usize>>, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(range, new_text, window, cx);
    }
    fn bounds_for_range(&mut self, _: std::ops::Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> { Some(bounds) }
    fn character_index_for_point(&mut self, point: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let line = self.last_layout.as_ref()?;
        let bounds = self.last_bounds.as_ref()?;
        Some(line.closest_index_for_x(point.x - bounds.left()))
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window);
        
        div()
            .flex()
            .items_center()
            .key_context("TextInput")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window.focus(&focus_handle);
            })
            .bg(if is_focused { hsla(0.0, 0.0, 0.16, 1.0) } else { hsla(0.0, 0.0, 0.12, 1.0) })
            .border_1()
            .border_color(if is_focused { hsla(0.6, 1.0, 0.5, 0.6) } else { hsla(0.0, 0.0, 0.25, 1.0) })
            .rounded_sm()
            .when(self.minimal, |s| s.h_full())
            .when(!self.minimal, |s| s.h_8().px_2())
            .w_full()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .cursor(CursorStyle::IBeam)
                    .child(TextInputElement { input: cx.entity() })
            )
            .when(!self.minimal && !self.content.is_empty(), |s: Div| {
                let h_clear = cx.entity().clone();
                s.child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .p_1()
                                .rounded_md()
                                .hover(|s| s.bg(hsla(0.0, 0.0, 0.3, 1.0)))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    let _ = h_clear.update(cx, |this, cx| {
                                        this.content.clear();
                                        this.selected_range = 0..0;
                                        (this.on_change)("", window, cx);
                                        cx.notify();
                                    });
                                })
                                .child(svg().path("icons/x.svg").size_3().text_color(hsla(0.0, 0.0, 0.6, 1.0)))
                        )
                )
            })
    }
}

struct TextInputElement {
    input: Entity<TextInput>,
}

impl IntoElement for TextInputElement {
    type Element = Self;
    fn into_element(self) -> Self::Element { self }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = (Option<ShapedLine>, Option<Bounds<Pixels>>);

    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut Self::RequestLayoutState, window: &mut Window, cx: &mut App) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = if input.content.is_empty() { input.placeholder.clone() } else { input.content.clone().into() };
        let style = window.text_style();
        let color = if input.content.is_empty() { hsla(0.0, 0.0, 0.45, 1.0) } else { style.color };
        
        // Use a fixed font size or the window's current text style
        let line = window.text_system().shape_line(content.clone(), style.font_size.to_pixels(window.rem_size()), &[TextRun {
            len: content.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }], None);
        
        (Some(line), Some(bounds))
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut Self::RequestLayoutState, (line, _): &mut Self::PrepaintState, window: &mut Window, cx: &mut App) {
        let input_handle = self.input.clone();
        let focus_handle = input_handle.read(cx).focus_handle.clone();
        window.handle_input(&focus_handle, ElementInputHandler::new(bounds, input_handle.clone()), cx);
        
        if let Some(line) = line.take() {
            let line_h = window.line_height();
            let y_offset = (bounds.size.height - line_h) / 2.0;

            line.paint(bounds.origin + point(px(0.0), y_offset), line_h, window, cx).unwrap();
            
            let input = input_handle.read(cx);
            if focus_handle.is_focused(window) && input.cursor_visible {
                let cursor_x = line.x_for_index(input.selected_range.start);
                let cursor_h = line_h * 0.8;
                let cursor_y = bounds.top() + y_offset + (line_h - cursor_h) / 2.0;
                window.paint_quad(fill(
                    Bounds::new(point(bounds.left() + cursor_x, cursor_y), size(px(1.5), cursor_h)), 
                    hsla(0.6, 1.0, 0.5, 1.0)
                ));
            }
            
            input_handle.update(cx, |input, _| {
                input.last_layout = Some(line);
                input.last_bounds = Some(bounds);
            });
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
