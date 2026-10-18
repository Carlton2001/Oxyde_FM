use gpui::{point, px, size, Bounds, Pixels, Size, Window, WindowBounds, WindowVisibility};
use oxyde_lib::models::config::WindowPlacement;

pub(crate) fn capture(window: &Window) -> Option<WindowPlacement> {
    // Minimized native placement can report a windowed state: keep the last
    // visible maximized/restorable state instead of replacing it on minimize.
    if window.visibility() == WindowVisibility::Hidden {
        return None;
    }
    let state = window.window_bounds();
    let bounds = state.get_bounds();
    let placement = WindowPlacement {
        x: bounds.origin.x.as_f32(),
        y: bounds.origin.y.as_f32(),
        width: bounds.size.width.as_f32(),
        height: bounds.size.height.as_f32(),
        maximized: matches!(state, WindowBounds::Maximized(_)),
    };
    valid(&placement).then_some(placement)
}

fn valid(placement: &WindowPlacement) -> bool {
    [placement.x, placement.y, placement.width, placement.height]
        .iter()
        .all(|value| value.is_finite())
        && placement.width >= 200.0
        && placement.height >= 100.0
        && placement.width <= 65536.0
        && placement.height <= 65536.0
        && placement.x.abs() < 100000.0
        && placement.y.abs() < 100000.0
}

/// `min` matches the window's minimum size, so a smaller saved size cannot
/// reopen below it; the screen still bounds both.
pub(crate) fn restore(
    saved: Option<WindowPlacement>,
    screens: &[Bounds<Pixels>],
    min: Size<Pixels>,
) -> WindowBounds {
    let saved = saved.filter(valid);
    let fallback = Bounds::new(point(px(100.0), px(100.0)), size(px(1200.0), px(800.0)));
    let requested = saved
        .map(|p| Bounds::new(point(px(p.x), px(p.y)), size(px(p.width), px(p.height))))
        .unwrap_or(fallback);
    // Pick the screen with the largest intersection, retaining negative desktop
    // coordinates. If a monitor disappeared, use the primary screen (first).
    let screen = screens
        .iter()
        .max_by(|a, b| intersection(requested, **a).total_cmp(&intersection(requested, **b)))
        .filter(|screen| intersection(requested, **screen) > 0.0)
        .or_else(|| screens.first());
    let bounds = if let Some(screen) = screen {
        let width = requested.size.width.max(min.width).min(screen.size.width);
        let height = requested.size.height.max(min.height).min(screen.size.height);
        let x = requested
            .origin
            .x
            .max(screen.origin.x)
            .min(screen.origin.x + screen.size.width - width);
        let y = requested
            .origin
            .y
            .max(screen.origin.y)
            .min(screen.origin.y + screen.size.height - height);
        Bounds::new(point(x, y), size(width, height))
    } else {
        requested
    };
    if saved.is_some_and(|p| p.maximized) {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

fn intersection(a: Bounds<Pixels>, b: Bounds<Pixels>) -> f32 {
    let width =
        (a.origin.x + a.size.width).min(b.origin.x + b.size.width) - a.origin.x.max(b.origin.x);
    let height =
        (a.origin.y + a.size.height).min(b.origin.y + b.size.height) - a.origin.y.max(b.origin.y);
    width.as_f32().max(0.0) * height.as_f32().max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_round_trip_preserves_secondary_monitor_and_maximized_restore_size() {
        let saved = WindowPlacement {
            x: -1100.0,
            y: 50.0,
            width: 800.0,
            height: 600.0,
            maximized: true,
        };
        let saved = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let screens = [
            Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1040.0))),
            Bounds::new(point(px(-1280.0), px(0.0)), size(px(1280.0), px(984.0))),
        ];
        assert_eq!(
            restore(Some(saved), &screens, Size::default()),
            WindowBounds::Maximized(Bounds::new(
                point(px(-1100.0), px(50.0)),
                size(px(800.0), px(600.0))
            ))
        );
    }
    #[test]
    fn disconnected_screen_and_invalid_geometry_return_to_visible_area() {
        let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(1024.0), px(728.0)));
        let mut saved = WindowPlacement {
            x: 3000.0,
            y: -100.0,
            width: 1600.0,
            height: 900.0,
            maximized: false,
        };
        assert_eq!(
            restore(Some(saved), &[screen], Size::default()),
            WindowBounds::Windowed(screen)
        );
        saved.width = f32::NAN;
        assert_eq!(
            restore(Some(saved), &[screen], Size::default()),
            WindowBounds::Windowed(screen)
        );
    }
    #[test]
    fn saved_size_below_minimum_reopens_at_minimum() {
        let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1040.0)));
        let saved = WindowPlacement { x: 100.0, y: 100.0, width: 300.0, height: 200.0, maximized: false };
        assert_eq!(
            restore(Some(saved), &[screen], size(px(720.0), px(480.0))),
            WindowBounds::Windowed(Bounds::new(point(px(100.0), px(100.0)), size(px(720.0), px(480.0))))
        );
    }
}
