mod completion_menu;
mod field;
mod options;
mod render;
pub(crate) use field::{control_frame, field_frame, read_only_field};
pub(crate) use options::{InputAppearance, TextInputOptions};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use unicode_segmentation::*;

actions!(
    input,
    [Backspace, Delete, Left, Right, Home, End, SelectAll, Enter, Escape, Copy, Cut, Paste, CompletionUp, CompletionDown, CompletePath]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", CompletionUp, Some("TextInput")),
        KeyBinding::new("down", CompletionDown, Some("TextInput")),
        KeyBinding::new("tab", CompletePath, Some("TextInput")),
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
    horizontal_offset: Pixels,
    on_change: Box<dyn Fn(&str, &mut Window, &mut App)>,
    on_enter: Box<dyn Fn(&mut Window, &mut App)>,
    on_escape: Box<dyn Fn(&mut Window, &mut App)>,
    on_context_menu: Box<dyn Fn(Entity<TextInput>, bool, Point<Pixels>, &mut Window, &mut App)>,
    cursor_visible: bool,
    blink_epoch: usize,
    options: TextInputOptions,
    password: bool,
    path_submit: bool,
    path_visibility: (bool, bool),
    completions: Vec<String>,
    completion_index: usize,
    completion_task: Option<Task<()>>,
    completion_epoch: u64,
    completion_bounds: Option<Bounds<Pixels>>,
    completion_anchor: Option<Bounds<Pixels>>,
    completion_scroll: UniformListScrollHandle,
    completion_pointer: completion_menu::scroll::PointerState,
}

fn masked_display_index(content: &str, byte: usize) -> usize {
    content[..byte].chars().count()
}

fn completion_parts(text: &str) -> Option<(String, String, char)> {
    if text.contains("://") || text.starts_with("::{") {
        return None;
    }
    let index = text.rfind(std::path::is_separator)?;
    let separator = std::path::MAIN_SEPARATOR;
    Some((
        native_path_text(&text[..=index]),
        text[index + 1..].to_string(),
        separator,
    ))
}

/// What the address bar shows for the network view, as Windows Explorer or Nautilus.
const NETWORK_TEXT: &str = if cfg!(target_os = "windows") { "\\\\" } else { "network:///" };

