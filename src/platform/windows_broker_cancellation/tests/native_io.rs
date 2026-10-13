use super::*;
use std::io::{Read, Write};
use windows::Win32::System::Pipes::CreatePipe;

#[test]
fn blocked_native_broker_read_is_cancelled_and_its_thread_can_read_again() {
    for deadline in [false, true] {
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        unsafe { CreatePipe(&mut read, &mut write, None, 0) }.unwrap();
        let mut input = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(read.0) });
        let mut output = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(write.0) });
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let guard = watch(
                if deadline {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(10)
                },
                flag,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            sender.send(None).unwrap();
            let error = input.read_exact(&mut [0]).unwrap_err();
            drop(guard);
            sender.send(Some(error.raw_os_error())).unwrap();
            let mut byte = [0];
            input.read_exact(&mut byte).unwrap();
            byte
        });
        assert_eq!(receiver.recv_timeout(Duration::from_secs(3)).unwrap(), None);
        if !deadline {
            cancel.store(true, Ordering::Release);
        }
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            Some(Some(995))
        );
        output.write_all(&[42]).unwrap();
        assert_eq!(thread.join().unwrap(), [42]);
    }
}
