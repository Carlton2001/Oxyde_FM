use super::super::windows_process_launch::native_probe;
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn runtime_directory_guard_refuses_native_rename_during_handle_transition() {
    let source = native_probe::executable();
    let observed = Rc::new(RefCell::new(None));
    let observe = observed.clone();
    HANDLE_TRANSITION_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move |target, parent| {
            let root = target.parent().unwrap().to_path_buf();
            let original = file_id(&File::open(target).unwrap()).unwrap();
            let error = rename_retained(target, parent, "original-owned.exe")
                .expect_err("native rename was not blocked by the directory guard");
            assert_eq!(error.raw_os_error(), Some(32), "{error}");
            assert!(target.exists());
            assert!(!root.join("original-owned.exe").exists());
            *observe.borrow_mut() = Some((root, original));
        }));
    });
    let mut profile = Profile::create().unwrap();
    profile.remove_before_execution().unwrap();
    let runtime = Runtime::create(&source, profile.sid, &|| Ok(())).unwrap();
    let (root, original) = observed
        .borrow_mut()
        .take()
        .expect("transition was not exercised");
    assert_eq!(file_id(runtime.binary.as_ref().unwrap()).unwrap(), original);
    drop(runtime);
    assert!(!root.exists(), "owned runtime was not cleaned");
}

#[test]
fn changed_bytes_on_same_object_before_admission_are_rejected() {
    use std::io::{Seek, SeekFrom};
    let source = native_probe::executable();
    let observed = Rc::new(RefCell::new(None));
    let observe = observed.clone();
    HANDLE_TRANSITION_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move |target, _parent| {
            let original = file_id(&File::open(target).unwrap()).unwrap();
            let mut writer = OpenOptions::new()
                .write(true)
                .share_mode(7)
                .open(target)
                .unwrap();
            writer.seek(SeekFrom::Start(0)).unwrap();
            writer.write_all(b"tampered-runtime").unwrap();
            writer.sync_all().unwrap();
            drop(writer);
            assert_eq!(file_id(&File::open(target).unwrap()).unwrap(), original);
            *observe.borrow_mut() = Some(target.parent().unwrap().to_path_buf());
        }));
    });
    let mut profile = Profile::create().unwrap();
    profile.remove_before_execution().unwrap();
    let error = Runtime::create(&source, profile.sid, &|| Ok(()))
        .err()
        .expect("modified runtime was admitted");
    assert!(error.to_string().contains("provenance mismatch"), "{error}");
    let root = observed
        .borrow_mut()
        .take()
        .expect("write attempt was not exercised");
    assert!(!root.exists(), "modified owned runtime was not cleaned");
}

/// Use the existing project's native RootDirectory rename form, avoiding
/// MoveFileEx's extra pathname-based parent opens under the directory guard.
fn rename_retained(source: &Path, parent: &File, leaf: &str) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::FILE_RENAME_INFO;
    #[repr(C)]
    struct IoStatusBlock {
        status: usize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtSetInformationFile(
            file: RawHandle,
            status: *mut IoStatusBlock,
            information: *const std::ffi::c_void,
            length: u32,
            class: u32,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }
    let source = OpenOptions::new()
        .access_mode(0x10000)
        .share_mode(7)
        .open(source)
        .inspect_err(|error| {
            eprintln!("Native rename DELETE open refused: {error}");
        })?;
    let name: Vec<u16> = leaf.encode_utf16().collect();
    let size = std::mem::size_of::<FILE_RENAME_INFO>() + name.len() * 2;
    let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let mut status = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let result = unsafe {
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).RootDirectory = HANDLE(parent.as_raw_handle());
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            buffer
                .as_mut_ptr()
                .cast::<u8>()
                .add(std::mem::offset_of!(FILE_RENAME_INFO, FileName))
                .cast::<u16>(),
            name.len(),
        );
        NtSetInformationFile(
            source.as_raw_handle(),
            &mut status,
            info.cast(),
            size as u32,
            10,
        )
    };
    if result < 0 {
        eprintln!("Native retained-directory rename refused: NTSTATUS {result:08x}");
        Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) } as i32,
        ))
    } else {
        Ok(())
    }
}
