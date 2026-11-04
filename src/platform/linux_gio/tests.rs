use super::*;

#[test]
fn remote_uris_are_the_gvfs_network_schemes() {
    assert!(is_remote_uri("smb://server/share"));
    assert!(is_remote_uri("SFTP://host/"));
    assert!(is_remote_uri("network:///"));
    // WebDAV announced by DNS-SD, as GNOME's personal file sharing is.
    assert!(is_remote_uri("dav+sd://files%20on%20host._webdav._tcp.local"));
    assert!(!is_remote_uri("file:///home"));
    assert!(!is_remote_uri("trash://"));
    assert!(!is_remote_uri("/run/user/1000/gvfs"));
    assert!(!is_remote_uri("smb:server"));
}

#[test]
fn only_network_mounts_of_the_fuse_folder_are_shares() {
    assert!(network_mount_name("smb-share:server=nas,share=data".as_ref()));
    assert!(network_mount_name("sftp:host=example.org,user=me".as_ref()));
    assert!(network_mount_name("dav+sd:host=files._webdav._tcp.local".as_ref()));
    assert!(!network_mount_name("smb-server:server=nas".as_ref()));
    assert!(!network_mount_name("mtp:host=Phone".as_ref()));
}

/// A live Samba share: `OXYDE_SMB_URI=smb://127.0.0.1:4450/Shared OXYDE_SMB_USER=oxyde
/// OXYDE_SMB_PASSWORD=secret cargo test linux_gio -- --ignored --test-threads=1`.
#[test]
#[ignore = "needs a Samba server, see the comment"]
fn mounts_lists_and_unmounts_a_live_share() {
    let uri = std::env::var("OXYDE_SMB_URI").expect("OXYDE_SMB_URI");
    let user = std::env::var("OXYDE_SMB_USER").expect("OXYDE_SMB_USER");
    let password = std::env::var("OXYDE_SMB_PASSWORD").expect("OXYDE_SMB_PASSWORD");
    let (server, share) = uri.trim_end_matches('/').rsplit_once('/').expect("smb://server/share");
    let share_mount = |mounts: &[RemoteMount]| mounts.iter().find(|mount| mount.uri.to_lowercase().starts_with(&uri.to_lowercase())).cloned();
    if let Some(mount) = share_mount(&mounts()) {
        unmount(&mount.path).expect("unmount a previous run");
    }

    // A wrong password is asked again, with `retry`; cancelling then fails quietly.
    let mut prompts = Vec::new();
    let never = AtomicBool::new(false);
    let refused = connect(&uri, &mut |prompt| {
        prompts.push(prompt.clone());
        if prompt.retry {
            RemoteAnswer::Cancel
        } else {
            RemoteAnswer::Login { user: user.clone(), domain: prompt.domain.clone(), password: "wrong".into(), save: PasswordSave::Never }
        }
    }, &never);
    // A password saved in the keyring mounts without asking: nothing to refuse then.
    if let Ok(RemoteLocation::Local(path)) = &refused {
        assert!(prompts.is_empty(), "{prompts:?}");
        eprintln!("Saved credentials mounted the share: the refusal is not tested");
        unmount(path).expect("unmount with saved credentials");
    } else {
        assert_eq!(refused, Err(RemoteError::Cancelled));
        assert_eq!(prompts.len(), 2, "{prompts:?}");
        assert!(prompts[0].needs_password && !prompts[0].retry && prompts[1].retry);
    }

    let location = connect(&uri, &mut |prompt| RemoteAnswer::Login {
        user: user.clone(), domain: prompt.domain.clone(), password: password.clone(), save: PasswordSave::Never,
    }, &never)
    .expect("mount");
    let RemoteLocation::Local(path) = location else { panic!("{location:?}") };
    assert!(path.is_dir(), "{path:?}");
    assert!(network_mount_name(path.file_name().unwrap()));
    // Already mounted: no prompt.
    assert_eq!(connect(&uri, &mut |_| panic!("asked again"), &never), Ok(RemoteLocation::Local(path.clone())));
    let mount = share_mount(&mounts()).expect("listed share");
    assert_eq!(mount.path, path);
    assert!(!mount.name.is_empty());

    let shares = children(&format!("{server}/")).expect("server shares");
    let listed = shares.iter().find(|child| child.name.eq_ignore_ascii_case(share)).expect("share listed");
    assert!(listed.directory);
    assert!(listed.uri.eq_ignore_ascii_case(&uri.trim_end_matches('/')), "{listed:?}");

    unmount(&path).expect("unmount");
    assert!(share_mount(&mounts()).is_none());
    unmount(&path).expect("unmounting twice succeeds");
}