pub(crate) fn native_path_text(path: &str) -> String {
    if path == "__network_vincinity__" {
        return NETWORK_TEXT.to_string();
    }
    if cfg!(target_os = "windows") && !path.contains("://") && !path.starts_with("::{") {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

pub(crate) fn navigation_path_text(text: &str) -> String {
    let path = native_path_text(text.trim());
    if path == NETWORK_TEXT { "__network_vincinity__".to_string() } else { path }
}

fn completion_matches(name: &str, prefix: &str) -> bool {
    if cfg!(target_os = "windows") {
        name.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        name.starts_with(prefix)
    }
}
fn masked_byte_index(content: &str, index: usize) -> usize {
    content
        .char_indices()
        .nth(index)
        .map_or(content.len(), |(byte, _)| byte)
}

impl TextInput {
    pub fn with_path_submit(mut self, show_hidden: bool, show_system: bool) -> Self {
        self.path_submit = true;
        self.path_visibility = (show_hidden, show_system);
        self
    }

    pub(crate) fn set_path_visibility(&mut self, show_hidden: bool, show_system: bool, cx: &mut Context<Self>) {
        if self.path_submit && self.path_visibility != (show_hidden, show_system) {
            self.path_visibility = (show_hidden, show_system);
            self.refresh_completions(cx);
            cx.notify();
        }
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, content: String, cx: &mut Context<Self>) {
        self.content = content;
        self.selected_range = self.content.len()..self.content.len();
        self.selection_anchor = self.content.len();
        self.selection_reversed = false;
        self.last_layout = None;
        cx.notify();
    }

    fn display_index(&self, byte: usize) -> usize {
        if self.password {
            masked_display_index(&self.content, byte)
        } else {
            byte
        }
    }
    pub fn set_placeholder(&mut self, placeholder: SharedString, cx: &mut Context<Self>) {
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder = placeholder;
        cx.notify();
    }

    pub fn placeholder_width(&self, window: &Window) -> Pixels {
        let style = window.text_style();
        window
            .text_system()
            .shape_line(
                self.placeholder.clone(),
                style.font_size.to_pixels(window.rem_size()),
                &[TextRun {
                    len: self.placeholder.len(),
                    font: style.font(),
                    color: style.color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            )
            .width
    }

    pub fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        initial_text: String,
        on_change: impl Fn(&str, &mut Window, &mut App) + 'static,
        on_enter: impl Fn(&mut Window, &mut App) + 'static,
        on_escape: impl Fn(&mut Window, &mut App) + 'static,
        on_context_menu: impl Fn(Entity<TextInput>, bool, Point<Pixels>, &mut Window, &mut App)
            + 'static,
        mut options: TextInputOptions,
        window: &mut Window,
    ) -> Self {
        let len = initial_text.len();
        let selected_range = options.initial_selection.take().unwrap_or(len..len);
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
            completions: Vec::new(),
            completion_index: 0,
            completion_task: None,
            completion_epoch: 0,
            completion_bounds: None,
            completion_anchor: None,
            completion_scroll: UniformListScrollHandle::new(),
            completion_pointer: Default::default(),
            path_submit: false,
            path_visibility: (false, false),
            password: false,
            focus_handle,
            content: initial_text,
            placeholder: placeholder.into(),
            selected_range,
            selection_anchor,
            selection_reversed: false,
            is_selecting: false,
            last_layout: None,
            last_bounds: None,
            horizontal_offset: px(0.0),
            on_change: Box::new(on_change),
            on_enter: Box::new(on_enter),
            on_escape: Box::new(on_escape),
            on_context_menu: Box::new(on_context_menu),
            cursor_visible: true,
            blink_epoch: 0,
            options,
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
        let index = line.closest_index_for_x(position.x - bounds.left() + self.horizontal_offset);
        if self.password {
            masked_byte_index(&self.content, index)
        } else {
            index.min(self.content.len())
        }
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
        if !event.dragging() {
            self.is_selecting = false;
            return;
        }
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
        let position = crate::ui::components::popup::owner_point(window, cx, event.position);
        crate::ui::components::popup::with_owner(window, cx, |window, cx| (self.on_context_menu)(input, self.has_selection(), position, window, cx));
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
            let text = text.replace(['\r', '\n'], " ");
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
        self.refresh_completions(cx);
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
        self.refresh_completions(cx);
        self.reset_blinking(cx);
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_enter)(window, cx);
    }

    fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if !self.completions.is_empty() {
            self.completions.clear();
            self.completion_epoch += 1;
            self.completion_task = None;
            cx.notify();
            return;
        }
        (self.on_escape)(window, cx);
    }

    fn refresh_completions(&mut self, cx: &mut Context<Self>) {
        if !self.path_submit {
            return;
        }
        self.completion_epoch += 1;
        self.completion_task = None;
        self.completions.clear();
        self.completion_index = 0;
        let text = self.content.clone();
        let Some((parent, prefix, separator)) = completion_parts(&text) else {
            return;
        };
        let epoch = self.completion_epoch;
        let (show_hidden, show_system) = self.path_visibility;
        let entity = cx.weak_entity();
        self.completion_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let result = oxyde_lib::commands::io::list_dir(
                    parent.clone(),
                    Some(show_hidden),
                    Some(show_system),
                )
                .await;
                let paths = cx.background_executor().spawn(async move {
                    let mut paths = result
                        .map(|entries| {
                            entries
                                .into_iter()
                                .filter(|entry| {
                                    entry.is_dir && completion_matches(&entry.name, &prefix)
                                })
                                .map(|entry| format!("{}{}{}", parent, entry.name, separator))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    paths.sort_by_cached_key(|path| path.to_lowercase());
                    paths
                }).await;
                let _ = entity.update(&mut cx, |input, cx| {
                    if input.completion_epoch == epoch && input.content == text {
                        input.completions = paths;
                        input.completion_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
                        cx.notify();
                    }
                });
            }
        }));
    }

    fn completion_up(&mut self, _: &CompletionUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.completions.is_empty() { cx.propagate(); return; }
        self.completion_index = (self.completion_index + self.completions.len() - 1) % self.completions.len();
        self.completion_scroll.scroll_to_item(self.completion_index, ScrollStrategy::Top);
        cx.notify();
    }

    fn completion_down(&mut self, _: &CompletionDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.completions.is_empty() { cx.propagate(); return; }
        self.completion_index = (self.completion_index + 1) % self.completions.len();
        self.completion_scroll.scroll_to_item(self.completion_index, ScrollStrategy::Bottom);
        cx.notify();
    }

    fn complete_path(&mut self, _: &CompletePath, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.completions.get(self.completion_index).cloned() else { cx.propagate(); return; };
        self.accept_completion(path, window, cx);
    }

    fn accept_completion(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.content = path;
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_anchor = end;
        self.selection_reversed = false;
        (self.on_change)(&self.content, window, cx);
        window.focus(&self.focus_handle, cx);
        self.refresh_completions(cx);
        self.reset_blinking(cx);
    }

    fn submit_on_outside_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button == MouseButton::Left {
            if !self.completions.is_empty() && self.completion_bounds.is_some_and(|bounds| bounds.contains(&event.position)) {
                return;
            }
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
        self.refresh_completions(cx);
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
            point(
                bounds.left() + line.x_for_index(self.display_index(range.start))
                    - self.horizontal_offset,
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(self.display_index(range.end))
                    - self.horizontal_offset,
                bounds.bottom(),
            ),
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
        let index = line.closest_index_for_x(point.x - bounds.left() + self.horizontal_offset);
        let byte_index = if self.password {
            masked_byte_index(&self.content, index)
        } else {
            index
        };
        Some(self.utf16_offset_for_byte(byte_index))
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
        // The flex wrapper has auto height. A percentage height resolves to zero
        // here and would make the text's content mask empty.
        style.size.height = window.line_height().into();
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
            if input.password {
                "*".repeat(input.content.chars().count()).into()
            } else {
                input.content.clone().into()
            }
        };
        let selected_range = input.display_index(input.selected_range.start)
            ..input.display_index(input.selected_range.end);
        let style = window.text_style();
        let selection_background = cx.theme().accent;
        let selection_foreground = cx.theme().text_on_accent;
        let color = if input.content.is_empty() {
            cx.theme().text_muted
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
                color: selection_foreground,
                background_color: Some(selection_background),
                underline: None,
                strikethrough: None,
            });
            if selected_range.end < content.len() {
                runs.push(TextRun {
                    len: content.len() - selected_range.end,
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

        let caret = if input.selection_reversed {
            input.selected_range.start
        } else {
            input.selected_range.end
        };
        let offset = if input.focus_handle.is_focused(window) {
            horizontal_scroll_offset(
                input.horizontal_offset,
                line.x_for_index(input.display_index(caret)),
                line.width,
                bounds.size.width,
                rem_pixels(rems(0.09375)),
            )
        } else {
            px(0.0)
        };
        self.input
            .update(cx, |input, _| input.horizontal_offset = offset);

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
            let cursor_index = input.display_index(if input.selection_reversed {
                selected_range.start
            } else {
                selected_range.end
            });
            let offset = input.horizontal_offset;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                if !selected_range.is_empty() {
                    line.paint_background(
                        bounds.origin + point(-offset, y_offset),
                        line_h,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .unwrap();
                }
                line.paint(
                    bounds.origin + point(-offset, y_offset),
                    line_h,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .unwrap();
                if focus_handle.is_focused(window) && cursor_visible && selected_range.is_empty() {
                    let cursor_x = line.x_for_index(cursor_index);
                    let cursor_h = line_h * 0.8;
                    let cursor_y = bounds.top() + y_offset + (line_h - cursor_h) / 2.0;
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + cursor_x - offset, cursor_y),
                            size(rem_pixels(rems(0.09375)), cursor_h),
                        ),
                        cx.theme().accent,
                    ));
                }
            });

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

