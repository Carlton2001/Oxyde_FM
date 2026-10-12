use crate::ui::app::OxydeApp;
use gpui::{App, Entity, WeakEntity, Window};

/// Continuity stays inside one region; moving to another restarts the delay.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum HoverRegion {
    Files(String),
    DirectoryTree,
}

#[derive(Clone)]
pub(super) struct Scope {
    pub(super) tag: String,
    pub(super) owner: WeakEntity<OxydeApp>,
    pub(super) region: HoverRegion,
}

impl Scope {
    pub(super) fn new(owner: &Entity<OxydeApp>, region: &HoverRegion, cx: &App) -> Option<Self> {
        let app = owner.read(cx);
        let tag = match region {
            HoverRegion::Files(pane) => {
                let tab = app.state.layout.find_pane(pane)?.active_tab();
                format!("{:?}|{pane}|{}|{}", owner.entity_id(), tab.id, tab.path)
            }
            HoverRegion::DirectoryTree => {
                if app.sidebar_collapsed {
                    return None;
                }
                format!("{:?}|directory-tree", owner.entity_id())
            }
        };
        Some(Self {
            tag,
            owner: owner.downgrade(),
            region: region.clone(),
        })
    }

    pub(super) fn same_region(&self, other: &Self) -> bool {
        self.owner.entity_id() == other.owner.entity_id() && self.region == other.region
    }

    pub(super) fn valid(&self, window: &Window, cx: &App) -> bool {
        let Some(owner) = self.owner.upgrade() else {
            return false;
        };
        let app = owner.read(cx);
        if !app.context_menus.is_empty()
            || app.drag_target.is_some()
            || !window.is_window_active()
            || !window.is_window_hovered()
        {
            return false;
        }
        Self::new(&owner, &self.region, cx).is_some_and(|current| current.tag == self.tag)
    }
}
