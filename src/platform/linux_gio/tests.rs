use super::*;

#[test]
fn remote_uris_are_the_gvfs_network_schemes() {
    assert!(is_remote_uri("smb://server/share"));
    assert!(is_remote_uri("SFTP://host/"));
    assert!(is_remote_uri("network:///"));
    assert!(!is_remote_uri("file:///home"));
    assert!(!is_remote_uri("trash://"));
    assert!(!is_remote_uri("/run/user/1000/gvfs"));
    assert!(!is_remote_uri("smb:server"));
}

#[test]
fn only_network_mounts_of_the_fuse_folder_are_shares() {
    assert!(network_mount_name("smb-share:server=nas,share=data".as_ref()));
    assert!(network_mount_name("sftp:host=example.org,user=me".as_ref()));
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
    let refused = connect(&uri, &mut |prompt| {
        prompts.push(prompt.clone());
        if prompt.retry {
            RemoteAnswer::Cancel
        } else {
            RemoteAnswer::Login { user: user.clone(), domain: prompt.domain.clone(), password: "wrong".into(), save: PasswordSave::Never }
        }
    });
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
    })
    .expect("mount");
    let RemoteLocation::Local(path) = location else { panic!("{location:?}") };
    assert!(path.is_dir(), "{path:?}");
    assert!(network_mount_name(path.file_name().unwrap()));
    // Already mounted: no prompt.
    assert_eq!(connect(&uri, &mut |_| panic!("asked again")), Ok(RemoteLocation::Local(path.clone())));
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


