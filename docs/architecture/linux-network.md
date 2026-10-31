# Linux network (SMB and GVfs)

Windows is unchanged.

## Principle: like Nautilus

Nautilus does not speak SMB itself: GIO asks GVfs to mount the location, GVfs asks for credentials through a `GMountOperation`, then the share is readable by URI and, thanks to `gvfsd-fuse`, under a local path `$XDG_RUNTIME_DIR/gvfs/smb-share:server=…,share=…`. Oxyde does the same and browses that path with its ordinary file operations: copies, trash, watching and properties know nothing about URIs.

## Layers

- `platform/remote.rs`: shared types (`RemotePrompt`, `RemoteAnswer`, `RemoteLocation`, `RemoteError`), plus `Platform::is_remote_uri` and `Platform::connect_remote`. Windows keeps the defaults (nothing is a remote URI).
- `platform/linux_gio.rs`: GIO loaded at runtime by `libloading` (`libgio-2.0.so.0`), without a crate or mandatory link: without GLib only the network is missing. Asynchronous calls run in a `GMainContext` private to the calling thread, which answers `ask-password` by blocking until the dialog responds.
  - `connect`: `g_file_mount_enclosing_volume` (already mounted counts as success), then `g_file_get_path` → `Local(FUSE path)`, otherwise `Browse(URI)` (server, network). The FUSE folder may arrive after GIO, especially right after unmounting the same share: `connect` waits 2 s at most.
  - `children`: enumerates a mounted URI. A server listing builds each share's URI from the server, because `standard::target-uri` loses a non-standard port and `g_file_get_child` on a server gives `._share`. `network:///` follows targets (servers discovered by WS-Discovery, DNS-SD, SMB). It first remounts without ever asking for credentials (refresh, restored tab). `Owned` never keeps a null pointer: GLib flags a `g_object_unref(NULL)`.
  - `mounts`: network-type folders in the FUSE directory (`smb-share`, `sftp`, `ftp`, `dav`, `afp-volume`, `nfs`), named by `g_mount_get_name` ("share on server"). Phones and cameras (`mtp`, `gphoto2`) stay out of the list.
  - `unmount`: `g_mount_unmount_with_operation`; a mount GVfs no longer knows (unmounted by Nautilus, FUSE folder still visible for a moment) counts as unmounted.
- `linux_volumes`: GVfs shares join the drive list (`remote`, protocol SMB, SFTP…, URI in `remote_path`), with the drive menu's Unmount action. The GVfs mount tracker's `Mounted`/`Unmounted` signals (session bus) bump the drive revision: `mountinfo` does not change, as the single FUSE mount stays in place. Kernel CIFS mounts stay listed with no action.

## Interface

- "Network" appears in the sidebar and lists `network:///`: servers announced by WS-Discovery (`wsdd`, Windows PCs), DNS-SD (Avahi: NAS, Macs, printers) or SMB browsing, like Nautilus's Other Locations › Networks. A server that does not announce itself opens by address. This view's address bar shows `network:///`, not Windows's `\\`.
- The address bar accepts `smb://server/share` and other GVfs schemes.
- `request_tab_listing` connects before listing any remote URI (`app/remote.rs`): a share opens its local path in the same tab, with no history entry for the URI; a server is listed by URI and remembered for the session. Closing the dialog returns to the previous location; a failure shows "Unable to connect" and GVfs's detail in a notification.
- Fixed `RemoteLogin` dialog (`components/remote_login_dialog.rs`), whose values are kept on input: a field calling back into Oxyde (Enter) cannot be reread during its own update. It shows GVfs's message without its title line, user, domain and password per flags, anonymous connection when the server allows it, "Credentials refused" on the second try, and GTK's three `GPasswordSave` choices as stacked radio buttons with GTK's full sentences, so no language truncates them: forget immediately (default), remember until logout, remember forever. Oxyde writes nothing: GVfs keeps the password in the mount's process, in the keyring's session collection cleared at logout, or in the "Login" keyring where Nautilus finds it.
- The breadcrumb starts at the share, named as in the sidebar; so does the tab.
- Properties of "Network" open the desktop settings, like `ms-settings:network` on Windows: `gnome-control-center network`, `systemsettings kcm_networkmanagement` or `cinnamon-settings network` by `XDG_CURRENT_DESKTOP`, otherwise `nm-connection-editor` (`platform/linux_settings.rs`). Verified under GNOME only.

## Limits

- A connection in progress cannot be canceled: navigating away ignores its result.
- One credential request at a time; a second connection asking meanwhile gives up.
- `ask-question` (unknown SFTP host fingerprint) has no answer: the mount fails. SMB does not ask.
- Without `gvfs-fuse`, a share has no local path: only listing remains possible.
- A tab restored on a since-unmounted share returns to the home folder.

## Testing

- `cargo test linux_gio`: recognized schemes, network mounts in the FUSE folder.
- Local Samba server, then the ignored test (wrong password asked again then canceled, mount, no new prompt once mounted, share list, double unmount):

  ```sh
  podman run -d --name oxyde-smb -p 4450:445 -e USER=oxyde -e PASS=secret \
    -v ~/oxyde-test/smb-share:/storage:Z docker.io/dockurr/samba:latest
  OXYDE_SMB_URI=smb://127.0.0.1:4450/Shared OXYDE_SMB_USER=oxyde OXYDE_SMB_PASSWORD=secret \
    cargo test --offline --lib linux_gio -- --include-ignored --test-threads=1
  ```

- `dbus-monitor --session "type='signal',interface='org.gtk.vfs.MountTracker'"` shows `Mounted` and `Unmounted` for `gio mount`, hence for Nautilus.
