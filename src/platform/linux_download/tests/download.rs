use super::*;

fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[test]
fn only_https_is_downloaded() {
    let error = download("http://example.com/latest.json", 1024).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn body_within_the_limit_is_returned() {
    assert_eq!(run(shell("printf 'manifest'"), 8).unwrap(), b"manifest");
}

#[test]
fn body_over_the_limit_stops_the_tool() {
    let error = run(shell("yes"), 1024).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
}

#[test]
fn failure_reports_the_tool_last_message() {
    let error = run(shell("echo 'first' >&2; echo '(22) The requested URL returned error: 404' >&2; exit 22"), 1024).unwrap_err();
    assert_eq!(error.to_string(), "sh: (22) The requested URL returned error: 404");
}
