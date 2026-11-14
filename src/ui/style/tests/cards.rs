use super::*;
use crate::ui::scale::set_reference;

#[test]
fn grid_sizes_step_and_stay_at_their_ends() {
    assert_eq!(GridSize::Small.step(false), GridSize::Small);
    assert_eq!(GridSize::Small.step(true), GridSize::Medium);
    assert_eq!(GridSize::Medium.step(true), GridSize::Large);
    assert_eq!(GridSize::Large.step(true), GridSize::Large);
    assert_eq!(GridSize::Large.step(false), GridSize::Medium);
    for size in [GridSize::Small, GridSize::Medium, GridSize::Large] {
        assert_eq!(GridSize::from_config(size.config_value()), size);
    }
    assert_eq!(GridSize::from_config("unknown"), GridSize::Medium);
}

#[test]
fn card_contents_fit_their_rows_and_cells() {
    set_reference(16);
    for layout in [CardLayout::Grid(GridSize::Small), CardLayout::Grid(GridSize::Medium), CardLayout::Grid(GridSize::Large), CardLayout::Tiles] {
        let spec = layout.spec();
        assert!(spec.icon.0 <= spec.media.0);
        assert!(spec.thumbnail_height.0 <= spec.media.0);
        if let CardLayout::Grid(_) = layout {
            // The card is its cell less 0.5 rem, then its border and padding.
            let content = spec.min_width.0 - 0.5 - 2.0 * (CARD_BORDER.0 + GRID_CARD_PADDING_X.0);
            assert!(spec.thumbnail_width.0 <= content, "{layout:?}");
            // Shown at most at the 128 px the thumbnails are made at.
            assert!(rem_pixels(spec.thumbnail_width).as_f32() <= 128.0);
        } else {
            assert!(CARD_NAME_HEIGHT.0 + 2.0 * CARD_DETAIL_HEIGHT.0 <= spec.media.0);
        }
    }
    assert_eq!(CardLayout::Grid(GridSize::Medium).row_height(), 126.0);
    assert!(CardLayout::Grid(GridSize::Small).row_height() < CardLayout::Grid(GridSize::Medium).row_height());
    assert!(CardLayout::Grid(GridSize::Large).row_height() > CardLayout::Grid(GridSize::Medium).row_height());
}
