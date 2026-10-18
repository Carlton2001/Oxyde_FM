use crate::ui::app::OxydeApp;
use crate::ui::elements::motion;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

#[derive(Clone, Copy, PartialEq)]
enum SplitZone {
    Left,
    Right,
    Top,
    Bottom,
}

impl SplitZone {
    /// Fractions of the pane covered by the highlight: left, top, width, height.
    fn rect(self, reach: f32) -> [f32; 4] {
        let half = 0.5 * reach;
        match self {
            Self::Left => [0.0, 0.0, half, 1.0],
            Self::Right => [1.0 - half, 0.0, half, 1.0],
            Self::Top => [0.0, 0.0, 1.0, half],
            Self::Bottom => [0.0, 1.0 - half, 1.0, half],
        }
    }
}

/// Half-pane highlight that grows from the targeted edge and glides to the next zone.
fn split_highlight(pane_id: &str, zone: SplitZone, visible: bool, theme: OxydeTheme) -> impl IntoElement {
    let [left, top, width, height] = zone.rect(1.0);
    let [from_left, from_top, from_width, from_height] = zone.rect(0.85);
    let id = |part: &str| SharedString::from(format!("split-highlight-{part}-{pane_id}"));
    let border = rems(0.0625);
    div()
        .absolute()
        .bg(theme.selected)
        .border_color(theme.accent)
        .map(|rect| match zone {
            SplitZone::Left => rect.border_r(border),
            SplitZone::Right => rect.border_l(border),
            SplitZone::Top => rect.border_b(border),
            SplitZone::Bottom => rect.border_t(border),
        })
        .with_spring(id("left"), motion::slide(left).from(from_left), |rect, v| rect.left(relative(v)))
        .with_spring(id("top"), motion::slide(top).from(from_top), |rect, v| {
            rect.map_element(|rect| rect.top(relative(v)))
        })
        .with_spring(id("width"), motion::slide(width).from(from_width), |rect, v| {
            rect.map_element(|rect| rect.map_element(|rect| rect.w(relative(v))))
        })
        .with_spring(id("height"), motion::slide(height).from(from_height), |rect, v| {
            rect.map_element(|rect| rect.map_element(|rect| rect.map_element(|rect| rect.h(relative(v)))))
        })
        .with_spring(id("presence"), motion::presence(visible).from(0.0), |rect, v| {
            rect.map_element(|rect| {
                rect.map_element(|rect| rect.map_element(|rect| rect.map_element(|rect| rect.opacity(v))))
            })
        })
}

impl OxydeApp {
    /// Zone to highlight in this pane and whether it is targeted, while it is visible or fading out.
    fn split_highlight(&self, pane_id: &str) -> Option<(SplitZone, bool)> {
        let visible = self.highlight_split_pane.as_deref() == Some(pane_id);
        let fading = self.dragging_tab.is_some() && self.split_highlight_last.as_deref() == Some(pane_id);
        if !visible && !fading {
            return None;
        }
        let zone = if self.highlight_left_zone {
            SplitZone::Left
        } else if self.highlight_right_zone {
            SplitZone::Right
        } else if self.highlight_top_zone {
            SplitZone::Top
        } else if self.highlight_bottom_zone {
            SplitZone::Bottom
        } else {
            return None;
        };
        Some((zone, visible))
    }

    pub(super) fn append_pane_split_drop_zones(
        &self,
        content: Stateful<Div>,
        p_id: String,
        handle: Entity<Self>,
        theme: OxydeTheme,
    ) -> Stateful<Div> {
        let h = handle.clone();
        content
            // Visual highlight for splits, gliding between zones while a tab is dragged.
            .when_some(self.split_highlight(&p_id), |s, (zone, visible)| {
                s.child(split_highlight(&p_id, zone, visible, theme))
            })
            // Split detection zones
            .child(
                div()
                    .absolute()
                    .left_0()
                    .h_full()
                    .w_1_4()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.split_highlight_last = Some(p_id_cap.clone());
                                v.highlight_left_zone = true;
                                v.highlight_right_zone = false;
                                v.highlight_top_zone = false;
                                v.highlight_bottom_zone = false;

                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(
                                    &f_p,
                                    f_t,
                                    &p_id_cap,
                                    crate::ui::workspace::LayoutAxis::Horizontal,
                                    false,
                                );
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.reconcile_watchers(cx);
                                this.save_workspace(cx);
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                div()
                    .absolute()
                    .right_0()
                    .h_full()
                    .w_1_4()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.split_highlight_last = Some(p_id_cap.clone());
                                v.highlight_right_zone = true;
                                v.highlight_left_zone = false;
                                v.highlight_top_zone = false;
                                v.highlight_bottom_zone = false;

                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(
                                    &f_p,
                                    f_t,
                                    &p_id_cap,
                                    crate::ui::workspace::LayoutAxis::Horizontal,
                                    true,
                                );
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.reconcile_watchers(cx);
                                this.save_workspace(cx);
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .w_full()
                    .h_1_4()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.split_highlight_last = Some(p_id_cap.clone());
                                v.highlight_top_zone = true;
                                v.highlight_left_zone = false;
                                v.highlight_right_zone = false;
                                v.highlight_bottom_zone = false;

                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(
                                    &f_p,
                                    f_t,
                                    &p_id_cap,
                                    crate::ui::workspace::LayoutAxis::Vertical,
                                    false,
                                );
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.reconcile_watchers(cx);
                                this.save_workspace(cx);
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .h_1_4()
                    .drag_over::<(String, usize)>({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |style, _, _, app| {
                            h.update(app, |v, _| {
                                v.highlight_split_pane = Some(p_id_cap.clone());
                                v.split_highlight_last = Some(p_id_cap.clone());
                                v.highlight_bottom_zone = true;
                                v.highlight_left_zone = false;
                                v.highlight_right_zone = false;
                                v.highlight_top_zone = false;

                                // Reset tab targeting when entering a split zone
                                v.drop_target_idx = None;
                                v.drop_target_pane = None;
                            });
                            style
                        }
                    })
                    .on_drop({
                        let h = h.clone();
                        let p_id_cap = p_id.clone();
                        move |(from_p, f_t): &(String, usize), _, app| {
                            let (f_p, f_t) = (from_p.clone(), *f_t);
                            h.update(app, |this, cx| {
                                this.state.split_tab_to_axis(
                                    &f_p,
                                    f_t,
                                    &p_id_cap,
                                    crate::ui::workspace::LayoutAxis::Vertical,
                                    true,
                                );
                                this.highlight_split_pane = None;
                                this.dragging_tab = None;
                                this.drop_target_idx = None;
                                this.drop_target_pane = None;
                                this.reconcile_watchers(cx);
                                this.save_workspace(cx);
                                cx.notify();
                            });
                        }
                    }),
            )
    }
}
