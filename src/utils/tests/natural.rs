use super::*;

#[test]
fn natural_order_compares_accented_and_cyrillic_letters_whole() {
    use std::cmp::Ordering::{Equal, Less};
    assert_eq!(compare_natural("École", "école"), Equal);
    assert_eq!(compare_natural("école2", "École10"), Less);
    assert_eq!(compare_natural("Ёлка", "ёлка"), Equal);
    assert_eq!(compare_natural("Береза", "ёлка"), Less);
    assert_eq!(compare_natural("é", "z"), "é".cmp("z"));
}
