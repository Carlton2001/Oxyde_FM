use super::*;

#[cfg(windows)]
#[::core::prelude::v1::test]
fn modifiers_respect_the_sources_allowed_operations() {
    let paths = vec!["C:/source/file".into()];
    let target = "C:/target";
    let state = ExternalDropState {
        can_move: false,
        ..Default::default()
    };
    assert_eq!(
        choose_effect(&paths, target, Modifiers::default(), &state),
        FileDropEffect::Copy
    );
    assert_eq!(
        choose_effect(
            &paths,
            target,
            Modifiers {
                shift: true,
                ..Default::default()
            },
            &state
        ),
        FileDropEffect::None
    );
    assert_eq!(
        choose_effect(
            &paths,
            target,
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            &state
        ),
        FileDropEffect::Copy
    );
}

#[::core::prelude::v1::test]
fn virtual_targets_and_recursive_or_noop_drops_are_rejected() {
    let source = if cfg!(windows) {
        "C:/source/folder"
    } else {
        "/source/folder"
    };
    let data = ExternalPaths([std::path::PathBuf::from(source)].into_iter().collect());
    assert!(transfer_paths(&data, "trash://").is_empty());
    assert!(transfer_paths(&data, "viewer://").is_empty());
    assert!(transfer_paths(&data, source).is_empty());
    assert!(transfer_paths(&data, &format!("{source}/child")).is_empty());
    assert_eq!(
        transfer_paths(&data, &format!("{source}-other")),
        vec![source.to_owned()]
    );
}

#[cfg(not(windows))]
#[::core::prelude::v1::test]
fn drops_negotiated_as_copy_never_move_their_sources() {
    let paths = vec!["/home/user/file".into()];
    let state = ExternalDropState::default();
    assert_eq!(
        choose_effect(&paths, "/home/user/target", Modifiers::default(), &state),
        FileDropEffect::Copy
    );
    assert_eq!(
        choose_effect(
            &paths,
            "/home/user/target",
            Modifiers {
                shift: true,
                ..Default::default()
            },
            &state
        ),
        FileDropEffect::Copy
    );
}

#[cfg(windows)]
#[::core::prelude::v1::test]
fn multiselection_from_mixed_volumes_defaults_to_copy() {
    let paths = vec!["C:/one".into(), "D:/two".into()];
    assert_eq!(
        choose_effect(
            &paths,
            "C:/target",
            Modifiers::default(),
            &Default::default()
        ),
        FileDropEffect::Copy
    );
    assert_eq!(
        choose_effect(
            &paths,
            "C:/target",
            Modifiers {
                shift: true,
                ..Default::default()
            },
            &Default::default()
        ),
        FileDropEffect::Move
    );
}
