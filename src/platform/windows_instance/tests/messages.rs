use super::{create, decode, encode, forward, pipe_name, receive, serve, wake, OPEN, SELECT};
use windows::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};

#[test]
fn a_message_carries_its_locations_in_order() {
    let locations = vec![r"C:\Users".to_owned(), r"\\server\share\Été".to_owned()];
    assert_eq!(decode(&encode(OPEN, &locations)), Some((OPEN, locations.clone())));
    assert_eq!(decode(&encode(SELECT, &locations)), Some((SELECT, locations)));
    assert_eq!(decode(&encode(OPEN, &[])), Some((OPEN, Vec::new())));
}

#[test]
fn a_location_cannot_add_a_line() {
    let locations = vec!["C:\\a\nC:\\b".to_owned(), r"C:\c".to_owned()];
    assert_eq!(decode(&encode(OPEN, &locations)), Some((OPEN, vec![r"C:\c".to_owned()])));
}

#[test]
fn another_protocol_is_not_a_message() {
    assert_eq!(decode(""), None);
    assert_eq!(decode("oxyde-open 2\nC:\\Users"), None);
    assert_eq!(decode(r"C:\Users"), None);
}

#[test]
fn the_pipe_is_named_after_session_storage_build_and_rights() {
    let name = pipe_name(3, r"C:\Users\Me\AppData\Roaming\Oxyde", false, false);
    assert!(name.starts_with(r"\\.\pipe\Oxyde.Instance.Release.3."));
    assert_eq!(name, pipe_name(3, r"c:\users\me\appdata\roaming\OXYDE", false, false));
    assert_ne!(name, pipe_name(4, r"C:\Users\Me\AppData\Roaming\Oxyde", false, false));
    assert_ne!(name, pipe_name(3, r"D:\Portable\data", false, false));
    assert_ne!(name, pipe_name(3, r"C:\Users\Me\AppData\Roaming\Oxyde", true, false));
    assert_ne!(name, pipe_name(3, r"C:\Users\Me\AppData\Roaming\Oxyde", false, true));
}

#[test]
fn a_later_launch_reaches_the_first_and_only_owner_of_the_pipe() {
    let name = format!(r"\\.\pipe\Oxyde.Instance.Test.{}", std::process::id());
    let pipe = create(&name).expect("first owner");
    assert!(create(&name).is_none());
    let reader = std::thread::spawn(move || {
        let mut received = Vec::new();
        for _ in 0..2 {
            unsafe { let _ = ConnectNamedPipe(pipe.0, None); }
            received.push(receive(&pipe).as_deref().and_then(decode));
            unsafe { let _ = DisconnectNamedPipe(pipe.0); }
        }
        received
    });
    forward(&name, OPEN, &[r"C:\Users".to_owned()]).unwrap();
    forward(&name, SELECT, &[]).unwrap();
    assert_eq!(reader.join().unwrap(), [Some((OPEN, vec![r"C:\Users".to_owned()])), Some((SELECT, Vec::new()))]);
}

#[test]
fn a_released_pipe_is_closed_and_its_name_free_for_the_next_oxyde() {
    use futures::StreamExt;
    let name = format!(r"\\.\pipe\Oxyde.Instance.Test.Release.{}", std::process::id());
    let pipe = create(&name).expect("first owner");
    let released = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (sender, mut receiver) = futures::channel::mpsc::unbounded();
    let served = std::thread::spawn({
        let released = released.clone();
        move || serve(pipe, sender, &released)
    });
    forward(&name, OPEN, &[]).unwrap();
    assert!(futures::executor::block_on(receiver.next()).is_some());
    released.store(true, std::sync::atomic::Ordering::SeqCst);
    wake(&name);
    served.join().unwrap();
    assert!(forward(&name, OPEN, &[]).is_err());
    assert!(create(&name).is_some());
}
