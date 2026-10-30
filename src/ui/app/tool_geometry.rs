use super::OxydeApp;
use gpui::*;

fn owner(cx: &App) -> Option<Entity<OxydeApp>> {
    cx.windows().into_iter().find_map(|handle| {
        handle
            .downcast::<OxydeApp>()
            .and_then(|handle| handle.entity(cx).ok())
    })
}

/// Restore position by surface kind; content-sized forms keep their current size.
pub(crate) fn restore(key: &str, fallback: Bounds<Pixels>, cx: &App) -> WindowBounds {
    restore_with(key, fallback, false, cx)
}

/// Freely resized windows also restore their saved size and maximized state.
pub(crate) fn restore_sized(key: &str, fallback: Bounds<Pixels>, cx: &App) -> WindowBounds {
    restore_with(key, fallback, true, cx)
}

fn restore_with(key: &str, fallback: Bounds<Pixels>, sized: bool, cx: &App) -> WindowBounds {
    let saved = owner(cx).and_then(|owner| {
        owner
            .read(cx)
            .config
            .tool_window_placements
            .get(key)
            .copied()
    });
    let Some(mut saved) = saved else {
        return WindowBounds::Windowed(fallback);
    };
    if !sized {
        saved.width = fallback.size.width.as_f32();
        saved.height = fallback.size.height.as_f32();
        saved.maximized = false;
    }
    let screens = cx
        .displays()
        .iter()
        .map(|display| display.visible_bounds())
        .collect::<Vec<_>>();
    let screens = crate::ui::window_geometry::restorable_screens(screens);
    crate::ui::window_geometry::restore(Some(saved), &screens, Size::default())
}

pub(crate) fn observe(key: &'static str, window: &mut Window, cx: &mut App) {
    let Some(owner) = owner(cx) else {
        return;
    };
    owner.update(cx, |_, cx| {
        let subscription = cx.observe_window_bounds(window, move |app, window, cx| {
            let Some(placement) = crate::ui::window_geometry::capture(window) else {
                return;
            };
            if app.config.tool_window_placements.get(key) == Some(&placement) {
                return;
            }
            app.config
                .tool_window_placements
                .insert(key.to_string(), placement);
            app.window_geometry_save_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(350))
                    .await;
                let _ = this.update(cx, |app, cx| app.save_preferences(cx));
            }));
        });
        subscription.detach();
    });
}
