use super::{Side, buttons, zone_pixels, ADWAITA_DISC, ADWAITA_GAP, BREEZE_DISC, BREEZE_GAP};
use gpui::{WindowButton, WindowButtonLayout};

const MIN: Option<WindowButton> = Some(WindowButton::Minimize);
const MAX: Option<WindowButton> = Some(WindowButton::Maximize);
const CLOSE: Option<WindowButton> = Some(WindowButton::Close);

#[test]
fn without_a_preference_the_usual_three_show_on_the_right() {
    assert_eq!(buttons(None, Side::Right, true), [WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close]);
    assert!(buttons(None, Side::Left, true).is_empty());
}

#[test]
fn each_side_keeps_the_desktop_choice() {
    let layout = WindowButtonLayout { left: [CLOSE, None, None], right: [MIN, None, None] };
    assert_eq!(buttons(Some(layout), Side::Left, true), [WindowButton::Close]);
    assert_eq!(buttons(Some(layout), Side::Right, true), [WindowButton::Minimize]);
}

#[test]
fn fixed_windows_only_keep_close() {
    let layout = WindowButtonLayout { left: [None; 3], right: [MIN, MAX, CLOSE] };
    assert_eq!(buttons(Some(layout), Side::Right, false), [WindowButton::Close]);
}

#[test]
fn hit_zones_stay_even_so_their_discs_fall_on_whole_pixels() {
    for rem in [12, 15, 16, 18, 20, 24] {
        crate::ui::scale::set_reference(rem);
        for (disc, gap) in [(ADWAITA_DISC, ADWAITA_GAP), (BREEZE_DISC, BREEZE_GAP)] {
            let zone = zone_pixels(disc, gap).as_f32();
            assert_eq!(zone % 2.0, 0.0, "{zone} px at {rem} px/rem");
        }
    }
    crate::ui::scale::set_reference(crate::ui::scale::BASE_REM_SIZE);
}
