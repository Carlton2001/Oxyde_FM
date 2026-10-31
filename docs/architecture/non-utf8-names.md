# Non-UTF-8 names on Linux

A Unix name is a byte sequence: a stick formatted under another encoding or an old archive can leave names that are not UTF-8 text, such as `photo-\xe9t\xe9.jpg` in Latin-1. Oxyde keeps its paths as `String` in the interface, selections and journal. Windows is not concerned: `encode` and `decode` keep its original conversion.

Lossy conversion would map each invalid byte to `U+FFFD`, so `photo-\xe9t\xe9.jpg` and a file really named `photo-�t�.jpg` would get the same path, and acting on one could hit the other.

## Rules

- **Reversible text** (`utils::path_text`): a valid path keeps its text. In a non-UTF-8 path, each invalid byte is written as NUL followed by two hex digits. No Unix path contains NUL: the text stays distinct from any other file, and code opening it without `decode` fails (`InvalidInput`) rather than reaching its lookalike. Listing, search and trash use it.
- **Display like GLib**: `name` shows `U+FFFD`, and the interface adds "(invalid encoding)" in the list, grid and tooltip. Tab, breadcrumb and address bar show `U+FFFD`.
- **Paths keeping the bytes**: opening such a folder (listing and watching), renaming such an item (starting from the `U+FFFD` name, which is the remedy), and restoring or purging the trash, which selects items by their reversible text.
- **Explicit refusal elsewhere**: copying, moving, deleting, trashing, compressing and extracting rebuild paths and journal text. They refuse such an item before any effect (`UnrepresentablePathFailure`, checked at preparation and in the operation queue). The translated message invites renaming. Other actions (open, preview, Properties) fail without effect by construction.

Accepting those operations, as Nautilus does, would require keeping bytes throughout the operation engine and the Undo/Redo journal.

Code: `models/file_entry.rs` (`has_invalid_name`), `models/error.rs`, `file_ops/scheduler.rs`, `ops/conflicts.rs`, `commands/io.rs`, `commands/search.rs`.

## Pitfalls

- NUL is the only byte absent from every Unix path. As an escape it makes a forgotten `decode` harmless rather than dangerous, which avoided reworking some 370 `String`/`Path` conversions.
- ntfs-3g refuses these names (`EILSEQ`): trials use Btrfs, ext4 or tmpfs.
