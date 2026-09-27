use crate::ui::app::OxydeApp;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

/// Renders the Settings / Preferences panel as a full-window modal overlay.
pub fn render_settings_panel(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let theme = *cx.theme();

    let show_hidden = view.show_hidden;
    let show_system = view.show_system;
    let show_media_previews = view.show_media_previews;

    let h_close = handle.clone();
    let h_backdrop = handle.clone();
    let h_hidden = handle.clone();
    let h_system = handle.clone();
    let h_media = handle.clone();

    // Full-screen backdrop — intercepts all clicks (prevents click-through)
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000088u32))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            // Click on backdrop closes the panel
            h_backdrop.update(cx, |this, cx| {
                this.settings_open = false;
                cx.notify();
            });
        })
        .child(
            // ── Modal panel ───────────────────────────────────────────────────
            div()
                .id("settings-panel")
                .w(px(440.0))
                .rounded_lg()
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border)
                .overflow_hidden()
                // Stop click from bubbling to the backdrop
                .on_mouse_down(MouseButton::Left, |_, _, cx| { cx.stop_propagation(); })
                // ── Header ───────────────────────────────────────────────────
                .child(
                    div()
                        .h(px(52.0))
                        .px(px(20.0))
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text)
                                .child("Préférences")
                        )
                        .child(
                            div()
                                .id("settings-close-btn")
                                .size(px(28.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff15u32)))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    h_close.update(cx, |this, cx| {
                                        this.settings_open = false;
                                        cx.notify();
                                    });
                                })
                                .child(
                                    svg()
                                        .path("icons/x.svg")
                                        .size(px(14.0))
                                        .text_color(theme.text_muted)
                                )
                        )
                )
                // ── Body ─────────────────────────────────────────────────────
                .child(
                    div()
                        .px(px(20.0))
                        .py(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        // ── FICHIERS section header ───────────────────────────
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_muted)
                                .pb(px(8.0))
                                .child("FICHIERS")
                        )
                        // ── Fichiers cachés ────────────────────────────────────
                        .child(settings_row(
                            "Fichiers cachés",
                            "Afficher les fichiers et dossiers cachés ou commençant par un point",
                            show_hidden,
                            "toggle-hidden",
                            h_hidden,
                            theme,
                            |this, cx| {
                                this.show_hidden = !this.show_hidden;
                                let pane_paths: Vec<(String, String)> = {
                                    let mut v = Vec::new();
                                    this.state.layout.visit_panes(&mut |p| {
                                        v.push((p.id.clone(), p.active_tab().path.clone()));
                                    });
                                    v
                                };
                                for (pid, path) in pane_paths {
                                    this.load_path(&pid, path, cx);
                                }
                                this.state.refresh_sidebar();
                            },
                        ))
                        // ── Fichiers système ───────────────────────────────────
                        .child(settings_row(
                            "Fichiers système",
                            "Afficher les fichiers et dossiers marqués comme système par l'OS",
                            show_system,
                            "toggle-system",
                            h_system,
                            theme,
                            |this, cx| {
                                this.show_system = !this.show_system;
                                let pane_paths: Vec<(String, String)> = {
                                    let mut v = Vec::new();
                                    this.state.layout.visit_panes(&mut |p| {
                                        v.push((p.id.clone(), p.active_tab().path.clone()));
                                    });
                                    v
                                };
                                for (pid, path) in pane_paths {
                                    this.load_path(&pid, path, cx);
                                }
                                this.state.refresh_sidebar();
                            },
                        ))
                        // ── Aperçus médias ─────────────────────────────────────
                        .child(settings_row(
                            "Aperçus médias",
                            "Afficher les aperçus des fichiers dans les info-bulles de la liste",
                            show_media_previews,
                            "toggle-media",
                            h_media,
                            theme,
                            |this, cx| {
                                this.show_media_previews = !this.show_media_previews;
                                cx.notify();
                            },
                        ))
                        // ── À PROPOS section ──────────────────────────────────
                        .child(
                            div()
                                .mt(px(16.0))
                                .pt(px(16.0))
                                .border_t_1()
                                .border_color(theme.border)
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_muted)
                                        .child("À PROPOS")
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(14.0))
                                        .child(
                                            img("icons/logo.png")
                                                .w(px(40.0))
                                                .h(px(40.0))
                                                .flex_none()
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(3.0))
                                                .child(
                                                    div()
                                                        .text_size(px(15.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme.text)
                                                        .child("Oxyde")
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(theme.text_muted)
                                                        .child(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(theme.text_muted)
                                                        .child("Explorateur de fichiers moderne pour Windows")
                                                )
                                        )
                                )
                        )
                )
        )
}

fn settings_row<F>(
    label: &str,
    description: &str,
    value: bool,
    toggle_id: &str,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    on_toggle: F,
) -> impl IntoElement
where
    F: Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
{
    let text_color = theme.text;
    let muted_color = theme.text_muted;
    let border_color: Hsla = rgba(0xffffff0au32).into();

    div()
        .py(px(10.0))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(border_color)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .mr(px(16.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(text_color)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(muted_color)
                        .child(description.to_string()),
                ),
        )
        .child(render_toggle(toggle_id, value, handle, on_toggle))
}

fn render_toggle<F>(
    id: &str,
    value: bool,
    handle: Entity<OxydeApp>,
    on_toggle: F,
) -> impl IntoElement
where
    F: Fn(&mut OxydeApp, &mut Context<OxydeApp>) + 'static,
{
    let on_bg: Hsla = rgba(0xF59E0BFFu32).into(); // amber-500
    let off_bg: Hsla = rgba(0x4B5563FFu32).into(); // gray-600
    let bg = if value { on_bg } else { off_bg };

    div()
        .id(SharedString::from(id.to_string()))
        .w(px(42.0))
        .h(px(22.0))
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .flex_none()
        .cursor_pointer()
        .when(value, |d| d.pl(px(22.0)))
        .when(!value, |d| d.pl(px(2.0)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |this, cx| {
                on_toggle(this, cx);
            });
        })
        .child(
            div()
                .size(px(18.0))
                .rounded_full()
                .bg(rgb(0xffffff))
                .flex_none(),
        )
}
