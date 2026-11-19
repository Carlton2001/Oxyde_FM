use super::*;

fn descriptor(name: &str, flags: i32, attributes: u32, size: u64) -> FILEDESCRIPTORW {
    let mut descriptor = FILEDESCRIPTORW {
        dwFlags: flags as u32,
        dwFileAttributes: attributes,
        nFileSizeHigh: (size >> 32) as u32,
        nFileSizeLow: size as u32,
        ftLastWriteTime: FILETIME { dwLowDateTime: 0x256d_4000, dwHighDateTime: 0x01bf_53eb },
        ..Default::default()
    };
    let mut file_name = [0_u16; 260];
    for (slot, unit) in file_name.iter_mut().zip(name.encode_utf16()) {
        *slot = unit;
    }
    descriptor.cFileName = file_name;
    descriptor
}

fn group(descriptors: &[FILEDESCRIPTORW]) -> Vec<u8> {
    let mut bytes = (descriptors.len() as u32).to_le_bytes().to_vec();
    for descriptor in descriptors {
        let raw = unsafe {
            std::slice::from_raw_parts((descriptor as *const FILEDESCRIPTORW).cast::<u8>(), size_of::<FILEDESCRIPTORW>())
        };
        bytes.extend_from_slice(raw);
    }
    bytes
}

#[test]
fn descriptors_give_relative_entries_and_top_level_items_once() {
    let all = FD_ATTRIBUTES.0 | FD_FILESIZE.0 | FD_WRITESTIME.0;
    let bytes = group(&[
        descriptor("folder", all, FILE_ATTRIBUTE_DIRECTORY.0, 0),
        descriptor(r"folder\inner.txt", all, 0, 5),
        descriptor("École.txt", FD_FILESIZE.0, 0, 1 << 33),
    ]);
    let entries = parse_descriptors(&bytes).unwrap();
    assert_eq!(entries.len(), 3);
    assert!(entries[0].directory);
    assert_eq!(entries[1].relative, PathBuf::from("folder").join("inner.txt"));
    assert_eq!(entries[1].size, Some(5));
    // 2000-01-01T00:00:00Z.
    assert_eq!(entries[1].modified, Some(SystemTime::UNIX_EPOCH + Duration::from_secs(946_684_800)));
    assert_eq!(entries[2].size, Some(1 << 33));
    assert_eq!(entries[2].modified, None);
    let staging = PathBuf::from(r"C:\staging");
    let listing = listing(staging, entries).unwrap();
    assert_eq!(listing.top, [r"C:\staging\folder", r"C:\staging\École.txt"]);
}

#[test]
fn unsafe_or_malformed_descriptors_are_refused() {
    for name in [r"..\outside.txt", r"C:\absolute.txt", r"\rooted.txt", "stream.txt:hidden", "CON", "trailing. ", ""] {
        assert!(parse_descriptors(&group(&[descriptor(name, 0, 0, 0)])).is_err(), "{name:?}");
    }
    let bytes = group(&[descriptor("a.txt", 0, 0, 0)]);
    assert!(parse_descriptors(&bytes[..bytes.len() - 1]).is_err());
    assert!(parse_descriptors(&0_u32.to_le_bytes()).is_err());
    let mut unterminated = descriptor("a.txt", 0, 0, 0);
    unterminated.cFileName = [u16::from(b'a'); 260];
    assert!(parse_descriptors(&group(&[unterminated])).is_err());
}

#[test]
fn a_directory_needs_its_attribute_flag() {
    let entries = parse_descriptors(&group(&[descriptor("folder", 0, FILE_ATTRIBUTE_DIRECTORY.0, 0)])).unwrap();
    assert!(!entries[0].directory);
}

/// A source offering a descriptor and one HGLOBAL content per file.
#[windows::core::implement(IDataObject)]
struct Streamed {
    descriptor: Vec<u8>,
    contents: Vec<Vec<u8>>,
}

#[allow(non_snake_case)]
impl IDataObject_Impl for Streamed_Impl {
    fn GetData(&self, requested: *const FORMATETC) -> Result<STGMEDIUM> {
        let requested = unsafe { &*requested };
        if requested.cfFormat == data::registered(CFSTR_FILEDESCRIPTORW) && requested.lindex == -1 {
            return data::medium(&self.descriptor);
        }
        if requested.cfFormat == data::registered(CFSTR_FILECONTENTS) && requested.tymed & TYMED_HGLOBAL.0 as u32 != 0 {
            if let Some(bytes) = usize::try_from(requested.lindex).ok().and_then(|index| self.contents.get(index)) {
                return data::medium(bytes);
            }
        }
        Err(DV_E_FORMATETC.into())
    }
    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> Result<()> { Err(E_NOTIMPL.into()) }
    fn QueryGetData(&self, _: *const FORMATETC) -> HRESULT { DV_E_FORMATETC }
    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT { E_NOTIMPL }
    fn SetData(&self, _: *const FORMATETC, _: *const STGMEDIUM, _: BOOL) -> Result<()> { Err(E_NOTIMPL.into()) }
    fn EnumFormatEtc(&self, _: u32) -> Result<IEnumFORMATETC> { Err(E_NOTIMPL.into()) }
    fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<IAdviseSink>) -> Result<u32> { Err(OLE_E_ADVISENOTSUPPORTED.into()) }
    fn DUnadvise(&self, _: u32) -> Result<()> { Err(OLE_E_ADVISENOTSUPPORTED.into()) }
    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> { Err(OLE_E_ADVISENOTSUPPORTED.into()) }
}

#[test]
fn streamed_files_are_written_to_staging_then_removed_with_it() {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let root = tempfile::tempdir().unwrap();
    let all = FD_ATTRIBUTES.0 | FD_FILESIZE.0 | FD_WRITESTIME.0;
    let object: IDataObject = Streamed {
        descriptor: group(&[
            descriptor("folder", all, FILE_ATTRIBUTE_DIRECTORY.0, 0),
            descriptor(r"folder\inner.txt", all, 0, 5),
            // A global block may be larger than the file.
            descriptor("top.txt", FD_FILESIZE.0, 0, 3),
        ]),
        contents: vec![Vec::new(), b"inner".to_vec(), b"topXXXX".to_vec()],
    }
    .into();
    let id = data::registered(CFSTR_FILEDESCRIPTORW);
    let mut storage = unsafe { object.GetData(&data::format(id)) }.unwrap();
    let entries = parse_descriptors(&data::read_global(&storage).unwrap()).unwrap();
    unsafe { ReleaseStgMedium(&mut storage) };
    let staging = root.path().join(STAGING).join("drop");
    let listing = listing(staging.clone(), entries).unwrap();
    let (paths, guard) = streamed(&object, listing).unwrap().extract().unwrap();
    assert_eq!(paths, [staging.join("folder").to_str().unwrap(), staging.join("top.txt").to_str().unwrap()]);
    assert_eq!(std::fs::read(staging.join("folder").join("inner.txt")).unwrap(), b"inner");
    assert_eq!(std::fs::read(staging.join("top.txt")).unwrap(), b"top");
    let modified = std::fs::metadata(staging.join("folder").join("inner.txt")).unwrap().modified().unwrap();
    assert_eq!(modified, SystemTime::UNIX_EPOCH + Duration::from_secs(946_684_800));
    drop(guard);
    for _ in 0..100 {
        if !staging.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("staging was not removed");
}
