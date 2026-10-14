use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Resize content-sized tool windows after layout, avoiding layout re-entry.
pub(crate) fn content_height_limit(window: &Window, cx: &App) -> Pixels {
    window.display(cx).map(|display| (display.visible_bounds().size.height
        - rems(5.0).to_pixels(window.rem_size())).max(px(100.0)))
        .unwrap_or(px(900.0))
}

pub(crate) fn fit_content_height(height: Pixels, window: &mut Window, cx: &App) {
    fit_content_height_at_width(height, window.viewport_size().width, window, cx);
}

pub(crate) fn fit_content_height_at_width(height: Pixels, width: Pixels, window: &mut Window, cx: &App) {
    let target = height.ceil().min(content_height_limit(window, cx));
    if ((window.viewport_size().height - target).abs() > px(1.0)
        || (window.viewport_size().width - width).abs() > px(1.0)) && target > px(0.0) {
        window.on_next_frame(move |window, _| {
            window.resize(size(width, target));
        });
    }
}

/// Rasterize the official SVG without changing its geometry, for WM_SETICON.
fn native_tool_icon(path: &str, cx: &App) -> Option<Vec<u8>> {
    let bytes = cx.asset_source().load(path).ok()??;
    let color: Rgba = cx.theme().accent.into();
    let stroke = format!(
        "#{:02x}{:02x}{:02x}",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8
    );
    let svg = std::str::from_utf8(&bytes)
        .ok()?
        .replace("currentColor", &stroke);
    let renderer = cx.svg_renderer();
    let parsed = renderer.parse_svg(svg.as_bytes()).ok()?;
    let rendered = renderer
        .render_parsed(
            &parsed,
            SvgSize::ExactSize(size(DevicePixels(48), DevicePixels(48))),
        )
        .ok()?;
    let mut rgba = rendered.as_bytes(0)?.to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(48, 48, rgba)?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut png, image::ImageFormat::Png)
        .ok()?;
    Some(png.into_inner())
}

/// Use the same official Lucide asset for the title and the native taskbar preview.
pub(crate) fn install_icon(path: &'static str, window: &mut Window) {
    window.on_next_frame(move |window, cx| {
        let icon = native_tool_icon(path, cx).or_else(|| {
            cx.asset_source()
                .load("icons/logo.png")
                .ok()
                .flatten()
                .map(|bytes| bytes.into_owned())
        });
        if let (Some(icon), Ok(handle)) = (
            icon,
            raw_window_handle::HasWindowHandle::window_handle(window),
        ) {
            if let Err(error) = oxyde_lib::platform::platform().install_pointer_capture(handle.as_raw()) {
                log::warn!("Cannot install tool pointer capture: {error}");
            }
            oxyde_lib::platform::platform().set_application_icon(handle.as_raw(), &icon);
        }
    });
}

pub(crate) fn header(
    icon: &'static str,
    title: SharedString,
    maximized: bool,
    allow_resize: bool,
    cx: &App,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = *cx.theme();
    div()
        .h(rems(2.25))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .bg(theme.surface)
        .border_b(rems(0.0625))
        .border_color(theme.border)
        .child(
            div().px(rems(0.75)).child(
                svg()
                    .path(icon)
                    .size(rems(1.0))
                    .text_color(theme.text_muted),
            ),
        )
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .window_control_area(WindowControlArea::Drag)
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(super::title_bar::render_window_controls_with_options(
            maximized,
            allow_resize,
            cx,
            on_close,
        ))
}

/// Shared secondary-window footer, matching the recycle-bin properties modal.
pub(crate) fn footer(cx: &App) -> Div {
    div()
        .flex_none()
        .px(rems(1.0))
        .py(rems(0.625))
        .border_t(rems(0.0625))
        .border_color(cx.theme().border)
        .flex()
        .items_center()
        .justify_end()
        .gap(rems(0.5))
}

pub(crate) fn action_button(
    id: impl Into<ElementId>,
    label: SharedString,
    primary: bool,
    cx: &App,
) -> Stateful<Div> {
    use crate::ui::elements::button::{button_style, ButtonVariant};
    button_style(div().id(id), *cx.theme(),
        if primary { ButtonVariant::Primary } else { ButtonVariant::Secondary }, true)
        .tab_index(0).flex().items_center().justify_center()
        .gap(rems(0.5)).px(rems(0.75)).py(rems(0.375))
        .text_size(rems(0.8125))
        .rounded(rems(0.375))
        // Callers may supply icon/text children instead of the built-in label.
        // An empty text child still introduces an extra flex gap before them.
        .when(!label.is_empty(), |button| button.child(label))
}

/// Retire owned surfaces before their native owner is destroyed.
pub(crate) fn close_transients(window: &mut Window, cx: &mut App) {
    crate::ui::components::popup::close_for(window, cx);
    crate::ui::components::tooltip::hover::close_for(window, cx);
}
