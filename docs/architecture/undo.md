# Undo and Redo

Ctrl + Z undoes the last recorded operation, Ctrl + Y or Ctrl + Shift + Z redoes it; the top bar and the list's context menu offer both, labelled with the operation (`components/history_actions.rs`). The top bar hides them in the trash.

## What is recorded

`HistoryManager` (`models/history.rs`) keeps a stack of `Transaction` (`models/transaction.rs`) for the session only: nothing is written to disk, and closing Oxyde forgets it. At most 100 entries; a new operation clears Redo.

| Operation | Recorded by | Undo |
| --- | --- | --- |
| Copy | `systems/file_ops/execution.rs` | Removes what the copy created, each file checked against its hash and identity |
| Move | same | Moves items back, folders and links included |
| Move to the trash | same, and `history_delete.rs` on Windows | Restores the items ([trash](trash.md#restoring)) |
| Rename | `app/rename.rs`, `commands::io::record_captured_rename` | Gives the item its old name back, checked against the identity captured on its handle |
| New folder | `app/rename.rs` | Removes the folder while it is still empty; naming it right after creation belongs to the creation, not a separate rename |

Not recorded: permanent deletions, renames inside archives, archive operations, permission and property changes.

## Guarantees

The journal records what each operation actually did (files and folders created, moved or copied, with their identities and hashes) rather than its request. Undo and Redo act only on items that still match: an item changed, replaced or gone since stops the action with a notice (`ui/errors.rs`, `UndoMoveChanged`, `UndoNewFolderNotEmpty`…), and nothing is overwritten.

- A copy that replaced existing files cannot be undone: the old contents are deliberately not kept. Its Redo is refused too, as it would need a new conflict resolution.
- One Undo or Redo runs at a time; a press while one runs is ignored.
- A folder recreated by Undo or Redo takes back the attributes its source had (`directory_attributes`, Linux only).

Platform details of the effects: [copies](copies.md), [moves](moves.md), [links](links.md), [deletion](../diagnostics/delete.md).
