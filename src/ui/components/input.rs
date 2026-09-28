use gpui::prelude::*;
use gpui::*;
use unicode_segmentation::*;

actions!(
    input,
    [Backspace, Delete, Left, Right, Home, End, SelectAll, Enter, Escape, Copy, Cut, Paste]
);

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
        KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
        KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
        KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
    ]);
}
pub struct TextInput {
    pub focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    selected_range: std::ops::Range<usize>,
    selection_anchor: usize,
    selection_reversed: bool,
    is_selecting: bool,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    on_change: Box<dyn Fn(&str, &mut Window, &mut App)>,
    on_enter: Box<dyn Fn(&mut Window, &mut App)>,
    on_escape: Box<dyn Fn(&mut Window, &mut App)>,
    on_context_menu: Box<dyn Fn(Entity<TextInput>, bool, Point<Pixels>, &mut Window, &mut App)>,
    cursor_visible: bool,
    blink_epoch: usize,
    pub minimal: bool,
    compact: bool,
    submit_on_outside: bool,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        initial_text: String,
        on_change: impl Fn(&str, &mut Window, &mut App) + 'static,
        on_enter: impl Fn(&mut Window, &mut App) + 'static,
        on_escape: impl Fn(&mut Window, &mut App) + 'static,
        on_context_menu: impl Fn(Entity<TextInput>, bool, Point<Pixels>, &mut Window, &mut App)
            + 'static,
        minimal: bool,
        compact: bool,
        submit_on_outside: bool,
        initial_selection: Option<std::ops::Range<usize>>,
        window: &mut Window,
    ) -> Self {
        let len = initial_text.len();
        let selected_range = initial_selection.unwrap_or(len..len);
        let selection_anchor = selected_range.start;
        let focus_handle = cx.focus_handle();

        cx.on_focus(&focus_handle, window, |this, _, cx| {
            this.reset_blinking(cx);
        })
        .detach();

        cx.on_focus_out(&focus_handle, window, |this, _, _, _| {
            this.blink_epoch += 1;
            this.cursor_visible = true;
        })
        .detach();

        Self {
            focus_handle,
            content: initial_text,
            placeholder: placeholder.into(),
            selected_range,
            selection_anchor,
            selection_reversed: false,
            is_selecting: false,
            last_layout: None,
            last_bounds: None,
            on_change: Box::new(on_change),
            on_enter: Box::new(on_enter),
            on_escape: Box::new(on_escape),
            on_context_menu: Box::new(on_context_menu),
            cursor_visible: true,
            blink_epoch: 0,
            minimal,
            compact,
            submit_on_outside,
        }
    }

    fn blink_cursors(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if epoch == self.blink_epoch {
            let this_weak = cx.weak_entity();
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    let _ = this_weak.update(&mut cx, |this, cx| {
                        if epoch == this.blink_epoch {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                            this.blink_cursors(epoch, cx);
                        }
                    });
                }
            })
            .detach();
        }
    }

    fn reset_blinking(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch += 1;
        self.cursor_visible = true; // Rétablit la visibilité immédiate
        cx.notify();
        let epoch = self.blink_epoch;
        self.blink_cursors(epoch, cx);
    }

    fn byte_offset_for_utf16(&self, utf16_offset: usize) -> usize {
        let mut units = 0;
        for (byte_offset, character) in self.content.char_indices() {
            if units >= utf16_offset {
                return byte_offset;
            }
            let next_units = units + character.len_utf16();
            if utf16_offset < next_units {
                return byte_offset;
            }
            units = next_units;
        }
        self.content.len()
    }

    fn range_from_utf16(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        self.byte_offset_for_utf16(range.start)..self.byte_offset_for_utf16(range.end)
    }

    fn utf16_offset_for_byte(&self, byte_offset: usize) -> usize {
        let byte_offset = byte_offset.min(self.content.len());
        let boundary = (0..=byte_offset)
            .rev()
            .find(|offset| self.content.is_char_boundary(*offset))
            .unwrap_or(0);
        self.content[..boundary].encode_utf16().count()
    }

    fn range_to_utf16(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        self.utf16_offset_for_byte(range.start)..self.utf16_offset_for_byte(range.end)
    }

    fn byte_index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (&self.last_bounds, &self.last_layout) else {
            return 0;
        };
        line.closest_index_for_x(position.x - bounds.left())
            .min(self.content.len())
    }

    fn select_word_at(&mut self, byte_index: usize) {
        if self.content.is_empty() {
            self.selected_range = 0..0;
            self.selection_anchor = 0;
            return;
        }
        let chars: Vec<(usize, char)> = self.content.char_indices().collect();
        let char_index = chars
            .iter()
            .position(|(start, character)| {
                let end = *start + character.len_utf8();
                byte_index >= *start && byte_index < end
            })
            .unwrap_or_else(|| chars.len().saturating_sub(1));
        let is_word = |character: char| character.is_alphanumeric() || character == '_';
        let selected_char = chars[char_index].1;
        let mut start = char_index;
        let mut end = char_index + 1;
        if is_word(selected_char) {
            while start > 0 && is_word(chars[start - 1].1) {
                start -= 1;
            }
            while end < chars.len() && is_word(chars[end].1) {
                end += 1;
            }
        }
        let byte_start = chars[start].0;
        let byte_end = chars
            .get(end)
            .map(|(offset, _)| *offset)
            .unwrap_or(self.content.len());
        self.selected_range = byte_start..byte_end;
        self.selection_anchor = byte_start;
        self.selection_reversed = false;
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Keep text selection clicks from reaching parent rows (for example, the
        // file list row hosting the inline rename field).
        cx.stop_propagation();
        window.focus(&self.focus_handle, cx);
        let index = self.byte_index_for_position(event.position);
        if event.click_count >= 3 {
            self.selected_range = 0..self.content.len();
            self.selection_anchor = 0;
            self.selection_reversed = false;
            self.is_selecting = false;
        } else if event.click_count == 2 {
            self.select_word_at(index);
            self.is_selecting = false;
        } else if event.modifiers.shift {
            let anchor = self.selection_anchor;
            self.selected_range = anchor.min(index)..anchor.max(index);
            self.selection_reversed = index < anchor;
            self.is_selecting = true;
        } else {
            self.selection_anchor = index;
            self.selected_range = index..index;
            self.selection_reversed = false;
            self.is_selecting = true;
        }
        self.reset_blinking(cx);
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let index = self.byte_index_for_position(event.position);
        self.selected_range = self.selection_anchor.min(index)..self.selection_anchor.max(index);
        self.selection_reversed = index < self.selection_anchor;
        self.cursor_visible = self.selected_range.is_empty();
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_right_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let input = cx.entity();
        (self.on_context_menu)(input, self.has_selection(), event.position, window, cx);
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_range.is_empty()
    }

    pub fn copy_selected_text(&self, cx: &mut Context<Self>) {
        if let Some(text) = self.content.get(self.selected_range.clone()) {
            if !text.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            }
        }
    }

    pub fn cut_selected_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.copy_selected_text(cx);
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    pub fn paste_from_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = text.replace('\r', " ").replace('\n', " ");
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_anchor = 0;
        self.selection_reversed = false;
        self.reset_blinking(cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_selected_text(cx);
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.cut_selected_text(window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_from_clipboard(window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..0;
        self.selection_anchor = 0;
        self.selection_reversed = false;
        self.reset_blinking(cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_anchor = end;
        self.selection_reversed = false;
        self.reset_blinking(cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.selected_range.start > 0 {
            let prev = self
                .content
                .grapheme_indices(true)
                .rev()
                .find(|(i, _)| *i < self.selected_range.start)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content
                .replace_range(prev..self.selected_range.start, "");
            self.selected_range = prev..prev;
            self.selection_anchor = prev;
            self.selection_reversed = false;
        } else {
            let cursor = self.selected_range.start;
            self.content.replace_range(self.selected_range.clone(), "");
            self.selected_range = cursor..cursor;
            self.selection_anchor = cursor;
            self.selection_reversed = false;
        }
        (self.on_change)(&self.content, window, cx);
        self.reset_blinking(cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() && self.selected_range.end < self.content.len() {
            let next = self
                .content
                .grapheme_indices(true)
                .find(|(i, _)| *i > self.selected_range.start)
                .map(|(i, _)| i)
                .unwrap_or(self.content.len());
            self.content
                .replace_range(self.selected_range.start..next, "");
            self.selected_range = self.selected_range.start..self.selected_range.start;
            self.selection_anchor = self.selected_range.start;
            self.selection_reversed = false;
        } else {
            let cursor = self.selected_range.start;
            self.content.replace_range(self.selected_range.clone(), "");
            self.selected_range = cursor..cursor;
            self.selection_anchor = cursor;
            self.selection_reversed = false;
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

    fn submit_on_outside_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Left {
            (self.on_enter)(window, cx);
        }
    }

    fn move_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.selected_range = self.selected_range.start..self.selected_range.start;
            self.selection_anchor = self.selected_range.start;
            self.selection_reversed = false;
            self.reset_blinking(cx);
        } else if self.selected_range.start > 0 {
            let prev = self
                .content
                .grapheme_indices(true)
                .rev()
                .find(|(i, _)| *i < self.selected_range.start)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.selected_range = prev..prev;
            self.selection_anchor = prev;
            self.selection_reversed = false;
            self.reset_blinking(cx);
        }
    }

    fn move_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.selected_range = self.selected_range.end..self.selected_range.end;
            self.selection_anchor = self.selected_range.start;
            self.selection_reversed = false;
            self.reset_blinking(cx);
        } else if self.selected_range.end < self.content.len() {
            let next = self
                .content
                .grapheme_indices(true)
                .find(|(i, _)| *i > self.selected_range.start)
                .map(|(i, _)| i)
                .unwrap_or(self.content.len());
            self.selected_range = next..next;
            self.selection_anchor = next;
            self.selection_reversed = false;
            self.reset_blinking(cx);
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        actual_range: &mut Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(range_utf16);
        actual_range.replace(self.range_to_utf16(range.clone()));
        self.content.get(range).map(|s| s.to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(self.selected_range.clone()),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(
        &self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        None
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<std::ops::Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|range| self.range_from_utf16(range))
            .unwrap_or(self.selected_range.clone());
        self.content.replace_range(range.clone(), new_text);
        let new_pos = range.start + new_text.len();
        self.selected_range = new_pos..new_pos;
        self.selection_anchor = new_pos;
        self.selection_reversed = false;
        self.is_selecting = false;
        (self.on_change)(&self.content, window, cx);
        self.reset_blinking(cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, new_text, window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let Some(line) = self.last_layout.as_ref() else {
            return Some(bounds);
        };
        let range = self.range_from_utf16(range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let line = self.last_layout.as_ref()?;
        let bounds = self.last_bounds.as_ref()?;
        let byte_index = line.closest_index_for_x(point.x - bounds.left());
        Some(self.utf16_offset_for_byte(byte_index))
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
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
            .when(self.submit_on_outside, |div| {
                div.on_mouse_down_out(cx.listener(Self::submit_on_outside_click))
            })
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .bg(if self.compact {
                hsla(0.0, 0.0, 0.0, 0.0)
            } else if is_focused {
                hsla(0.0, 0.0, 0.16, 1.0)
            } else {
                hsla(0.0, 0.0, 0.12, 1.0)
            })
            .border_1()
            .border_color(if self.compact {
                hsla(0.0, 0.0, 0.0, 0.0)
            } else if is_focused {
                hsla(0.6, 1.0, 0.5, 0.6)
            } else {
                hsla(0.0, 0.0, 0.25, 1.0)
            })
            .rounded_sm()
            .when(self.compact, |s| s.rounded_none())
            .when(self.minimal && !self.compact, |s| s.h_full().px_3().py_2())
            .when(self.minimal && self.compact, |s| s.h_full().px(px(0.0)))
            .when(!self.minimal, |s| s.h_8().px_2())
            .w_full()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .cursor(CursorStyle::IBeam)
                    .child(TextInputElement { input: cx.entity() }),
            )
            .when(!self.minimal && !self.content.is_empty(), |s: Div| {
                let h_clear = cx.entity().clone();
                s.child(
                    div().flex().items_center().child(
                        div()
                            .p_1()
                            .rounded_md()
                            .hover(|s| s.bg(hsla(0.0, 0.0, 0.3, 1.0)))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                h_clear.update(cx, |this, cx| {
                                    this.content.clear();
                                    this.selected_range = 0..0;
                                    (this.on_change)("", window, cx);
                                    cx.notify();
                                });
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size_3()
                                    .text_color(hsla(0.0, 0.0, 0.6, 1.0)),
                            ),
                    ),
                )
            })
    }
}

struct TextInputElement {
    input: Entity<TextInput>,
}

impl IntoElement for TextInputElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = (Option<ShapedLine>, Option<Bounds<Pixels>>);

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = if input.content.is_empty() {
            input.placeholder.clone()
        } else {
            input.content.clone().into()
        };
        let selected_range = input.selected_range.clone();
        let style = window.text_style();
        let color = if input.content.is_empty() {
            hsla(0.0, 0.0, 0.45, 1.0)
        } else {
            style.color
        };

        let mut runs = Vec::with_capacity(3);
        if !input.content.is_empty() && !selected_range.is_empty() {
            if selected_range.start > 0 {
                runs.push(TextRun {
                    len: selected_range.start,
                    font: style.font(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                });
            }
            runs.push(TextRun {
                len: selected_range.end - selected_range.start,
                font: style.font(),
                color,
                background_color: Some(hsla(0.60, 0.90, 0.52, 0.95)),
                underline: None,
                strikethrough: None,
            });
            if selected_range.end < input.content.len() {
                runs.push(TextRun {
                    len: input.content.len() - selected_range.end,
                    font: style.font(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                });
            }
        } else {
            runs.push(TextRun {
                len: content.len(),
                font: style.font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            });
        }

        let line = window.text_system().shape_line(
            content.clone(),
            style.font_size.to_pixels(window.rem_size()),
            &runs,
            None,
        );

        (Some(line), Some(bounds))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        (line, _): &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let input_handle = self.input.clone();
        let focus_handle = input_handle.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, input_handle.clone()),
            cx,
        );

        if let Some(line) = line.take() {
            let line_h = window.line_height();
            let y_offset = (bounds.size.height - line_h) / 2.0;

            let input = input_handle.read(cx);
            let selected_range = input.selected_range.clone();
            let cursor_visible = input.cursor_visible;
            if !selected_range.is_empty() {
                line.paint_background(
                    bounds.origin + point(px(0.0), y_offset),
                    line_h,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .unwrap();
            }
            line.paint(
                bounds.origin + point(px(0.0), y_offset),
                line_h,
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .unwrap();
            if focus_handle.is_focused(window) && cursor_visible && selected_range.is_empty() {
                let cursor_x = line.x_for_index(selected_range.start);
                let cursor_h = line_h * 0.8;
                let cursor_y = bounds.top() + y_offset + (line_h - cursor_h) / 2.0;
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x, cursor_y),
                        size(px(1.5), cursor_h),
                    ),
                    hsla(0.6, 1.0, 0.5, 1.0),
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
