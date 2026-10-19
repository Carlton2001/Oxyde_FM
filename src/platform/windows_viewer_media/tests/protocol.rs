use super::*;

#[test]
fn rejects_oversized_truncated_and_invalid_placements() {
    assert!(read_command(&mut &b"{\"parent\":1}"[..]).is_err());
    assert!(read_command(&mut &vec![b' '; 513][..]).is_err());
    for (parent, rect) in [
        (0, [0, 0, 100, 100]),
        (1, [0, 0, -1, 100]),
        (1, [0, 0, 100, 131_073]),
    ] {
        let command = ViewerMediaCommand {
            parent,
            rect,
            ..Default::default()
        };
        let mut bytes = serde_json::to_vec(&command).unwrap();
        bytes.push(b'\n');
        assert!(read_command(&mut &bytes[..]).is_err());
    }
}

#[test]
fn reads_bounded_commands_in_order_and_detects_eof() {
    let command = ViewerMediaCommand {
        parent: 42,
        rect: [10, 20, 800, 600],
        visible: true,
        paused: false,
    };
    let mut bytes = serde_json::to_vec(&command).unwrap();
    bytes.push(b'\n');
    let mut input = bytes.as_slice();
    assert_eq!(read_command(&mut input).unwrap(), Some(command));
    assert_eq!(read_command(&mut input).unwrap(), None);
}
