use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

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
        .flex_none()
        .flex()
        .items_center()
        .bg(theme.header)
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

pub(crate) fn action_button(
    id: impl Into<ElementId>,
    label: SharedString,
    primary: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = *cx.theme();
    div()
        .id(id)
        .tab_index(0)
        .flex()
        .items_center()
        .gap(rems(0.5))
        .px(rems(0.75))
        .py(rems(0.5))
        .rounded(rems(0.375))
        .bg(if primary { theme.accent } else { theme.header })
        .text_color(if primary {
            theme.text_on_accent
        } else {
            theme.text
        })
        .hover(move |s| {
            s.bg(if primary {
                theme.accent
            } else {
                theme.control_hover
            })
        })
        .focus_visible(move |s| s.border(rems(0.0625)).border_color(theme.accent))
        .child(label)
}
