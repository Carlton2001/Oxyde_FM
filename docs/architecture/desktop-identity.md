# Linux desktop identity

What makes GNOME and other freedesktop desktops recognize Oxyde: application identifier, desktop entry and icon. Opening a folder passed as an argument and AppStream metadata come with [the packages](../guides/linux-distribution.md). Windows keeps its identity (`com.oxyde.desktop`, the binary's `.ico`).

## Rules

- Identifier `io.github.Carlton2001.Oxyde`, following the Flathub convention for GitHub-hosted applications: `app_id` of every Oxyde window (`APP_ID` in `src/ui/mod.rs`, ignored on Windows), name of the desktop entry and icon, and `StartupWMClass` for X11. It never changes once published.
- Desktop entry modeled on Nautilus's (`Utility;Core;FileManager;`), name and description translated in all five languages. `Exec=oxyde %U`: Oxyde opens received locations.
- Icon: the SVG logo (`assets/icons/logo_oxyde.svg`), the one the Windows icon derives from, in `hicolor/scalable/apps`.
- `scripts/install-linux-desktop.sh` installs or removes the entry and icon for the user; `--profile` makes Oxyde run on an isolated profile, fixed by environment variables in `Exec`.

## Pitfall

GNOME refuses window queries (`org.gnome.Shell.Introspect` is denied outside allowed applications). `WAYLAND_DEBUG=client` is the way to check what a window announces. Validate the entry with `desktop-file-validate`.
