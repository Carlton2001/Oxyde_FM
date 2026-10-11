use crate::ui::app::OxydeApp;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(super) fn append_pane_split_drop_zones(
        &self,
        content: Stateful<Div>,
        p_id: String,
        handle: Entity<Self>,
        theme: OxydeTheme,
    ) -> Stateful<Div> {
        let h = handle.clone();
        content
            // Visual highlights for splits
            .when(self.highlight_split_pane == Some(p_id.clone()), |s| {
                s.child(
                    div()
                        .absolute()
                        .size_full()
                        .when(self.highlight_left_zone, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .h_full()
                                    .w_1_2()
                                    .bg(theme.selected)
                                    .border_r(rems(0.0625))
                                    .border_color(theme.accent),
                            )
                        })
                        .when(self.highlight_right_zone, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .h_full()
                                    .w_1_2()
                                    .bg(theme.selected)
                                    .border_l(rems(0.0625))
                                    .border_color(theme.accent),
                            )
                        })
                        .when(self.highlight_top_zone, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .w_full()
                                    .h_1_2()
                                    .bg(theme.selected)
                                    .border_b(rems(0.0625))
                                    .border_color(theme.accent),
                            )
                        })
                        .when(self.highlight_bottom_zone, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .w_full()
                                    .h_1_2()
                                    .bg(theme.selected)
                                    .border_t(rems(0.0625))
                                    .border_color(theme.accent),
                            )
                        }),
                )
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
