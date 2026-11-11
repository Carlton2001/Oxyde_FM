# Packaging

Installer and package sources, one folder per system.

## Windows

`windows/oxyde.iss` is the Inno Setup script of the Windows installer, used by `scripts/build-distribution.ps1`. Building, installing and portable use: [Windows distribution](../docs/guides/distribution.md).

## Linux

Linux package sources, used by `scripts/build-linux-packages.sh`. Procedure, dependencies and CI: [Linux distribution](../docs/guides/linux-distribution.md).

| Path in `linux/` | Purpose |
| --- | --- |
| `oxyde.spec` | RPM spec |
| `debian/` | DEB `control`, `copyright`, `conffiles`, maintainer scripts and the `Containerfile` of the Ubuntu 24.04 build image |
| `install.sh` | Release installer: DEB on Debian and Ubuntu, per-user archive elsewhere |
| `signing/oxyde-packages.asc` | Public GPG key of Linux releases; also compiled into Oxyde and written into `install.sh` |
| `apparmor/` | Profile allowing user namespaces on Ubuntu, so bubblewrap works |
| `io.github.Carlton2001.Oxyde.desktop` | Desktop entry |
| `io.github.Carlton2001.Oxyde.metainfo.xml` | AppStream metadata |
| `icons/` | Hicolor PNGs 16–512, generated from the SVG logo and kept in Git |

Files under `linux/` are forced to LF by `.gitattributes`, PNGs excepted.
