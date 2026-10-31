# Linux Properties

The Properties window on Linux: Unix permissions (owner, group, access, execution) and Linux-specific information, modeled on Nautilus. Windows keeps its tabs (General, Shortcut), its system-properties button, and receives neither permissions nor MIME type.

## Rules

- **Permissions tab** for a single item, like Nautilus's Permissions page: owner, group and others, each with an access level.
  - File: None, Read only, Read and write. The owner has no "None". The "Allow executing file as program" box sets the three execute bits. As in Nautilus (GIO's rule), it appears only for a type that can execute: subtype of `application/x-executable` or `text/plain`.
  - Folder: None, List files only, Access files, Create and delete files. The owner has no "None" here either.
  - A combination matching no level shows as is (`rw-`, …) and stays a menu choice, to return to the original state.
- **Who can change**: the owner or root. Otherwise the tab is read-only, with Nautilus's message. The group is chosen from the user's groups.
- **Administrator**: when Oxyde runs as root, owner and group open to all accounts: every NSS account, named "login – Real name" from GECOS, and every group. Oxyde does not ask for elevation.
- **Applying** happens on OK, like the rest of the window, off the UI thread. It targets the object opened at load (same `dev`/inode, same rights and group). If the item has changed, nothing is applied and a message invites reopening. setuid, setgid and sticky bits are kept, except when the kernel removes them on a group change.
- **Account names** through NSS (`getent`), like GLib's calls: local, SSSD and LDAP accounts. Otherwise the number shows.
- A symbolic link describes its target; a broken link has no Permissions tab.
- **Multiple selection**, like Nautilus:
  - one level per class for files and one for folders when the selection has both;
  - a class whose items differ shows "Mixed"; that choice stays in the menu and gives each item back its own bits;
  - owner and group shown, common or "Several", not editable; no execute box;
  - editable if the user can change each item of the type; Content tab when the selection has only editable folders;
  - OK applies item by item, each checked as a single item. Failures are counted and the first named; other items change, and a new OK retries only what remains;
  - account names are kept a few seconds, so a large selection does not run one `getent` per item.
- **Content tab** (folders only): Nautilus 50's "Change permissions of enclosed items" dialog, without a sub-screen. For each class, a level for files and one for folders, prefilled from the umask (read from `/proc/self/status`). A class whose bits match no level stays "Unchanged". Nothing changes inside until the "Change permissions of enclosed items" box is checked; OK then applies these levels at any depth, before changing the folder itself:
  - each folder changes after its content, so one closing itself to its owner is still traversed;
  - a closed folder the change opens is changed first, then traversed;
  - links are never followed and other filesystems never visited;
  - each item changes through the descriptor whose type was checked;
  - failures are counted, the first is named, other items change.
- **Filesystem without Unix rights**: ntfs-3g (without its `permissions` option) and FAT accept `chmod` and keep nothing. Oxyde rereads the object after the change and reports it, rather than closing as if all went well.
- **Type** like Nautilus, "PNG image (image/png)", by GIO's algorithm (xdgmime) on the shared-mime-info database:
  - by name (`globs2`): exact name, otherwise the longest `*.ext` suffix, then other patterns while fewer than two types remain; exact case first;
  - one type by name is enough; otherwise content (`magic`) decides: a rule of priority 80 or more wins, otherwise the first name type deriving from it;
  - no recognized name: content, otherwise text if the first 128 bytes have no control character, otherwise unknown; an empty file is `application/x-zerosize`.

  Description in the interface language, otherwise the database's. Folders keep Oxyde's label, often absent from the database. Pipes, sockets and devices are named by nature, without being opened.

Code: `models/permissions.rs`, `FileProperties` (`permissions`, `mime_type`), `Platform` (`unix_permissions`, `set_unix_permissions`, `mime_type`), `commands::io::set_selection_permissions`, the Properties window's `tabs` module.

## Remaining differences from Nautilus

- No elevation (`admin://` and PolKit): the owner changes only if Oxyde runs as root.
- Nautilus applies each choice immediately; Oxyde on OK.

## Pitfalls

- ntfs-3g accepts `chmod` and `chgrp` and keeps nothing: rereading the object is the only way to know.
- The MIME database classes all ELF as programs (`application/x-executable`); libraries differ by name (`*.so*`). Sorting by `e_type` would hide the execute box of PIE programs, that is nearly all of them.
- A list of folders to traverse, rather than recursion, lets each folder change after its content without fearing deep trees.
