use super::*;

#[test]
fn kde_gets_breeze_and_the_others_adwaita() {
    assert_eq!(for_desktop("KDE"), WindowControlStyle::Breeze);
    assert_eq!(for_desktop("ubuntu:KDE"), WindowControlStyle::Breeze);
    assert_eq!(for_desktop("GNOME"), WindowControlStyle::Adwaita);
    assert_eq!(for_desktop("XFCE"), WindowControlStyle::Adwaita);
    assert_eq!(for_desktop(""), WindowControlStyle::Adwaita);
}
