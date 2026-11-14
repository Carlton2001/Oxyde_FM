//! Card geometry of the grid, at its three sizes, and of the tiles. Every measure of
//! a card (scrolling, rectangle selection, the keyboard, groups, the file chooser)
//! comes from here, so the layout and the hit tests cannot disagree.
use crate::ui::scale::rem_pixels;
use gpui::{rems, Rems};
use std::cell::Cell;

/// The grid's card size, one for the whole application, changed with Ctrl and the wheel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl GridSize {
    pub fn from_config(value: &str) -> Self {
        match value {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }

    /// The default writes nothing, so configurations keep only a chosen size.
    pub fn config_value(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "",
            Self::Large => "large",
        }
    }

    /// The next size up or down, staying at the ends.
    pub fn step(self, larger: bool) -> Self {
        match (self, larger) {
            (Self::Small, true) | (Self::Large, false) => Self::Medium,
            (Self::Medium | Self::Large, true) => Self::Large,
            (Self::Medium | Self::Small, false) => Self::Small,
        }
    }
}

thread_local! {
    // Read by layout and hit testing on the UI thread, synchronized from the
    // preferences before rendering, as the rem reference is.
    static GRID_SIZE: Cell<GridSize> = const { Cell::new(GridSize::Medium) };
}

pub fn set_grid_size(size: GridSize) {
    GRID_SIZE.with(|current| current.set(size));
}

pub fn grid_size() -> GridSize {
    GRID_SIZE.with(Cell::get)
}

/// How a card view lays its entries out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardLayout {
    /// Icon or thumbnail above the name and the type.
    Grid(GridSize),
    /// Icon or thumbnail beside the name, the type and a detail line.
    Tiles,
}

/// The authored measures of a card, in rems.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardSpec {
    /// Narrowest cell: the columns are as many as fit, then share the width.
    pub min_width: Rems,
    pub row_height: Rems,
    /// Height of the icon area (grid) or side of the icon square (tiles).
    pub media: Rems,
    pub icon: Rems,
    pub thumbnail_width: Rems,
    pub thumbnail_height: Rems,
}

/// Room around a card inside its cell: 0.125 rem of margin on each side.
pub const CARD_MARGIN: Rems = rems(0.125);
pub const CARD_BORDER: Rems = rems(0.0625);
/// The grid card's horizontal padding, kept small so the name gets the width.
pub const GRID_CARD_PADDING_X: Rems = rems(0.125);
pub const GRID_CARD_PADDING_Y: Rems = rems(0.25);
/// Space between the icon area, the name and the type.
pub const GRID_CARD_GAP: Rems = rems(0.125);
pub const CARD_NAME_HEIGHT: Rems = rems(1.125);
pub const CARD_DETAIL_HEIGHT: Rems = rems(0.875);
pub const TILE_PADDING_X: Rems = rems(0.375);
pub const TILE_PADDING_Y: Rems = rems(0.375);

impl CardLayout {
    pub fn grid() -> Self {
        Self::Grid(grid_size())
    }

    pub fn spec(self) -> CardSpec {
        // A grid row: margins, border and padding around the icon area, the name
        // and the type, with a gap between each.
        let grid = |min_width: f32, media: f32, icon: f32, thumbnail_width: f32| CardSpec {
            min_width: rems(min_width),
            row_height: rems(media + 2.0 * (CARD_MARGIN.0 + CARD_BORDER.0 + GRID_CARD_PADDING_Y.0 + GRID_CARD_GAP.0) + CARD_NAME_HEIGHT.0 + CARD_DETAIL_HEIGHT.0),
            media: rems(media),
            icon: rems(icon),
            thumbnail_width: rems(thumbnail_width),
            thumbnail_height: rems(media),
        };
        match self {
            // The thumbnails are 128 px: the largest area shows them at their size.
            Self::Grid(GridSize::Small) => grid(5.75, 3.25, 3.0, 4.5),
            Self::Grid(GridSize::Medium) => grid(7.25, 4.75, 4.0, 6.0),
            Self::Grid(GridSize::Large) => grid(9.5, 7.0, 6.0, 8.0),
            Self::Tiles => CardSpec {
                min_width: rems(15.0),
                row_height: rems(3.0 + 2.0 * (CARD_MARGIN.0 + CARD_BORDER.0 + TILE_PADDING_Y.0)),
                media: rems(3.0),
                icon: rems(2.75),
                thumbnail_width: rems(3.0),
                thumbnail_height: rems(3.0),
            },
        }
    }

    pub fn min_width(self) -> f32 {
        rem_pixels(self.spec().min_width).as_f32()
    }

    pub fn row_height(self) -> f32 {
        rem_pixels(self.spec().row_height).as_f32()
    }
}

#[cfg(test)]
#[path = "tests/cards.rs"]
mod tests;
