use super::{create, decode, encode, forward, pipe_name, receive};
use windows::Win32::System::Pipes::{ConnectNamedPipe, DisconnectNamedPipe};

#[test]
fn a_message_carries_its_locations_in_order() {
    let locations = vec![r"C:\Users".to_owned(), r"\\server\share\Été".to_owned()];
    assert_eq!(decode(&encode(&locations)), Some(locations));
    assert_eq!(decode(&encode(&[])), Some(Vec::new()));
}

#[test]
fn a_location_cannot_add_a_line() {
    let locations = vec!["C:\\a\nC:\\b".to_owned(), r"C:\c".to_owned()];
    assert_eq!(decode(&encode(&locations)), Some(vec![r"C:\c".to_owned()]));
}

#[test]
fn another_protocol_is_not_a_message() {
    assert_eq!(decode(""), None);
    assert_eq!(decode("oxyde-open 2\nC:\\Users"), None);
    assert_eq!(decode(r"C:\Users"), None);
}

#[test]
fn the_pipe_is_named_after_session_storage_and_build() {
    let name = pipe_name(3, r"C:\Users\Me\AppData\Roaming\Oxyde", false);
    assert!(name.starts_with(r"\\.\pipe\Oxyde.Instance.Release.3."));
    assert_eq!(name, pipe_name(3, r"c:\users\me\appdata\roaming\OXYDE", false));
    assert_ne!(name, pipe_name(4, r"C:\Users\Me\AppData\Roaming\Oxyde", false));
    assert_ne!(name, pipe_name(3, r"D:\Portable\data", false));
    assert_ne!(name, pipe_name(3, r"C:\Users\Me\AppData\Roaming\Oxyde", true));
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
    forward(&name, &[r"C:\Users".to_owned()]).unwrap();
    forward(&name, &[]).unwrap();
    assert_eq!(reader.join().unwrap(), [Some(vec![r"C:\Users".to_owned()]), Some(Vec::new())]);
}
