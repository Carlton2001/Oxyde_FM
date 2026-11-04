use super::*;

fn links(text: &str) -> Vec<&str> {
    find(text).into_iter().map(|range| &text[range]).collect()
}

#[test]
fn www_and_scheme_addresses_drop_trailing_punctuation() {
    assert_eq!(links("Visit www.commonmark.org."), ["www.commonmark.org"]);
    assert_eq!(links("See https://example.com/a?b=1, then"), ["https://example.com/a?b=1"]);
    assert_eq!(links("(www.google.com/search?q=Markup+(business))"), ["www.google.com/search?q=Markup+(business)"]);
    assert_eq!(links("www.google.com/search?q=commonmark&hl;"), ["www.google.com/search?q=commonmark"]);
    assert_eq!(links("www.commonmark.org/he<lp"), ["www.commonmark.org/he"]);
}

#[test]
fn invalid_domains_and_mid_word_starts_are_not_links() {
    assert!(links("www.exa_mple.com_ and http://localhost and xwww.example.com").is_empty());
}

#[test]
fn email_addresses_need_a_period_in_their_host() {
    assert_eq!(links("Write to foo@bar.baz."), ["foo@bar.baz"]);
    assert_eq!(links("a.b-c_d@a.b"), ["a.b-c_d@a.b"]);
    assert!(links("a.b-c_d@a.b- and hello@world").is_empty());
}
