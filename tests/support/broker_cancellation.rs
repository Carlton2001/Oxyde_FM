use super::*;

#[test]
fn real_archive_broker_cancels_blocked_native_io_before_private_output_publication() {
    use std::{
        io::Read,
        os::windows::io::{FromRawHandle, OwnedHandle},
    };
    use windows::Win32::{Foundation::HANDLE, System::Pipes::CreatePipe};
    native(|| {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("input");
        fs::write(&source, b"safe").unwrap();
        let destination = root.path().join("output.zip");
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        unsafe { CreatePipe(&mut read, &mut write, None, 0) }.unwrap();
        let mut input = fs::File::from(unsafe { OwnedHandle::from_raw_handle(read.0) });
        let _writer = unsafe { OwnedHandle::from_raw_handle(write.0) };
        let op = operation(FileOpType::Compress);
        let cancel = op.lock().unwrap().cancel_flag.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let canceller = std::thread::spawn(move || {
            receiver.recv_timeout(Duration::from_secs(20)).unwrap();
            std::thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Release);
        });
        let observed = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let witness = observed.clone();
        let result = oxyde_lib::utils::archive_mutation_test::with_hook_for_tests(
            move |stage, _, _| {
                if stage == oxyde_lib::utils::archive_mutation_test::Stage::BrokerBeforeSourceRead {
                    sender.send(()).unwrap();
                    let error = input.read_exact(&mut [0]).unwrap_err();
                    witness.store(error.raw_os_error().unwrap_or(-1), Ordering::Release);
                    return Err(error);
                }
                Ok(())
            },
            || {
                FileOperationManager::perform_compress(
                    &op,
                    vec![source.clone()],
                    Some(destination.clone()),
                )
            },
        );
        canceller.join().unwrap();
        assert!(result.is_err());
        assert_eq!(observed.load(Ordering::Acquire), 995);
        assert!(!destination.exists());
        assert_eq!(fs::read(source).unwrap(), b"safe");
    });
}
