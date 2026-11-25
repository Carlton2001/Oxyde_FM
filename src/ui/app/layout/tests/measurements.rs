use super::widest_tabular;
use std::cell::RefCell;

#[test]
fn texts_differing_by_digits_are_shaped_once() {
    let shaped = RefCell::new(Vec::new());
    let width = widest_tabular(
        ["1,5 Ko", "9,2 Ko", "12,5 Ko", "3 o"].into_iter().map(String::from),
        |text| {
            shaped.borrow_mut().push(text.to_owned());
            text.chars().count() as f32
        },
    );
    assert_eq!(width, 7.0);
    let mut shaped = shaped.into_inner();
    shaped.sort();
    assert_eq!(shaped, ["0 o", "0,0 Ko", "00,0 Ko"]);
}
