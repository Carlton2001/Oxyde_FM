use super::{arguments, listed};

fn pinned() -> Vec<(String, String)> {
    [("Work", r"C:\Work"), ("Trash", "trash://"), ("Network", "__network_vincinity__"), ("Data", r"D:\")]
        .map(|(name, path)| (name.to_owned(), path.to_owned()))
        .to_vec()
}

#[test]
fn only_folders_oxyde_can_be_started_on_are_listed() {
    let pinned = pinned();
    let paths: Vec<&str> = listed(&pinned, &[]).into_iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, [r"C:\Work", r"D:\"]);
}

#[test]
fn a_folder_the_user_removed_from_the_menu_is_left_out() {
    let pinned = pinned();
    let removed = [arguments(r"c:\work")];
    let paths: Vec<&str> = listed(&pinned, &removed).into_iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(paths, [r"D:\"]);
}

/// Needs the desktop Shell, which keeps the lists: run with `--ignored` in a session.
#[test]
#[ignore]
fn the_shell_accepts_the_menu() {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Shell::{DestinationList, ICustomDestinationList};
    let identity = "Oxyde.Test.JumpList";
    let menu = super::TaskbarMenu { category: "Quick access".into(), pinned: pinned(), administrator: "Run as administrator".into() };
    super::write(&menu, identity).unwrap();
    let _apartment = super::ComApartment::sta_or_existing().unwrap();
    unsafe {
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER).unwrap();
        list.DeleteList(&HSTRING::from(identity)).unwrap();
    }
}
