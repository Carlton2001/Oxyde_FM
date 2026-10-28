use super::*;

/// Runs a shell script in the sandbox of `profile`, given `source` open.
fn run(profile: Profile, source: Option<&Path>, script: &str) -> std::process::Output {
    let source = source.map(|path| (path, File::open(path).unwrap()));
    let mut launch = command(Path::new("/usr/bin/sh"), profile, source).unwrap();
    launch.command.arg("-c").arg(script).arg("sandbox");
    if let Some(source) = &launch.source {
        launch.command.arg(source);
    }
    launch.command.stdin(Stdio::null()).output().unwrap()
}

#[test]
fn a_worker_reads_its_document_and_nothing_else_of_the_user() {
    if !available() {
        eprintln!("skipped: bubblewrap is unusable here");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let document = directory.path().join("report.pdf");
    std::fs::write(&document, b"%PDF-1.7 only this").unwrap();
    std::fs::write(directory.path().join("secret.txt"), b"private").unwrap();
    let script = format!("cat \"$1\"; echo; ls /oxyde; test -e '{}' && echo visible",
        directory.path().join("secret.txt").display());
    let output = run(Profile::Document, Some(&document), &script);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "%PDF-1.7 only this\nsource.pdf\n", "{output:?}");
}

#[test]
fn a_worker_has_no_network_namespace_or_debugger() {
    if !available() || seccomp::filter().is_none() || !Path::new("/usr/bin/python3").exists() {
        eprintln!("skipped: bubblewrap, libseccomp or Python is missing");
        return;
    }
    let script = "python3 -I -c '
import ctypes, errno, os, socket
for family in (socket.AF_INET, socket.AF_INET6, socket.AF_NETLINK):
    try: socket.socket(family); print(\"socket\", family)
    except OSError as error: assert error.errno == errno.EAFNOSUPPORT, error
socket.socket(socket.AF_UNIX).close()
libc = ctypes.CDLL(None, use_errno=True)
assert libc.unshare(0x10000000) == -1 and ctypes.get_errno() == errno.EPERM
assert libc.ptrace(16, 1, 0, 0) == -1 and ctypes.get_errno() == errno.EPERM
print(\"confined\")'";
    let output = run(Profile::Video, None, script);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "confined\n", "{output:?}");
}

#[test]
fn only_the_player_reaches_the_sound_server() {
    if !available() {
        eprintln!("skipped: bubblewrap is unusable here");
        return;
    }
    let Some(runtime) = dirs::runtime_dir().filter(|runtime| runtime.join("pulse/native").exists()) else {
        eprintln!("skipped: no sound server");
        return;
    };
    let script = format!("test -S '{}' && echo sound", runtime.join("pulse/native").display());
    assert_eq!(run(Profile::Player, None, &script).stdout, b"sound\n");
    assert_eq!(run(Profile::Video, None, &script).stdout, b"");
}
