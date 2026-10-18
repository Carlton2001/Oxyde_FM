use super::*;

#[::core::prelude::v1::test]
fn action_variants_reserve_identical_strokes_without_changing_padding() {
    for id in crate::ui::theme::ThemeId::ALL {
        let theme = OxydeTheme::for_id(id);
        let mut reference = button_style(div().id("reference"), theme, ButtonVariant::Secondary, true);
        for variant in [ButtonVariant::Ghost, ButtonVariant::Primary, ButtonVariant::Danger] {
            for enabled in [true, false] {
                let mut button = button_style(div().id("action"), theme, variant, enabled);
                assert_eq!(button.style().border_widths, reference.style().border_widths);
                assert_eq!(button.style().padding, reference.style().padding);
                assert_eq!(button.style().size, reference.style().size);
            }
        }
    }
}
