use super::*;
use windows::Win32::System::Com::*;

#[test]
fn native_object_preserves_multiselection_unicode_and_separate_medium_ownership() {
    let paths = vec![r"C:\école\🦀.txt".into(), r"\\server\share\folder".into()];
    let formats = Rc::new(RefCell::new(BTreeMap::from([(
        CF_HDROP.0,
        crate::platform::windows_clipboard_write::file_bytes(&paths),
    )])));
    let object: IDataObject = data::FileData { formats }.into();
    let mut first = unsafe { object.GetData(&data::format(CF_HDROP.0)) }.unwrap();
    let mut second = unsafe { object.GetData(&data::format(CF_HDROP.0)) }.unwrap();
    assert_ne!(unsafe { first.u.hGlobal }, unsafe { second.u.hGlobal });
    unsafe {
        ReleaseStgMedium(&mut first);
    }
    assert_eq!(data::read_paths(&object).unwrap(), paths);
    unsafe {
        ReleaseStgMedium(&mut second);
    }
}

#[test]
fn optimized_move_acknowledgement_does_not_request_source_deletion() {
    let formats = Rc::new(RefCell::new(BTreeMap::new()));
    let object: IDataObject = data::FileData {
        formats: formats.clone(),
    }
    .into();
    let id = data::registered(w!("Performed DropEffect"));
    data::set_effect(&object, id, DROPEFFECT_NONE);
    assert_eq!(formats.borrow()[&id], 0_u32.to_le_bytes());
    data::set_effect(&object, id, DROPEFFECT_MOVE);
    assert_eq!(formats.borrow()[&id], DROPEFFECT_MOVE.0.to_le_bytes());
}

#[test]
fn malformed_native_paths_and_unsupported_media_are_rejected() {
    let object: IDataObject = data::FileData {
        formats: Rc::new(RefCell::new(BTreeMap::from([(CF_HDROP.0, vec![0; 4])]))),
    }
    .into();
    assert!(data::read_paths(&object).is_err());
    let mut requested = data::format(CF_HDROP.0);
    requested.tymed = TYMED_ISTREAM.0 as u32;
    assert_eq!(unsafe { object.QueryGetData(&requested) }, DV_E_TYMED);
}
