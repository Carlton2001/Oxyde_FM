use super::{shift, target, Slot};

fn slots() -> Vec<Slot> {
    ["/a", "/b"].iter().enumerate()
        .map(|(index, path)| Slot { top: 28.0 * (index + 1) as f32, height: 28.0, path: path.to_string() })
        .collect()
}

#[test]
fn row_halves_choose_the_side_and_the_ends_clamp() {
    let slots = slots();
    assert_eq!(target(&slots, 0.0), Some(("/a".into(), false)));
    assert_eq!(target(&slots, 30.0), Some(("/a".into(), false)));
    assert_eq!(target(&slots, 50.0), Some(("/b".into(), false)));
    assert_eq!(target(&slots, 60.0), Some(("/b".into(), false)));
    assert_eq!(target(&slots, 500.0), Some(("/b".into(), true)));
}

#[test]
fn rows_between_the_source_and_the_target_make_room() {
    let moved = |target| [0, 1, 2, 3].map(|index| shift(index, 1, target, 10.0));
    // Toward the end: the rows after the source close its place.
    assert_eq!(moved(4), [0.0, 0.0, -10.0, -10.0]);
    // Before the first row: it moves down.
    assert_eq!(moved(0), [10.0, 0.0, 0.0, 0.0]);
    // Either side of its own slot: nothing moves, the room stays open.
    assert_eq!(moved(1), [0.0; 4]);
    assert_eq!(moved(2), [0.0; 4]);
}
