use super::*;

const TABLE: &str = "\
22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw
23 22 0:21 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw
61 23 8:17 / /run/media/user/My\\040Key rw,nosuid - exfat /dev/sdb1 rw
62 22 0:21 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw
63 22 0:5 / net:[4026531840] rw - nsfs nsfs rw
64 22 0:60 / /mnt/share rw master:9 - cifs //server/My\\040Share rw
";

#[test]
fn mount_points_are_unescaped_sorted_and_unique() {
    assert_eq!(points(&entries(TABLE)), ["/", "/home", "/mnt/share", "/run/media/user/My Key"]);
}

#[test]
fn entries_keep_the_filesystem_type_and_source_after_optional_fields() {
    let entries = entries(TABLE);
    let share = entries.iter().find(|entry| entry.point == "/mnt/share").unwrap();
    assert_eq!((share.fs_type.as_str(), share.source.as_str()), ("cifs", "//server/My Share"));
    assert!(entries.iter().all(|entry| entry.point != "net:[4026531840]"));
}

#[test]
fn malformed_escapes_are_kept_verbatim() {
    assert_eq!(unescape("/a\\9b"), "/a\\9b");
    assert_eq!(unescape("/end\\04"), "/end\\04");
    assert_eq!(unescape("/tab\\011x"), "/tab\tx");
}
