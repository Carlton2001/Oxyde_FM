use crate::ui::app::OxydeApp;
use super::ThemeContext;
use gpui::prelude::*;
use gpui::*;

impl OxydeApp {
    pub(crate) fn render_pane(
        &self,
        pane: &crate::ui::workspace::PaneState,
        handle: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::ui::components::file_list::render_file_list;
        use crate::ui::components::top_bar::render_top_bar;

        let p_id = pane.id.clone();
        let theme = *cx.theme();

        div()
            .id(SharedString::from(format!(
                "pane-container-{}",
                p_id.clone()
            )))
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(self.render_pane_tabs(pane, handle.clone(), cx))
            .child({
                let content = div()
                    .id(SharedString::from(format!("pane-content-{}", p_id.clone())))
                    .size_full()
                    .flex()
                    .flex_col()
                    .relative()
                    .child(render_top_bar(self, p_id.clone(), handle.clone(), cx))
                    .when(self.state.find_pane(&p_id).is_some_and(|pane| pane.active_tab().quick_search.is_some()), |pane| pane.child(self.render_quick_search_banner(&p_id, cx)))
                    .child(render_file_list(self, p_id.clone(), handle.clone(), cx));
                self.append_pane_split_drop_zones(content, p_id.clone(), handle.clone(), theme)
            })
            .into_any_element()
    }
}

