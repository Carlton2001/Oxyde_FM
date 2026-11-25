use super::*;

fn entry(point: &str, fs_type: &str, source: &str) -> MountEntry {
    MountEntry { point: point.into(), fs_type: fs_type.into(), source: source.into(), read_only: false }
}

fn listed(entries: &[MountEntry]) -> Vec<(String, String)> {
    drives(entries).into_iter().map(|drive| (drive.path, drive.drive_type)).collect()
}

#[test]
fn system_mounts_and_snapshots_are_not_drives() {
    let home = dirs::home_dir().unwrap();
    let inside_home = home.join("Disque").to_string_lossy().into_owned();
    let entries = [
        entry("/", "btrfs", "/dev/nvme0n1p3"),
        entry("/boot", "ext4", "/dev/nvme0n1p2"),
        entry("/boot/efi", "vfat", "/dev/nvme0n1p1"),
        entry("/home", "btrfs", "/dev/nvme0n1p3"),
        entry("/var/log", "btrfs", "/dev/nvme0n1p3"),
        entry("/.snapshots", "btrfs", "/dev/nvme0n1p3"),
        entry("/run/media/me/USB", "vfat", "/dev/sdb1"),
        entry(&inside_home, "ext4", "/dev/sdc1"),
        entry("/mnt/c", "9p", "C:\\"),
        entry("/mnt/wslg/distro", "ext4", "/dev/sdd"),
        entry("/usr/lib/wsl/drivers", "9p", "drivers"),
    ];
    assert_eq!(listed(&entries), [
        ("/".into(), "fixed".into()),
        ("/run/media/me/USB".into(), "fixed".into()),
        (inside_home, "fixed".into()),
        ("/mnt/c".into(), "fixed".into()),
    ]);
}

#[test]
fn network_mounts_are_remote_wherever_they_are() {
    let entries = [
        entry("/srv/nas", "nfs4", "nas:/data"),
        entry("/media/ssh", "fuse.sshfs", "me@host:"),
        entry("/media/cloud", "fuse.rclone", "drive:"),
        entry("/media/share", "9p", "share"),
        entry("/media/ntfs", "fuseblk", "/dev/sde1"),
        entry("/var/lib/nfs", "nfs", "nas:/var"),
    ];
    assert_eq!(listed(&entries), [
        ("/srv/nas".into(), "remote".into()),
        ("/media/ssh".into(), "remote".into()),
        ("/media/cloud".into(), "remote".into()),
        ("/media/share".into(), "remote".into()),
        ("/media/ntfs".into(), "fixed".into()),
    ]);
}
