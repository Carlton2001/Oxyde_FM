//! Directory name rules used by transfer preflight. Linux compares names exactly,
//! except in a `casefold` folder (ext4, f2fs, tmpfs) and on the file systems of
//! removable media and Windows shares, which ignore case.
use super::FileNameRules;
use rustix::fs::{Mode, OFlags};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
};
use unicode_normalization::UnicodeNormalization;

/// `FS_CASEFOLD_FL`, absent from rustix's flag names.
const CASEFOLD: u32 = 0x4000_0000;

/// `statfs` magic numbers of file systems whose lookups ignore case.
const CASELESS: [i64; 6] = [
    0x4d44,      // vfat, msdos
    0x2011_bab0, // exFAT
    0xff53_4d42, // CIFS
    0xfe53_4d42, // SMB2/3
    0x517b,      // smbfs
    0x482b,      // HFS+ (HFSX volumes are sensitive; prudence costs one conflict)
];

/// `FUSE_SUPER_MAGIC`: the mount table tells which file system serves it.
const FUSE: i64 = 0x6573_5546;

pub(super) fn rules(directory: &File) -> io::Result<FileNameRules> {
    if casefold(directory) {
        return Ok(FileNameRules::linux_casefold());
    }
    let kind = rustix::fs::fstatfs(directory)?.f_type as i64;
    let caseless = CASELESS.contains(&kind) || (kind == FUSE && fuse_type(directory).is_some_and(|kind| caseless_fuse(&kind)));
    Ok(if caseless { FileNameRules::linux_caseless() } else { FileNameRules::sensitive() })
}

/// The FUSE file system type of the mount holding `directory`, from the mount table.
fn fuse_type(directory: &File) -> Option<String> {
    let status = rustix::fs::statx(directory, "", rustix::fs::AtFlags::EMPTY_PATH, rustix::fs::StatxFlags::MNT_ID).ok()?;
    if status.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 {
        return None;
    }
    let table = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    mount_type(&table, status.stx_mnt_id)
}

/// `id parent dev root point options [optional…] - fs_type source super_options`
fn mount_type(table: &str, id: u64) -> Option<String> {
    table.lines()
        .find(|line| line.split(' ').next().and_then(|field| field.parse::<u64>().ok()) == Some(id))
        .and_then(|line| line.split_once(" - "))
        .and_then(|(_, filesystem)| filesystem.split(' ').next())
        .map(str::to_owned)
}

/// FUSE servers of case-insensitive file systems: block devices (exFAT, NTFS, whose
/// Windows names prudence treats alike) and GVfs, which serves Windows shares.
fn caseless_fuse(kind: &str) -> bool {
    kind == "fuseblk" || ["gvfsd-fuse", "exfat", "ntfs", "smb", "cifs"].iter().any(|name| kind.starts_with("fuse") && kind.contains(name))
}

/// The flag needs a readable descriptor; a held directory may be `O_PATH`. Systems
/// without inode flags (FAT, FUSE) have no casefold folder either.
fn casefold(directory: &File) -> bool {
    let flags = rustix::fs::ioctl_getflags(directory).or_else(|_| {
        let readable = rustix::fs::openat(directory, ".", OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())?;
        rustix::fs::ioctl_getflags(readable)
    });
    flags.is_ok_and(|flags| flags.bits() & CASEFOLD != 0)
}

/// One character for one, as FAT, exFAT and Windows servers upcase names.
pub(super) fn upcase(name: &OsStr) -> OsString {
    let Some(name) = name.to_str() else { return name.to_os_string() };
    name.chars()
        .map(|character| {
            let mut upper = character.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(single), None) => single,
                _ => character,
            }
        })
        .collect::<String>()
        .into()
}

/// The kernel's casefold compares normalized (NFD) and case-folded names; upper then
/// lower case approaches full folding (« ß » and « SS » meet).
pub(super) fn casefold_key(name: &OsStr) -> OsString {
    let Some(name) = name.to_str() else { return name.to_os_string() };
    name.nfd()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfd()
        .collect::<String>()
        .into()
}

#[cfg(test)]
#[path = "linux_file_names/tests/rules.rs"]
mod tests;
