use super::DuplicateWindow;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl DuplicateWindow {
    pub(super) fn render_sources(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> Stateful<Div> {
        let section_title = |icon: &'static str, message: Message| {
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .mb(rems(0.5))
                .text_size(rems(0.75))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(
                    svg()
                        .path(icon)
                        .size(rems(0.875))
                        .text_color(theme.text_muted),
                )
                .child(cx.i18n().text(message).to_uppercase())
        };
        let mut sidebar = div()
            .id("duplicate-sidebar")
            .w(rems(17.5))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.sources_scroll)
            .px(rems(1.25))
            .py(rems(0.8))
            .flex()
            .flex_col()
            .border_r(rems(0.0625))
            .border_color(theme.border)
            .child(section_title(
                "icons/database.svg",
                Message::DuplicateLocations,
            ));
        let mut locations = div()
            .flex_none()
            .rounded(rems(0.375))
            .border(rems(0.0625))
            .border_color(theme.border)
            .overflow_hidden();
        for (index, (path, label, disk)) in self.locations.iter().enumerate() {
            let selected = self.selected.contains(path);
            let current = label.is_empty();
            let target = path.clone();
            let label = if current {
                cx.i18n().text(Message::DuplicateCurrent)
            } else {
                label.clone().into()
            };
            let (media, physical) = disk.split_once("  ").unwrap_or((disk.as_str(), ""));
            let media_name = if media.contains("SSD") {
                "SSD"
            } else if media.contains("HDD") {
                "HDD"
            } else {
                media
            };
            let badge_bg = if media.contains("SSD") {
                theme.media_ssd
            } else if media.contains("HDD") {
                theme.media_hdd
            } else if self.location_icons[index] == "icons/usb.svg" {
                theme.media_removable
            } else if self.location_icons[index] == "icons/network.svg" {
                theme.media_network
            } else {
                theme.badge_surface
            };
            let mut badges = div().flex().items_center().gap(rems(0.375));
            if !media_name.is_empty() {
                badges = badges.child(
                    div()
                        .rounded(rems(0.2))
                        .px(rems(0.35))
                        .py(rems(0.0625))
                        .text_size(rems(0.625))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(badge_bg)
                        .text_color(theme.text_on_badge)
                        .child(media_name.to_string()),
                );
            }
            if !physical.is_empty() {
                badges = badges.child(
                    div()
                        .rounded(rems(0.2))
                        .px(rems(0.35))
                        .py(rems(0.0625))
                        .text_size(rems(0.625))
                        .bg(theme.badge_surface)
                        .text_color(theme.text_muted)
                        .child(physical.to_uppercase()),
                );
            }
            let mut content = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(rems(0.2))
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected { theme.accent } else { theme.text })
                        .child(label),
                )
                .child(badges);
            if current {
                content = content.child(
                    div()
                        .id(("duplicate-source-path", index))
                        .truncate()
                        .text_size(rems(0.75))
                        .text_color(theme.text_muted)
                        .tooltip(crate::ui::components::tooltip::Tooltip::text(path.clone()))
                        .child(path.clone()),
                );
            }
            locations = locations.child(
                div()
                    .id(("location", index))
                    .flex_none()
                    .tab_index(0)
                    .focus_visible(move |s| s.bg(theme.selected))
                    .px(rems(0.75))
                    .py(rems(0.625))
                    .flex()
                    .items_center()
                    .gap(rems(0.75))
                    .bg(if selected {
                        theme.selected
                    } else {
                        theme.background
                    })
                    .border_b(if index + 1 < self.locations.len() {
                        rems(0.0625)
                    } else {
                        rems(0.0)
                    })
                    .border_color(theme.border)
                    .hover(move |s| {
                        s.bg(if selected {
                            theme.selected
                        } else {
                            theme.hover
                        })
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.running {
                            if !this.selected.remove(&target) {
                                this.selected.insert(target.clone());
                            }
                            cx.notify();
                        }
                    }))
                    .child(
                        svg()
                            .path(self.location_icons[index])
                            .size(rems(0.875))
                            .flex_none()
                            .text_color(theme.text_muted),
                    )
                    .child(content),
            );
        }
        sidebar = sidebar.child(locations);
        if self.selected.len() > 1 {
            sidebar = sidebar.child(
                div()
                    .flex_none()
                    .p(rems(0.75))
                    .mt(rems(0.125))
                    .rounded(rems(0.5))
                    .border(rems(0.0625))
                    .border_color(theme.warning_border)
                    .bg(theme.warning_background)
                    .text_color(theme.warning_text)
                    .child(cx.i18n().text(Message::DuplicateWarning)),
            );
        }
        sidebar = sidebar.child(div().mt(rems(1.75)).child(section_title(
            "icons/settings-2.svg",
            Message::DuplicateSettings,
        )));
        let mut options = div()
            .flex_none()
            .rounded(rems(0.5))
            .border(rems(0.0625))
            .border_color(theme.border)
            .bg(theme.header)
            .p(rems(0.75))
            .flex()
            .flex_col()
            .gap(rems(0.6));
        for (index, message) in [
            Message::DuplicateName,
            Message::DuplicateSize,
            Message::DuplicateContent,
        ]
        .into_iter()
        .enumerate()
        {
            let checked = self.options[index];
            options = options.child(
                div()
                    .id(("criterion", index))
                    .tab_index(0)
                    .flex()
                    .items_center()
                    .gap(rems(0.5))
                    .focus_visible(move |s| s.bg(theme.selected))
                    .child(
                        div()
                            .size(rems(1.0))
                            .rounded(rems(0.25))
                            .border(rems(0.0625))
                            .border_color(if checked { theme.accent } else { theme.border })
                            .bg(if checked {
                                theme.accent
                            } else {
                                theme.background
                            })
                            .when(checked, |s| {
                                s.child(
                                    svg()
                                        .path("icons/check.svg")
                                        .size(rems(0.875))
                                        .text_color(theme.text_on_accent),
                                )
                            }),
                    )
                    .child(cx.i18n().text(message))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.running {
                            this.options[index] = !this.options[index];
                            let options = this.options;
                            let _ = this.owner.update(cx, |owner, cx| {
                                owner.config.duplicate_search_options = Some(serde_json::json!(options));
                                owner.save_preferences(cx);
                            });
                            cx.notify();
                        }
                    })),
            );
        }
        sidebar = sidebar.child(options);
            sidebar
    }
}
