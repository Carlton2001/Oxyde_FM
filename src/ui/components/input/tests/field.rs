use super::*;

#[::core::prelude::v1::test]
fn standalone_and_composed_fields_share_the_entire_surface_style() {
    for id in crate::ui::theme::ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        for focused in [false, true] {
            let mut standalone = input_surface(div(), InputAppearance::Standard, focused, &theme);
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
        let mut idle = input_surface(div(), appearance, false, &theme);
        let mut focused = input_surface(div(), appearance, true, &theme);
        let mut empty = div();
        assert_eq!(idle.style().border_color, empty.style().border_color);
        assert_eq!(idle.style().border_widths, empty.style().border_widths);
        assert_eq!(idle.style().background, empty.style().background);
        assert_eq!(idle.style(), focused.style());
    }
}
