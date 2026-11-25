use super::*;

fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[test]
fn output_and_messages_come_back_with_the_status() {
    let output = output_within(shell("printf out; printf err >&2; exit 3"), Duration::from_secs(10), 16).unwrap();
    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr, b"err");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn a_stuck_tool_and_its_children_are_killed_at_the_deadline() {
    let started = Instant::now();
    let error = output_within(shell("sleep 30 & sleep 30"), Duration::from_millis(200), 16).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_tool_that_closes_its_output_and_keeps_running_is_killed_too() {
    let started = Instant::now();
    let error = output_within(shell("exec >&- 2>&-; sleep 30"), Duration::from_millis(200), 16).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn output_over_the_limit_stops_the_tool() {
    let error = output_within(shell("yes"), Duration::from_secs(10), 1024).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
}
