use super::*;

#[::core::prelude::v1::test]
fn standalone_and_composed_fields_share_the_entire_surface_style() {
    for id in crate::ui::theme::ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for focused in [false, true] {
            let mut standalone = input_surface(div(), InputAppearance::Standard, focused, false, &theme);
            let mut composed = standard_surface(div(), focused, &theme).px_3();
            assert_eq!(standalone.style(), composed.style());
        }
    }
}

#[::core::prelude::v1::test]
fn focus_only_changes_the_semantic_border_color() {
    for id in crate::ui::theme::ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        let mut idle = standard_surface(div(), false, &theme);
        let mut focused = standard_surface(div(), true, &theme);
        assert_eq!(idle.style().border_color, Some(theme.border));
        assert_eq!(focused.style().border_color, Some(theme.accent));
        idle.style().border_color = focused.style().border_color;
        assert_eq!(idle.style(), focused.style());
    }
}

#[::core::prelude::v1::test]
fn embedded_editors_do_not_add_a_second_border_or_surface() {
    let theme = OxydeTheme::dark();
    for appearance in [InputAppearance::Embedded, InputAppearance::Bare] {
        let mut idle = input_surface(div(), appearance, false, false, &theme);
        let mut focused = input_surface(div(), appearance, true, false, &theme);
        let mut empty = div();
        assert_eq!(idle.style().border_color, empty.style().border_color);
        assert_eq!(idle.style().border_widths, empty.style().border_widths);
        assert_eq!(idle.style().background, empty.style().background);
        assert_eq!(idle.style(), focused.style());
    }
}

#[::core::prelude::v1::test]
fn a_refused_text_takes_the_error_border_over_focus() {
    let theme = OxydeTheme::dark();
    for appearance in [InputAppearance::Standard, InputAppearance::Inline] {
        let mut refused = input_surface(div(), appearance, true, true, &theme);
        assert_eq!(refused.style().border_color, Some(theme.error));
    }
}

#[::core::prelude::v1::test]
fn inline_fields_share_the_standard_corners_and_text_margin() {
    let theme = OxydeTheme::dark();
    let mut inline = input_surface(div(), InputAppearance::Inline, false, false, &theme);
    let mut standard = input_surface(div(), InputAppearance::Standard, false, false, &theme);
    assert_eq!(inline.style().corner_radii, standard.style().corner_radii);
    assert_eq!(inline.style().padding, standard.style().padding);
}
