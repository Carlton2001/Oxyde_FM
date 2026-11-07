use super::*;

const GNOME: &str = "[preferred]\ndefault=gnome;gtk;\norg.freedesktop.impl.portal.Secret=oo7-portal;gnome-keyring;\n";

#[test]
fn an_existing_line_gets_oxyde_first_and_back_as_it_was() {
    let text = "[preferred]\ndefault=gnome;gtk;\norg.freedesktop.impl.portal.FileChooser=strata;gnome;gtk;\n";
    let enabled = with_portal(text, "oxyde");
    assert_eq!(enabled, "[preferred]\ndefault=gnome;gtk;\norg.freedesktop.impl.portal.FileChooser=oxyde;strata;gnome;gtk;\n");
    assert_eq!(portals(&enabled), ["oxyde", "strata", "gnome", "gtk"]);
    assert_eq!(without_portal(&enabled, "oxyde"), text);
    // Turning it on twice keeps one entry.
    assert_eq!(with_portal(&enabled, "oxyde"), enabled);
}

#[test]
fn a_missing_line_is_added_with_the_default_after_oxyde_then_removed() {
    let enabled = with_portal(GNOME, "oxyde");
    assert_eq!(enabled, format!("{GNOME}{ADDED}\norg.freedesktop.impl.portal.FileChooser=oxyde;gnome;gtk;\n"));
    assert_eq!(portals(GNOME), ["gnome", "gtk"]);
    assert_eq!(portals(&enabled)[0], "oxyde");
    assert_eq!(without_portal(&enabled, "oxyde"), GNOME);
}

#[test]
fn the_added_line_stays_in_preferred_before_another_section() {
    let text = "[preferred]\ndefault=kde;\n\n[other]\nkey=value\n";
    let enabled = with_portal(text, "oxyde");
    assert_eq!(enabled, format!("[preferred]\ndefault=kde;\n{ADDED}\norg.freedesktop.impl.portal.FileChooser=oxyde;kde;\n\n[other]\nkey=value\n"));
    assert_eq!(without_portal(&enabled, "oxyde"), text);
}

#[test]
fn a_file_without_preferred_gets_the_section() {
    assert_eq!(with_portal("", "oxyde"), format!("[preferred]\n{ADDED}\norg.freedesktop.impl.portal.FileChooser=oxyde;\n"));
    assert_eq!(with_portal("# notes\n", "oxyde"), format!("# notes\n\n[preferred]\n{ADDED}\norg.freedesktop.impl.portal.FileChooser=oxyde;\n"));
}

#[test]
fn a_line_another_program_took_over_keeps_its_value() {
    let taken = format!("{GNOME}{ADDED}\norg.freedesktop.impl.portal.FileChooser=strata;gnome;gtk;\n");
    assert_eq!(without_portal(&taken, "oxyde"), format!("{GNOME}org.freedesktop.impl.portal.FileChooser=strata;gnome;gtk;\n"));
}

#[test]
fn an_existing_line_left_with_only_oxyde_goes() {
    let text = "[preferred]\ndefault=gnome;\norg.freedesktop.impl.portal.FileChooser=oxyde;\n";
    assert_eq!(without_portal(text, "oxyde"), "[preferred]\ndefault=gnome;\n");
}

#[test]
fn other_builds_and_other_sections_are_left_alone() {
    let text = "[preferred]\norg.freedesktop.impl.portal.FileChooser=oxyde;oxyde-devel;gnome;\n[x]\norg.freedesktop.impl.portal.FileChooser=oxyde;\n";
    assert_eq!(without_portal(text, "oxyde-devel"), "[preferred]\norg.freedesktop.impl.portal.FileChooser=oxyde;gnome;\n[x]\norg.freedesktop.impl.portal.FileChooser=oxyde;\n");
}

#[test]
fn a_written_file_names_its_source() {
    let text = created("/usr/share/xdg-desktop-portal/gnome-portals.conf", GNOME);
    assert_eq!(creation(&text), Some(("/usr/share/xdg-desktop-portal/gnome-portals.conf", GNOME)));
    assert_eq!(creation(GNOME), None);
}
