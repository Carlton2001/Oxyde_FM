use super::*;
use PermissionClass::{Group, Others, Owner};

#[test]
fn file_levels_ignore_the_execution_bit() {
    assert_eq!(Access::of(0o644, false, Owner), Some(Access::ReadWrite));
    assert_eq!(Access::of(0o755, false, Owner), Some(Access::ReadWrite));
    assert_eq!(Access::of(0o755, false, Group), Some(Access::ReadOnly));
    assert_eq!(Access::of(0o750, false, Others), Some(Access::None));
    assert_eq!(Access::of(0o601, false, Others), Some(Access::None));
}

#[test]
fn combinations_outside_the_levels_are_shown_as_they_are() {
    // A file owner cannot be offered None, nor write without read.
    assert_eq!(Access::of(0o044, false, Owner), None);
    assert_eq!(Access::of(0o620, false, Group), None);
    assert_eq!(symbolic(0o620, Group), "-w-");
    assert_eq!(Access::of(0o761, true, Group), None);
    assert_eq!(Access::of(0o711, true, Others), None);
    assert_eq!(symbolic(0o711, Others), "--x");
}

#[test]
fn folder_levels_follow_nautilus() {
    assert_eq!(Access::of(0o750, true, Owner), Some(Access::CreateDelete));
    assert_eq!(Access::of(0o750, true, Group), Some(Access::AccessFiles));
    assert_eq!(Access::of(0o740, true, Group), Some(Access::ListFiles));
    assert_eq!(Access::of(0o750, true, Others), Some(Access::None));
    // As for a file, Nautilus never offers None to the owner of a folder.
    assert_eq!(Access::of(0o050, true, Owner), None);
    assert!(!Access::choices(true, Owner).contains(&Access::None));
}

#[test]
fn a_level_changes_only_its_class_and_keeps_special_bits() {
    assert_eq!(Access::ReadOnly.apply(0o4755, false, Owner), 0o4555);
    assert_eq!(Access::ReadWrite.apply(0o644, false, Others), 0o646);
    assert_eq!(Access::None.apply(0o2775, true, Others), 0o2770);
    assert_eq!(Access::AccessFiles.apply(0o1700, true, Group), 0o1750);
    assert_eq!(Access::ListFiles.apply(0o755, true, Others), 0o754);
}

#[test]
fn every_level_offered_reads_back_as_itself() {
    for is_dir in [false, true] {
        for class in [Owner, Group, Others] {
            for &level in Access::choices(is_dir, class) {
                for mode in [0o000, 0o777, 0o4751, 0o640] {
                    assert_eq!(Access::of(level.apply(mode, is_dir, class), is_dir, class), Some(level));
                }
            }
        }
    }
}

#[test]
fn execution_sets_and_clears_the_three_bits_together() {
    assert!(!executable(0o644));
    assert_eq!(with_executable(0o640, true), 0o751);
    assert!(executable(0o751));
    assert_eq!(with_executable(0o4755, false), 0o4644);
}

#[test]
fn enclosed_levels_change_only_the_chosen_classes_of_each_kind() {
    let change = Levels {
        files: [None, Some(Access::ReadOnly), Some(Access::None)],
        folders: [None, None, Some(Access::AccessFiles)],
    };
    assert!(!change.is_empty() && Levels::default().is_empty());
    assert_eq!(change.mode(0o766, false), 0o740);
    // A file keeps its execution bits: they have their own checkbox.
    assert_eq!(change.mode(0o755, false), 0o751);
    assert_eq!(change.mode(0o2770, true), 0o2775);
}

#[test]
fn a_selection_shares_a_level_only_when_every_item_has_its_bits() {
    assert_eq!(shared([0o640, 0o644], false, Owner), Some(0o640));
    assert_eq!(shared([0o640, 0o644], false, Others), None);
    // The execution bit of a file has its own checkbox; a folder's is part of its level.
    assert_eq!(shared([0o644, 0o755], false, Group), Some(0o644));
    assert_eq!(shared([0o755, 0o745], true, Group), None);
    assert_eq!(shared([], true, Owner), None);
}

#[test]
fn enclosed_levels_start_from_the_umask_as_nautilus() {
    use Access::{AccessFiles, CreateDelete, ReadOnly, ReadWrite};
    let usual = Levels::from_umask(0o022);
    assert_eq!(usual.files, [Some(ReadWrite), Some(ReadOnly), Some(ReadOnly)]);
    assert_eq!(usual.folders, [Some(CreateDelete), Some(AccessFiles), Some(AccessFiles)]);
    let private = Levels::from_umask(0o077);
    assert_eq!(private.files, [Some(ReadWrite), Some(Access::None), Some(Access::None)]);
    assert_eq!(private.folders, [Some(CreateDelete), Some(Access::None), Some(Access::None)]);
    // Write without read is no level: that class stays as each item has it.
    assert_eq!(Levels::from_umask(0o042).folders[1], None);
}
