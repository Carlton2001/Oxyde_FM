use super::*;

#[test]
fn mount_points_are_unescaped_sorted_and_unique() {
    let table = "\
22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw
23 22 0:21 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw
61 23 8:17 / /run/media/user/My\\040Key rw,nosuid - exfat /dev/sdb1 rw
62 22 0:21 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw
63 22 0:5 / net:[4026531840] rw - nsfs nsfs rw
";
    assert_eq!(parse(table), ["/", "/home", "/run/media/user/My Key"]);
}

#[test]
fn malformed_escapes_are_kept_verbatim() {
    assert_eq!(unescape("/a\\9b"), "/a\\9b");
    assert_eq!(unescape("/end\\04"), "/end\\04");
    assert_eq!(unescape("/tab\\011x"), "/tab\tx");
}
