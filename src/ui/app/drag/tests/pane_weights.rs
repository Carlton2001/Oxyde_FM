use super::resized_pair_weight;

#[test]
fn light_pair_does_not_panic() {
    assert_eq!(resized_pair_weight(0.05, 0.1, 3.0), 0.05);
    assert_eq!(resized_pair_weight(0.1, 0.2, -1.0), 0.1);
}

#[test]
fn normal_pair_is_clamped() {
    assert_eq!(resized_pair_weight(1.0, 2.0, 5.0), 1.9);
    assert_eq!(resized_pair_weight(1.0, 2.0, -5.0), 0.1);
    assert_eq!(resized_pair_weight(1.0, 2.0, 0.25), 1.25);
}
