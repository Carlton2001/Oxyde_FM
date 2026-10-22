use super::native_action_opens_media;
use oxyde_lib::models::WinMenuItem;

fn item(id: i32, verb: Option<&str>, children: Vec<WinMenuItem>) -> WinMenuItem {
    WinMenuItem {
        id,
        label: "Localized label".into(),
        verb: verb.map(str::to_owned),
        has_submenu: !children.is_empty(),
        children,
    }
}

#[test]
fn opening_commands_are_identified_by_canonical_verb() {
    for verb in ["open", "openas", "OpenWith", "play", "playwith"] {
        let items = [item(7, Some(verb), vec![])];
        assert!(native_action_opens_media(&items, 7, false), "{verb}");
        assert!(!native_action_opens_media(&items, 8, false));
    }
    for verb in [None, Some("properties"), Some("copy"), Some("enqueue")] {
        assert!(!native_action_opens_media(&[item(7, verb, vec![])], 7, false));
    }
}

#[test]
fn open_with_submenu_applications_inherit_the_opening_action() {
    let items = [
        item(-1, Some("openas"), vec![item(8, None, vec![])]),
        item(9, Some("properties"), vec![]),
    ];
    assert!(native_action_opens_media(&items, 8, false));
    assert!(!native_action_opens_media(&items, -1, false));
    assert!(!native_action_opens_media(&items, 9, false));
}

#[test]
fn open_with_popup_without_a_canonical_verb_pauses_for_each_application() {
    for label in [
        "Ouvrir avec",
        "Open with",
        "Öffnen mit",
        "Abrir con",
        "Apri con",
        " &Ouvrir &avec… ",
        "Open with...\tCtrl+O",
    ] {
        let mut popup = item(-1, None, vec![
            item(8, None, vec![]),
            item(9, Some("Application.CustomVerb"), vec![]),
        ]);
        popup.label = label.into();
        let items = [popup, item(10, None, vec![])];
        assert!(native_action_opens_media(&items, 8, false), "{label}");
        assert!(native_action_opens_media(&items, 9, false), "{label}");
        assert!(!native_action_opens_media(&items, -1, false), "{label}");
        assert!(!native_action_opens_media(&items, 10, false), "{label}");
    }
}

#[test]
fn unrelated_submenus_and_leaf_labels_do_not_pause_playback() {
    for label in ["Envoyer vers", "Partager", "Open with another tool"] {
        let mut popup = item(-1, None, vec![item(8, None, vec![])]);
        popup.label = label.into();
        assert!(!native_action_opens_media(&[popup], 8, false), "{label}");
    }
    let mut leaf = item(8, None, vec![]);
    leaf.label = "Open with".into();
    assert!(!native_action_opens_media(&[leaf], 8, false));
}