#[cfg(test)]
mod password_tests {
    use super::{masked_byte_index, masked_display_index};
    #[::core::prelude::v1::test]
    fn masked_unicode_selection_and_caret_round_trip_to_original_byte_boundaries() {
        for content in ["", "secret", "été🔑密碼", "a\u{301}🔒"] {
            for (index, (byte, _)) in content.char_indices().enumerate() {
                assert_eq!(masked_display_index(content, byte), index);
                assert_eq!(masked_byte_index(content, index), byte);
            }
            let end = content.chars().count();
            assert_eq!(masked_display_index(content, content.len()), end);
            assert_eq!(masked_byte_index(content, end), content.len());
            assert_eq!(masked_byte_index(content, end + 1), content.len());
        }
    }
}

#[cfg(test)]
mod completion_tests {
    use super::completion_parts;

    #[::core::prelude::v1::test]
    fn splits_native_roots_and_unicode_prefixes() {
        #[cfg(target_os = "windows")]
        let windows = [
            ("C:\\Us", "C:\\", "Us"),
            ("\\\\server\\share\\Do", "\\\\server\\share\\", "Do"),
        ];
        // `\` belongs to the name outside Windows.
        #[cfg(not(target_os = "windows"))]
        let windows = [("/x/a\\b", "/x/", "a\\b")];
        for (text, parent, prefix) in [
            ("C:/Users/été", "C:/Users/", "été"),
            ("/usr/", "/usr/", ""),
            ("/é", "/", "é"),
        ]
        .into_iter()
        .chain(windows)
        {
            assert_eq!(completion_parts(text), Some((super::native_path_text(parent), prefix.into(), std::path::MAIN_SEPARATOR)));
        }
        for text in ["", "C:", "search://query", "trash://", "::{shell}"] {
            assert!(completion_parts(text).is_none());
        }
    }

