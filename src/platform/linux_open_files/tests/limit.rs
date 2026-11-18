use super::*;

#[test]
fn programs_started_receive_the_limit_oxyde_received() {
    let limit = getrlimit(Resource::Nofile);
    if limit.current >= limit.maximum.or(Some(UNLIMITED_CEILING)) {
        return;
    }
    let received = limit.current.unwrap();
    raise().unwrap();
    assert!(getrlimit(Resource::Nofile).current > Some(received));
    let mut command = Command::new("sh");
    command.args(["-c", "ulimit -Sn"]);
    leave_out_of(&mut command);
    let output = command.output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), received.to_string());
}
