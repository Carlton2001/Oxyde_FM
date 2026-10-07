use super::*;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Info,
    Success,
    Error,
}

pub(crate) struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: Message,
    pub detail: String,
    pub count: usize,
    created: std::time::Instant,
}

impl OxydeApp {
    pub(crate) fn change_favorite(&mut self, path: String, remove: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                if remove {
                    oxyde_lib::commands::system::remove_from_quick_access(path)
                } else {
                    oxyde_lib::commands::system::add_to_quick_access(path)
                }
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => this.state.reload_quick_access_and_refresh(),
                    Ok(Err(error)) => this.toast(
                        ToastKind::Error,
                        Message::ToastFailed,
                        error.to_string(),
                        0,
                        cx,
                    ),
                    Err(error) => this.toast(
                        ToastKind::Error,
                        Message::ToastFailed,
                        error.to_string(),
                        0,
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn toast(
        &mut self,
        kind: ToastKind,
        message: Message,
        detail: String,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        if self.toasts.last().is_some_and(|toast| {
            toast.kind == kind
                && toast.message == message
                && toast.detail == detail
                && toast.count == count
                && toast.created.elapsed().as_secs_f32() < 1.0
        }) {
            return;
        }
        self.next_toast_id += 1;
        let id = self.next_toast_id;
        self.toasts.push(Toast {
            id,
            kind,
            message,
            detail,
            count,
            created: std::time::Instant::now(),
        });
        // Bound the overlay when many independent actions finish together.
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(if kind == ToastKind::Info {
                    2
                } else {
                    5
                }))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|toast| toast.id != id);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_toasts(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let handle = cx.entity();
        div()
            .absolute()
            .bottom(rems(1.0))
            .right(rems(1.0))
            .flex()
            .flex_col()
            .items_end()
            .gap(rems(0.5))
            .max_w(window.viewport_size().width - rem_pixels(rems(2.0)))
            .children(self.toasts.iter().map(|toast| {
                let id = toast.id;
                let close = handle.clone();
                let mut args = FluentArgs::new();
                args.set("detail", toast.detail.clone());
                args.set("count", toast.count);
                args.set(
                    "number",
                    crate::ui::format::format_number(toast.count as u64, cx.i18n().language()),
                );
                let (icon, color) = match toast.kind {
                    ToastKind::Info => ("icons/info.svg", theme.accent),
                    ToastKind::Success => ("icons/circle-check.svg", theme.toast_success),
                    ToastKind::Error => ("icons/circle-x.svg", theme.toast_error),
                };
                div()
                    .id(("toast", id))
                    .max_w(rem_pixels(rems(24.0)).min(window.viewport_size().width - rem_pixels(rems(2.0))))
                    .px(rems(1.0))
                    .py(rems(0.75))
                    .rounded(rems(0.5))
                    .border(rems(0.0625))
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .shadow(crate::ui::style::rem_shadow(false))
                    .occlude()
                    .flex()
                    .items_center()
                    .gap(rems(0.75))
                    .text_size(rems(0.8125))
                    .text_color(theme.text)
                    .child(
                        svg()
                            .path(icon)
                            .size(rems(1.125))
                            .flex_shrink_0()
                            .text_color(color),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .child(cx.i18n().text_with_args(toast.message, &args)),
                    )
                    .child(
                        div()
                            .id(("toast-close", id))
                            .p(rems(0.25))
                            .rounded(rems(0.25))
                            .flex_shrink_0()
                            .hover(|style| style.bg(theme.hover))
                            .on_click(move |_, _, cx| {
                                close.update(cx, |this, cx| {
                                    this.toasts.retain(|toast| toast.id != id);
                                    cx.notify();
                                });
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size(rems(0.875))
                                    .text_color(theme.text_muted),
                            ),
                    )
            }))
    }
}
