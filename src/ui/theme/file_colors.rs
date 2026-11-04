//! File icon colors computed from a theme's seeds, for user themes.
//! Each category keeps Oxyde's hue; lightness follows the background, chroma follows the
//! accent, and folders take the accent. Work happens in OKLCH, where lightness is perceptual.

use super::legibility::contrast;
use super::ColorVision;
use crate::ui::components::icons::FileIconPalette;
use gpui::{rgb, Hsla};

#[cfg(test)]
#[path = "tests/file_colors.rs"]
mod tests;

/// Minimum contrast of an icon against both file panes: WCAG's floor for graphics.
pub(super) const ICON_CONTRAST: f32 = 3.0;
/// A category this close to the accent hue turns away from it, so folders stay apart.
const ACCENT_SEPARATION: f32 = 30.0;

/// Hue (degrees) and chroma at the reference saturation.
/// `lightness` shifts a category from the theme's icon lightness. Red and ocre, or green,
/// look alike to most color-blind eyes, so these pairs also differ in lightness.
struct Category {
    hue: f32,
    chroma: f32,
    lightness: f32,
}

/// Order: archive, image, audio, video, PDF, code.
type Categories = [Category; 6];

const fn category(hue: f32, chroma: f32, lightness: f32) -> Category {
    Category { hue, chroma, lightness }
}

/// Oxyde's code colors, archive and PDF already apart in lightness.
const TYPICAL: Categories = [
    category(85.0, 0.12, 0.08),
    category(142.0, 0.14, 0.02),
    category(300.0, 0.17, 0.0),
    category(355.0, 0.18, 0.0),
    category(30.0, 0.18, -0.09),
    category(210.0, 0.11, 0.0),
];

/// Protan and deutan vision keeps the blue-yellow axis and lightness: archives light
/// yellow-green, images teal, audio dark pink, video orange, PDF violet, code sky blue.
/// Searched to keep categories apart under a full deuteranopia and protanopia simulation.
const RED_GREEN: Categories = [
    category(130.0, 0.13, 0.18),
    category(168.0, 0.063, -0.025),
    category(325.0, 0.07, -0.18),
    category(61.0, 0.146, -0.137),
    category(282.0, 0.2, -0.18),
    category(247.0, 0.084, 0.122),
];

/// Tritan vision keeps the red-cyan axis and lightness: archives light yellow, images
/// green, audio dark violet, video red, PDF dark pink, code dark blue.
/// Searched to keep categories apart under a full tritanopia simulation.
const BLUE_YELLOW: Categories = [
    category(72.0, 0.11, 0.18),
    category(115.0, 0.177, -0.001),
    category(305.0, 0.2, -0.179),
    category(34.0, 0.194, -0.061),
    category(337.0, 0.105, -0.079),
    category(253.0, 0.193, -0.18),
];

fn categories(vision: ColorVision) -> &'static Categories {
    match vision {
        ColorVision::Typical => &TYPICAL,
        ColorVision::RedGreen => &RED_GREEN,
        ColorVision::BlueYellow => &BLUE_YELLOW,
    }
}

/// Accent chroma at which categories keep their reference chroma.
const REFERENCE_CHROMA: f32 = 0.17;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

fn linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn gamma(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

pub(super) fn to_oklch(color: u32) -> Oklch {
    let channel = |shift: u32| linear(((color >> shift) & 0xff) as f32 / 255.0);
    let (r, g, b) = (channel(16), channel(8), channel(0));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    Oklch { l: lightness, c: a.hypot(b), h: b.atan2(a).to_degrees().rem_euclid(360.0) }
}

/// Linear sRGB, possibly outside [0, 1].
fn oklch_to_linear(color: Oklch) -> [f32; 3] {
    let (a, b) = (color.c * color.h.to_radians().cos(), color.c * color.h.to_radians().sin());
    let l = (color.l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (color.l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (color.l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

/// The nearest displayable color: chroma drops until it fits sRGB, hue and lightness stay.
pub(super) fn from_oklch(color: Oklch) -> u32 {
    let fits = |c: f32| oklch_to_linear(Oklch { c, ..color }).iter().all(|v| (-0.0001..=1.0001).contains(v));
    let (mut low, mut high) = (0.0_f32, color.c);
    if !fits(high) {
        for _ in 0..20 {
            let middle = (low + high) / 2.0;
            if fits(middle) { low = middle } else { high = middle }
        }
        high = low;
    }
    let byte = |v: f32| (gamma(v.clamp(0.0, 1.0)) * 255.0).round() as u32;
    let [r, g, b] = oklch_to_linear(Oklch { c: high, ..color });
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

fn hsla(color: u32) -> Hsla {
    rgb(color).into()
}

/// Smallest lightness change, away from the panes, reaching `ICON_CONTRAST` on all of them.
fn legible(color: Oklch, panes: &[Hsla], light: bool) -> u32 {
    let worst = |c: u32| panes.iter().map(|pane| contrast(hsla(c), *pane)).fold(f32::INFINITY, f32::min);
    let step = if light { -0.01 } else { 0.01 };
    (0..=60)
        .map(|n| from_oklch(Oklch { l: (color.l + step * n as f32).clamp(0.0, 1.0), ..color }))
        .find(|candidate| worst(*candidate) >= ICON_CONTRAST)
        .unwrap_or_else(|| from_oklch(Oklch { l: if light { 0.3 } else { 0.9 }, ..color }))
}

fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Turn `hue` away from `accent` until they are `ACCENT_SEPARATION` apart, on the side
/// leaving more room to the other categories.
fn away_from(hue: f32, accent: f32, all: &Categories) -> f32 {
    if hue_distance(hue, accent) >= ACCENT_SEPARATION {
        return hue;
    }
    let room = |candidate: f32| all.iter().map(|other| other.hue).filter(|other| *other != hue)
        .map(|other| hue_distance(candidate, other)).fold(f32::INFINITY, f32::min);
    let [up, down] = [1.0, -1.0].map(|side| (accent + side * ACCENT_SEPARATION).rem_euclid(360.0));
    if room(up) >= room(down) { up } else { down }
}

/// `panes` are the two file pane backgrounds; `light` tells which side lightness moves to.
pub(super) fn derive(panes: [u32; 2], accent: u32, light: bool, vision: ColorVision) -> FileIconPalette {
    let all = categories(vision);
    let panes = panes.map(hsla);
    let accent = to_oklch(accent);
    // A gray accent says nothing about saturation: keep the reference chroma.
    let scale = if accent.c < 0.03 { 1.0 } else { (accent.c / REFERENCE_CHROMA).clamp(0.6, 1.2) };
    let lightness = if light { 0.55 } else { 0.74 };
    let color = |category: &Category| {
        let hue = if accent.c < 0.03 { category.hue } else { away_from(category.hue, accent.h, all) };
        hsla(legible(Oklch { l: lightness + category.lightness, c: category.chroma * scale, h: hue }, &panes, light))
    };
    FileIconPalette {
        folder: hsla(legible(accent, &panes, light)),
        archive: color(&all[0]),
        image: color(&all[1]),
        audio: color(&all[2]),
        video: color(&all[3]),
        pdf: color(&all[4]),
        code: color(&all[5]),
    }
}

/// `#rrggbb` of a palette entry, for tests and comparisons.
#[cfg(test)]
pub(super) fn hex(color: Hsla) -> String {
    let color: gpui::Rgba = color.into();
    let byte = |v: f32| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(color.r), byte(color.g), byte(color.b))
}
