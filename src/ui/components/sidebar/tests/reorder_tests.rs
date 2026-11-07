use super::{target, Slot};

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
