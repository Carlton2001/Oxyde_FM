use super::*;

#[test]
fn keeps_literal_ampersands_and_rejects_ambiguous_commands() {
    assert_eq!(clean_label("  Google &Docs\tCtrl+D "), "Google Docs");
    assert_eq!(clean_label("A && B"), "A & B");
    assert_eq!(unique([42].into_iter()), Some(42));
    assert_eq!(unique([42, 43].into_iter()), None);
}

#[test]
fn removes_only_the_new_submenu_without_changing_other_command_ids() {
    let item = |id, label: &str, has_submenu, verb: Option<&str>| WinMenuItem {
        id,
        label: label.into(),
        verb: verb.map(str::to_owned),
        has_submenu,
        children: Vec::new(),
    };
    for (title, verb) in [("Nouveau", None), ("Neu", None), ("Créer", Some("New"))] {
        let mut items = vec![
            item(4, "Ouvrir dans le Terminal", false, None),
            item(-1, title, true, verb),
            item(18, "Nouvelle fenêtre", false, None),
            item(-1, "Accorder l’accès à", true, None),
        ];
        remove_new_submenu(&mut items);
        assert_eq!(
            items
                .iter()
                .map(|item| (item.id, item.label.as_str()))
                .collect::<Vec<_>>(),
            [
                (4, "Ouvrir dans le Terminal"),
                (18, "Nouvelle fenêtre"),
                (-1, "Accorder l’accès à"),
            ]
        );
    }
}

#[test]
#[ignore = "Requires the installed Windows Shell handlers in a desktop session"]
fn reads_the_same_entries_as_more_options_and_invokes_a_fresh_command() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().to_str().unwrap().to_owned();
    let native = super::super::get_native_context_menu_items(vec![folder.clone()], true).unwrap();
    let new_submenu = native
        .iter()
        .find(|item| {
            item.has_submenu && (item.verb.as_deref() == Some("new") || new_label(&item.label))
        })
        .expect("the Windows background menu has a New submenu");
    let system_folder_label = super::super::super::windows_shell_new::folder_menu_label();
    let expected: Vec<_> = new_submenu
        .children
        .iter()
        .filter(|item| {
            item.id > 0
                && !item.has_submenu
                && !folder_label(&item.label, system_folder_label.as_deref())
        })
        .map(|item| item.label.as_str())
        .collect();
    let actual = entries(&folder, true).unwrap();
    assert!(
        !actual.is_empty(),
        "the installed Shell must advertise at least one file type"
    );
    let actual_labels: Vec<_> = actual
        .iter()
        .map(|item| item.command.label.as_str())
        .collect();
    eprintln!(
        "Windows New menu: {actual_labels:?}, with {} native bitmaps",
        actual.iter().filter(|item| item.icon.is_some()).count()
    );
    // The general Shell menu applies its standard-verb filter to New as well,
    // hiding commands such as Shortcut. Compare all of its rows in order.
    let comparable: Vec<_> = actual_labels
        .iter()
        .copied()
        .filter(|label| expected.contains(label))
        .collect();
    assert_eq!(comparable, expected);

    let folder_command = with_new_submenu(&folder, |context, menu, system_label| {
        commands(context, menu, false).find_map(|(_, command, _)| {
            folder_label(&command.label, system_label).then_some(command)
        })
    })
    .unwrap()
    .flatten()
    .expect("the Windows folder command is present");
    let forged = ShellNewCommand {
        label: "Oxyde missing command".into(),
        verb: folder_command.verb.clone(),
    };
    assert!(execute(&folder, &forged).is_err());
    assert_eq!(temp.path().read_dir().unwrap().count(), 0);
    execute(&folder, &folder_command).unwrap();
    assert!(wait_for(&temp, |path| path.is_dir()));

    if let Some(text) = actual.iter().find(|item| {
        matches!(
            item.command.label.as_str(),
            "Document texte" | "Text Document"
        )
    }) {
        execute(&folder, &text.command).unwrap();
        assert!(wait_for(&temp, |path| path
            .extension()
            .is_some_and(|extension| extension == "txt")));
    }
}

fn wait_for(temp: &tempfile::TempDir, matches: impl Fn(&std::path::Path) -> bool) -> bool {
    for _ in 0..20 {
        if temp
            .path()
            .read_dir()
            .unwrap()
            .any(|entry| matches(&entry.unwrap().path()))
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}
