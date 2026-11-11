use super::{Columns, PADDING};
use gpui::{px, rems, Pixels};

const REM: Pixels = px(16.0);

/// The width leaving `name_room` rems to the name and secondary columns.
fn width_for(name_room: f32) -> Pixels {
    PADDING.to_pixels(REM) + rems(0.5 + 1.25 + 1.5).to_pixels(REM) + rems(name_room).to_pixels(REM)
}

fn shown(width: Pixels) -> (bool, bool) {
    let columns = Columns::for_width(width, REM);
    (columns.size, columns.date)
}

#[test]
fn before_the_first_layout_every_column_shows() {
    assert_eq!(shown(px(0.0)), (true, true));
}

#[test]
fn the_date_gives_way_first_then_the_size() {
    let all = Columns::NAME + Columns::SIZE + Columns::DATE;
    assert_eq!(shown(width_for(all)), (true, true));
    assert_eq!(shown(width_for(all - 0.5)), (true, false));
    assert_eq!(shown(width_for(Columns::NAME + Columns::SIZE)), (true, false));
    assert_eq!(shown(width_for(Columns::NAME + Columns::SIZE - 0.5)), (false, false));
    assert_eq!(shown(px(40.0)), (false, false));
}
