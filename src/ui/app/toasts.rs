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
    archive_source: Option<String>,
}

impl OxydeApp {
    pub(crate) fn archive_recovery_notice(&mut self, detail: &str, cx: &mut Context<Self>) -> bool {
        let Some(recovery) = oxyde_lib::models::error::ArchiveRecoveryFailure::from_message(detail) else { return false; };
        self.refresh_all_tabs(cx);
        self.state.refresh_sidebar();
        self.reconcile_watchers(cx);
        self.toast(if recovery.completed { ToastKind::Info } else { ToastKind::Error }, Message::ArchiveRecoveryNotice, detail.into(), 0, cx);
        true
    }

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
                    Ok(Ok(())) => this.refresh_quick_access(cx),
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
            archive_source: None,
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
                this.toasts.retain(|toast| {
                    toast.id != id
                        || toast.archive_source.is_some()
                        || oxyde_lib::models::error::ArchiveLimitFailure::from_message(
                            &toast.detail,
                        )
                        .is_some()
                        || oxyde_lib::models::error::ArchiveRecoveryFailure::from_message(&toast.detail).is_some()
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn archive_cache_failed(
        &mut self,
        issue: oxyde_lib::models::error::ArchiveCacheIssue,
        source: String,
        cx: &mut Context<Self>,
    ) {
        use oxyde_lib::models::error::ArchiveCacheIssue;
        let message = match issue {
            ArchiveCacheIssue::Full => Message::ArchiveCacheFull,
            ArchiveCacheIssue::Busy => Message::ArchiveCacheBusy,
            ArchiveCacheIssue::Unavailable => Message::ArchiveCacheUnavailable,
        };
        let name = std::path::Path::new(&source)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.toast(ToastKind::Error, message, name, 0, cx);
        if let Some(toast) = self.toasts.last_mut() {
            toast.archive_source = Some(source);
        }
    }

    fn extract_archive_to_folder(&mut self, source: String, toast_id: u64, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(cx.i18n().text(Message::ArchiveCacheExtract)),
        });
        cx.spawn(async move |this, cx| {
            let folder = match prompt.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => path,
                    None => return,
                },
                Ok(Ok(None)) => return,
                result => {
                    let detail = format!("{result:?}");
                    let _ = this.update(cx, |this, cx| {
                        this.toast(ToastKind::Error, Message::OpenItemFailed, detail, 0, cx)
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|toast| toast.id != toast_id);
                cx.notify();
            });
            let result = tokio::task::spawn_blocking(move || {
                let path = oxyde_lib::commands::archive::extract_archive_file_to(
                    std::path::Path::new(&source),
                    &folder,
                )?;
                let opened = oxyde_lib::platform::platform().open_item(&path.to_string_lossy());
                Ok::<_, oxyde_lib::models::CommandError>((path, opened))
            })
            .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok((path, opened))) => {
                    this.toast(
                        ToastKind::Success,
                        Message::ArchiveCacheExported,
                        path.to_string_lossy().into_owned(),
                        0,
                        cx,
                    );
                    if let Err(error) = opened {
                        this.toast(ToastKind::Error, Message::OpenItemFailed, error, 0, cx);
                    }
                }
                Ok(Err(error)) => this.toast(
                    ToastKind::Error,
                    Message::OpenItemFailed,
                    error.to_string(),
                    0,
                    cx,
                ),
                Err(error) => this.toast(
                    ToastKind::Error,
                    Message::OpenItemFailed,
                    error.to_string(),
                    0,
                    cx,
                ),
            });
        })
        .detach();
    }

    pub(crate) fn render_toasts(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let handle = cx.entity();
        div()
            .absolute()
            .bottom(crate::ui::style::STATUS_BAR_HEIGHT + rems(1.0))
            .right(rems(1.0))
            .flex()
            .flex_col()
            .items_end()
            .gap(rems(0.5))
            .max_w(window.viewport_size().width - rem_pixels(rems(2.0)))
            .children(crate::ui::components::operations::render_operation_progress(self, cx))
            .children(self.toasts.iter().map(|toast| {
                let id = toast.id;
                let close = handle.clone();
                let export = handle.clone();
                let source = toast.archive_source.clone();
                let mut args = FluentArgs::new();
                args.set(
                    "detail",
                    if toast.kind == ToastKind::Error || oxyde_lib::models::error::ArchiveRecoveryFailure::from_message(&toast.detail).is_some() {
                        crate::ui::format::error_detail(&toast.detail, cx.i18n())
                    } else {
                        crate::ui::format::display_paths(&toast.detail)
                    },
                );
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
                let message = cx.i18n().text_with_args(toast.message, &args);
                let width = if source.is_some() || toast.message == Message::ArchiveRecoveryNotice || message.chars().count() > 120 { 40.0 } else { 24.0 };
                div()
                    .id(("toast", id))
                    .max_w(
                        rem_pixels(rems(width))
                            .min((window.viewport_size().width - rem_pixels(rems(2.0))).max(px(1.0))),
                    )
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
                            .flex()
                            .flex_col()
                            .gap(rems(0.5))
                            .child(message)
                            .when(toast.message == Message::ArchiveRecoveryNotice, |column| {
                                column.child(crate::ui::format::error_detail(&toast.detail, cx.i18n()))
                            })
                            .when_some(source, |column, source| {
                                column.child(
                                    div()
                                        .id(("archive-cache-export", id))
                                        .self_start()
                                        .px(rems(0.75))
                                        .py(rems(0.375))
                                        .rounded(rems(0.25))
                                        .border(rems(0.0625))
                                        .border_color(theme.accent)
                                        .bg(theme.accent)
                                        .text_color(theme.text_on_accent)
                                        .hover(|style| style.opacity(0.9))
                                        .child(cx.i18n().text(Message::ArchiveCacheExtract))
                                        .on_click(move |_, _, cx| {
                                            export.update(cx, |this, cx| {
                                                this.extract_archive_to_folder(
                                                    source.clone(),
                                                    id,
                                                    cx,
                                                )
                                            });
                                        }),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id(("toast-close", id))
                            .self_start()
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
