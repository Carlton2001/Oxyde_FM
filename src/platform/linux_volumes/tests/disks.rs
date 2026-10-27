use super::*;
use std::os::unix::fs::symlink;

fn entry(point: &str, fs_type: &str, source: &str) -> MountEntry {
    MountEntry { point: point.into(), fs_type: fs_type.into(), source: source.into() }
}

/// A fake sysfs: an NVMe SSD with two partitions, a rotating disk, and a LUKS volume on it.
fn sysfs() -> tempfile::TempDir {
    let sys = tempfile::tempdir().unwrap();
    let devices = sys.path().join("devices");
    let disk = |name: &str, dev: &str, rotational: &str| {
        let path = devices.join(name);
        std::fs::create_dir_all(path.join("queue")).unwrap();
        std::fs::write(path.join("dev"), format!("{dev}\n")).unwrap();
        std::fs::write(path.join("queue/rotational"), format!("{rotational}\n")).unwrap();
        path
    };
    let partition = |disk: &Path, name: &str| {
        let path = disk.join(name);
        std::fs::create_dir_all(path.join("slaves")).unwrap();
        std::fs::write(path.join("partition"), "1\n").unwrap();
        path
    };
    let nvme = disk("nvme1n1", "259:0", "0");
    let hdd = disk("sda", "8:0", "1");
    let luks = disk("dm-0", "253:0", "1");
    std::fs::create_dir_all(luks.join("slaves")).unwrap();
    let block = sys.path().join("dev/block");
    std::fs::create_dir_all(&block).unwrap();
    symlink(partition(&nvme, "nvme1n1p5"), block.join("259:5")).unwrap();
    symlink(partition(&nvme, "nvme1n1p7"), block.join("259:7")).unwrap();
    let sda1 = partition(&hdd, "sda1");
    symlink(&sda1, block.join("8:1")).unwrap();
    symlink(&sda1, luks.join("slaves/sda1")).unwrap();
    symlink(&luks, block.join("253:0")).unwrap();
    sys
}

fn device(source: &str) -> Option<(u32, u32)> {
    match source {
        "/dev/nvme1n1p7" => Some((259, 7)),
        "/dev/nvme1n1p5" => Some((259, 5)),
        "/dev/sda1" => Some((8, 1)),
        "/dev/mapper/luks" => Some((253, 0)),
        _ => None,
    }
}

#[test]
fn paths_resolve_to_their_whole_disk_through_partitions_and_device_mapper() {
    let sys = sysfs();
    let entries = [
        entry("/", "btrfs", "/dev/nvme1n1p7"),
        entry("/run/media/user/Data", "fuseblk", "/dev/nvme1n1p5"),
        entry("/run/media/user/Backup", "ext4", "/dev/sda1"),
        entry("/home/user/Vault", "ext4", "/dev/mapper/luks"),
        entry("/tmp", "tmpfs", "tmpfs"),
    ];
    let disk = |path: &str| resolve(Path::new(path), &entries, sys.path(), &device);
    let nvme = Disk { id: (259 << 20) + 1, ssd: true };
    assert_eq!(disk("/home/user/file"), nvme);
    assert_eq!(disk("/run/media/user/Data/photo.jpg"), nvme);
    assert_eq!(disk("/run/media/user/Backup/a"), Disk { id: (8 << 20) + 1, ssd: false });
    assert_eq!(disk("/home/user/Vault/a"), disk("/run/media/user/Backup"));
    assert_eq!(disk("/tmp/file"), Disk::default());
    // Mount points match whole components only.
    assert_eq!(disk("/run/media/user/DataOld"), nvme);
}

#[test]
fn network_shares_are_paced_per_host() {
    let sys = sysfs();
    let entries = [
        entry("/", "btrfs", "/dev/nvme1n1p7"),
        entry("/mnt/music", "cifs", "//nas/music"),
        entry("/mnt/video", "nfs4", "nas:/export/video"),
        entry("/mnt/remote", "fuse.sshfs", "user@server:/home/user"),
    ];
    let disk = |path: &str| resolve(Path::new(path), &entries, sys.path(), &device);
    assert_eq!(disk("/mnt/music/a").id, disk("/mnt/video/b").id);
    assert_ne!(disk("/mnt/music/a").id, disk("/mnt/remote/c").id);
    assert!(disk("/mnt/remote/c").id & (1 << 63) != 0 && !disk("/mnt/music/a").ssd);
}
