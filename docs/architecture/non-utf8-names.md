# Names that are not valid Unicode

A Unix name is a byte sequence: a stick formatted under another encoding or an old archive can leave names that are not UTF-8 text, such as `photo-\xe9t\xe9.jpg` in Latin-1. An NTFS name is a sequence of UTF-16 units that Windows does not check: a faulty program can leave a lone surrogate in it. Oxyde keeps its paths as `String` in the interface, selections and journal.

Lossy conversion would map each invalid byte to `U+FFFD`, so `photo-\xe9t\xe9.jpg` and a file really named `photo-�t�.jpg` would get the same path, and acting on one could hit the other.

## Rules

- **Reversible text** (`utils::path_text`): a valid path keeps its text. In a non-UTF-8 Unix path, each invalid byte is written as NUL followed by two hex digits; in a Windows path, each lone surrogate as NUL followed by four. No path contains NUL: the text stays distinct from any other file, and code opening it without `decode` fails (`InvalidInput`) rather than reaching its lookalike. Listing, search and trash use it.
- **Display like GLib**: `name` shows `U+FFFD`, and the interface adds "(invalid encoding)" in the list, grid and tooltip. Tab, breadcrumb and address bar show `U+FFFD`.
- **Paths keeping the bytes**: opening such a folder (listing and watching), renaming such an item (starting from the `U+FFFD` name, which is the remedy), and restoring or purging the trash, which selects items by their reversible text.
- **Explicit refusal elsewhere**: copying, moving, deleting, trashing, compressing and extracting rebuild paths and journal text. They refuse such an item before any effect (`UnrepresentablePathFailure`, checked at preparation and in the operation queue). The translated message invites renaming. Other actions (open, preview, Properties) fail without effect by construction.

Accepting those operations, as Nautilus does, would require keeping bytes throughout the operation engine and the Undo/Redo journal.

Code: `models/file_entry.rs` (`has_invalid_name`), `models/error.rs`, `file_ops/scheduler.rs`, `ops/conflicts.rs`, `commands/io.rs`, `commands/search.rs`.

## Windows

A Windows API reads its text up to the first NUL. Handed the escaped text, it would act on a shorter path, `C:\dossier\foo` for `C:\dossier\foo␀D800.txt`, which may be another file. Every path Oxyde converts to UTF-16 itself goes through `path_text::wide` or `path_text::hstring`, which decode the text and refuse a NUL left over. The worker sandbox decodes its arguments the same way, since a command line also ends at its first NUL. The Rust standard library refuses a NUL by itself.

The previews (icons, thumbnails, PDF, video), opening, Properties, the shell context menu, the clipboard and dragging out of Oxyde use the decoded path. File operations refuse such a name, as on Linux.

Code: `utils/path_text.rs`, `platform/windows_worker_sandbox.rs`, `platform/windows_shell_selection.rs` (`shell_wide`).

## Pitfalls

- NUL is the only byte absent from every Unix path, and the only unit absent from every NTFS name. As an escape it makes a forgotten `decode` harmless rather than dangerous, which avoided reworking some 370 `String`/`Path` conversions. On Windows this holds only through `wide` and `hstring`: a new UTF-16 conversion of a path must use them.
- ntfs-3g refuses these names (`EILSEQ`): trials use Btrfs, ext4 or tmpfs. On Windows, a test creates a lone-surrogate name with `OsString::from_wide`.
