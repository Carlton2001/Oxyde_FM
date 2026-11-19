use super::*;

#[test]
fn invalid_page_size_keeps_its_place() {
    let sizes = parse_pdf_sizes("600 800
0 0
800 600
NaN 1
").unwrap();
    assert_eq!(
        sizes,
        [(600.0, 800.0), (600.0, 800.0), (800.0, 600.0), (800.0, 600.0)]
    );
}

#[test]
fn leading_invalid_page_takes_the_next_valid_size() {
    let sizes = parse_pdf_sizes("-1 5
800 600
").unwrap();
    assert_eq!(sizes, [(800.0, 600.0), (800.0, 600.0)]);
}

#[test]
fn only_invalid_pages_fall_back_to_letter() {
    assert_eq!(parse_pdf_sizes("0 0
").unwrap(), [(612.0, 792.0)]);
    assert!(parse_pdf_sizes("").is_err());
}
