use super::is_visible;

#[test]
fn system_takes_precedence_in_all_attribute_and_preference_combinations() {
    for hidden in [false, true] {
        for system in [false, true] {
            for show_hidden in [false, true] {
                for show_system in [false, true] {
                    let expected = match (hidden, system) {
                        (_, true) => show_system,
                        (true, false) => show_hidden,
                        (false, false) => true,
                    };
                    assert_eq!(
                        is_visible(hidden, system, show_hidden, show_system),
                        expected
                    );
                }
            }
        }
    }
}