    #[::core::prelude::v1::test]
    fn native_display_preserves_virtual_paths_and_unc_roots() {
        let network = if cfg!(target_os = "windows") { "\\\\" } else { "network:///" };
        assert_eq!(super::native_path_text("__network_vincinity__"), network);
        assert_eq!(super::navigation_path_text(&format!(" {network} ")), "__network_vincinity__");
        assert_eq!(super::navigation_path_text("\\\\SERVEUR"), "\\\\SERVEUR");
        assert_eq!(
            super::navigation_path_text("\\\\SERVEUR\\Partage commun"),
            "\\\\SERVEUR\\Partage commun"
        );
        assert_eq!(super::navigation_path_text("smb://SERVEUR/"), "smb://SERVEUR/");
        assert_eq!(super::native_path_text("trash://"), "trash://");
        assert_eq!(super::native_path_text("search://query"), "search://query");
        if cfg!(target_os = "windows") {
            assert_eq!(super::native_path_text("C:/Users/été"), "C:\\Users\\été");
            assert_eq!(
                super::native_path_text("//server/share/folder"),
                "\\\\server\\share\\folder"
            );
        } else {
            assert_eq!(super::native_path_text("/usr/local"), "/usr/local");
        }
    }
}

/// Keep the active end of the selection visible without moving the text unnecessarily.
fn horizontal_scroll_offset(
    previous: Pixels,
    caret: Pixels,
    text_width: Pixels,
    viewport: Pixels,
    caret_width: Pixels,
) -> Pixels {
    let available = (viewport - caret_width).max(px(0.0));
    let maximum = (text_width - available).max(px(0.0));
    let mut offset = previous.min(maximum).max(px(0.0));
    if caret < offset {
        offset = caret;
    } else if caret > offset + available {
        offset = caret - available;
    }
    offset.min(maximum).max(px(0.0))
}

#[cfg(test)]
mod horizontal_scroll_tests {
    use super::horizontal_scroll_offset;
    use gpui::px;
    #[::core::prelude::v1::test]
    fn long_text_follows_caret_and_clamps_after_resize_or_deletion() {
        let scroll = |previous, caret, text, viewport| {
            horizontal_scroll_offset(px(previous), px(caret), px(text), px(viewport), px(2.0))
                .as_f32()
        };
        assert_eq!(scroll(0.0, 300.0, 300.0, 100.0), 202.0);
        assert_eq!(scroll(202.0, 250.0, 300.0, 100.0), 202.0);
        assert_eq!(scroll(202.0, 0.0, 300.0, 100.0), 0.0);
        assert_eq!(scroll(202.0, 20.0, 20.0, 100.0), 0.0);
        assert_eq!(scroll(202.0, 300.0, 300.0, 400.0), 0.0);
        assert_eq!(scroll(0.0, 300.0, 300.0, 0.0), 300.0);
    }
}
