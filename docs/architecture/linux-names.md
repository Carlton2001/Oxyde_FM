# Linux names and paths

What a file name means on Linux in shared code: the only separator is `/`, `\` is an ordinary name character, and two names differing only by case are two items, except on filesystems that ignore case. Windows keeps its separators, case handling and tests, grouped under `cfg(windows)`.

## Rules

- **Separator**: `std::path::is_separator` for every native path, so `/` alone outside Windows. A folder `a\b` is one breadcrumb step, one tab title and one completion step.
- **Comparison**: exact outside Windows, byte for byte (`paths_match`). Two paths differing only by case stay two items for quick access and selection.
- **New archive name**: outside Windows, refuses empty, `.`, `..` and `/`, like Nautilus's compress dialog; the rest is allowed.
- **Case per folder**, for collisions prepared before a copy, behind `Platform::file_name_rules`:
  - folder marked `casefold` (ext4, f2fs, tmpfs): caseless and ignoring Unicode normalization differences;
  - vfat, exFAT, SMB/CIFS: caseless;
  - any other system, FUSE included (ntfs-3g): case-sensitive, the Linux norm.

  An error on the cautious (caseless) side only announces one conflict too many. Copying `Report.txt` and `report.txt` to these targets is refused together, before any write.
- **Renaming with only a case change** on a caseless system: refused, like Nautilus (GIO fails the same way); no two-step rename.
- **Archives**: internal names keep the current policy (`\` read as `/`, Windows-incompatible names refused). Accepting them would be a separate decision, with Windows tests.

## Remaining gaps

- ntfs3 mounted with `nocase` and lowntfs-3g with `ignore_case` are treated as case-sensitive: rare, and the error is a failure at copy time, with no loss.

## Running the image tests

`OXYDE_TEST_CASELESS=<mounts>` and `OXYDE_TEST_CASEFOLD=<+F folder>:<ordinary folder>`. A user-owned ext4 casefold image is created with `mkfs.ext4 -O casefold -E root_owner=$(id -u):$(id -g)`, then `chattr +F` on an empty folder.

## Pitfalls

- vfat mounted by UDisks (`utf8`) also ignores the case of accented letters.
- `FS_IOC_GETFLAGS` refuses an `O_PATH` descriptor: reopening `.` for reading is enough to read the `casefold` flag.
