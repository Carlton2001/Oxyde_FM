use super::{is_popup_scroll, DragTarget};

#[test]
fn popup_pointer_updates_only_menu_scrolls_and_never_background_drags() {
    assert!(!is_popup_scroll(None));
    assert!(!is_popup_scroll(Some(&DragTarget::SidebarResize)));
    assert!(!is_popup_scroll(Some(&DragTarget::FileListV {
        pane_id: "pane".into(), viewport_h: 100.0, content_h: 500.0, track_id: "files".into(),
    })));
    assert!(is_popup_scroll(Some(&DragTarget::ContextMenuV {
        menu_idx: 0, viewport_h: 100.0, content_h: 500.0, track_id: "menu".into(),
    })));
    assert!(is_popup_scroll(Some(&DragTarget::BreadcrumbV {
        viewport_h: 100.0, content_h: 500.0, track_id: "breadcrumb".into(),
    })));
}
