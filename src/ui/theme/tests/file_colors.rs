use super::*;
use crate::ui::theme::{ThemeId, ThemeSeed};

fn derived(id: ThemeId) -> FileIconPalette {
    derived_with(id, ColorVision::Typical)
}

fn derived_with(id: ThemeId, vision: ColorVision) -> FileIconPalette {
    let seed = ThemeSeed::for_id(id);
    derive([seed.background, seed.active_background], seed.accent, id.is_light(), vision)
}

fn entries(palette: FileIconPalette) -> [(&'static str, Hsla); 7] {
    [("folder", palette.folder), ("archive", palette.archive), ("image", palette.image),
     ("audio", palette.audio), ("video", palette.video), ("pdf", palette.pdf), ("code", palette.code)]
}

#[test]
fn oklch_round_trips_displayable_colors() {
    for color in [0x000000, 0xffffff, 0x3d5af1, 0xd4a54a, 0x087f5b, 0xf92672] {
        assert_eq!(from_oklch(to_oklch(color)), color, "{color:06x}");
    }
}

#[test]
fn derived_icons_stay_legible_on_both_panes() {
    for id in ThemeId::ALL {
        let seed = ThemeSeed::for_id(id);
        for (name, color) in ColorVision::ALL.into_iter().flat_map(|vision| entries(derived_with(id, vision))) {
            for pane in [seed.background, seed.active_background] {
                let ratio = contrast(color, rgb(pane).into());
                assert!(ratio >= ICON_CONTRAST - 0.01, "{id:?} {name}: {ratio}");
            }
        }
    }
}

#[test]
fn categories_turn_away_from_the_accent() {
    // Oxyde Dark's gold accent sits on the archive hue.
    let palette = derived(ThemeId::Dark);
    let folder = to_oklch(u32::from_str_radix(&hex(palette.folder)[1..], 16).unwrap());
    let archive = to_oklch(u32::from_str_radix(&hex(palette.archive)[1..], 16).unwrap());
    assert!(hue_distance(folder.h, archive.h) >= ACCENT_SEPARATION - 1.0);
}

/// `cargo test --offline print_derived_file_colors -- --ignored --nocapture`
#[test]
#[ignore]
fn print_derived_file_colors() {
    for vision in ColorVision::ALL {
        for id in ThemeId::ALL {
            let line = entries(derived_with(id, vision)).map(|(name, color)| format!("{name}={}", hex(color))).join(" ");
            println!("DERIVED {} {id:?} {line}", vision.as_str());
        }
    }
}