/// Writes the GTK bookmark file Nautilus reads, under a temporary configuration folder:
/// `cargo test linux_gio::tests::recent_servers -- --ignored`.
#[test]
#[ignore = "changes XDG_CONFIG_HOME for the process"]
fn recent_servers_round_trip_through_the_gtk_bookmark_file() {
    let config = std::env::temp_dir().join(format!("oxyde-servers-{}", std::process::id()));
    // SAFETY: the only test of its run, see the comment.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &config) };
    assert!(servers::recent().is_empty());
    servers::remember("smb://nas/data", None);
    servers::remember("sftp://host/", Some("Host"));
    let recent = servers::recent();
    assert_eq!(recent.iter().map(|server| server.uri.as_str()).collect::<Vec<_>>(), ["smb://nas/data", "sftp://host/"]);
    assert_eq!(recent[1].title.as_deref(), Some("Host"));
    assert!(std::fs::read_to_string(config.join("gtk-4.0/servers")).unwrap().contains("<xbel"));
    servers::forget("smb://nas/data");
    servers::forget("smb://unknown/");
    assert_eq!(servers::recent().len(), 1);
    let _ = std::fs::remove_dir_all(&config);
}

/// GVfs waits long on an unreachable server; setting the flag gives up at once.
#[test]
#[ignore = "needs GVfs"]
fn a_cancelled_connection_gives_up() {
    let cancel = AtomicBool::new(true);
    let started = std::time::Instant::now();
    let result = connect("smb://192.0.2.1/share", &mut |_| RemoteAnswer::Cancel, &cancel);
    assert_eq!(result, Err(RemoteError::Cancelled));
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
}

/// A live SFTP server GVfs has never seen, by a LAN address: GVfs skips the host key check
/// for localhost. `OXYDE_SFTP_URI=sftp://oxyde@<LAN IP>:2222/ OXYDE_SFTP_PASSWORD=secret
/// cargo test linux_gio -- --ignored --test-threads=1`, then `ssh-keygen -R '[<LAN IP>]:2222'`
/// forgets the accepted key. Refusing the key counts as cancelled, not as a failure.
#[test]
#[ignore = "needs an SFTP server unknown to known_hosts, see the comment"]
fn an_unknown_sftp_host_key_is_asked_then_accepted() {
    let uri = std::env::var("OXYDE_SFTP_URI").expect("OXYDE_SFTP_URI");
    let password = std::env::var("OXYDE_SFTP_PASSWORD").expect("OXYDE_SFTP_PASSWORD");
    let never = AtomicBool::new(false);
    let mut questions = Vec::new();
    let refused = connect(&uri, &mut |prompt| {
        questions.push(prompt.clone());
        match prompt.choices.len() {
            0 => RemoteAnswer::Login { user: prompt.user.clone(), domain: String::new(), password: password.clone(), save: PasswordSave::Never },
            count => RemoteAnswer::Choice(count - 1),
        }
    }, &never);
    eprintln!("{questions:#?}");
    assert_eq!(refused, Err(RemoteError::Cancelled), "{questions:?}");
    let questions: Vec<_> = questions.into_iter().filter(|prompt| !prompt.choices.is_empty()).collect();
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(questions[0].choices.len() >= 2 && !questions[0].message.is_empty(), "{questions:?}");

    let location = connect(&uri, &mut |prompt| {
        if prompt.choices.is_empty() {
            RemoteAnswer::Login { user: prompt.user.clone(), domain: String::new(), password: password.clone(), save: PasswordSave::Never }
        } else {
            RemoteAnswer::Choice(0)
        }
    }, &never)
    .expect("mount after accepting the key");
    let RemoteLocation::Local(path) = location else { panic!("{location:?}") };
    assert!(path.join("upload").is_dir(), "{path:?}");
    unmount(&path).expect("unmount");
}

#[test]
fn uris_compare_through_every_escaping_level() {
    assert_eq!(
        unescaped("dav+sd://biluf%27s%20files._webdav._tcp.local"),
        unescaped("dav+sd://biluf%2527s%2520files._webdav._tcp.local/"),
    );
    assert_ne!(unescaped("smb://nas/a"), unescaped("smb://nas/b"));
    assert_eq!(unescaped("smb://nas/100%"), "smb://nas/100%");
}
