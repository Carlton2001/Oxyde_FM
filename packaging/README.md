# Packaging

Linux package sources, used by `scripts/build-linux-packages.sh`. Procedure, dependencies and CI: [Linux distribution](../docs/guides/linux-distribution.md). The Windows installer is in [`installer/`](../installer/README.md).

| Path in `linux/` | Purpose |
| --- | --- |
| `oxyde.spec` | RPM spec |
| `debian/` | DEB `control`, `copyright`, `conffiles`, maintainer scripts and the `Containerfile` of the Ubuntu 24.04 build image |
| `apparmor/` | Profile allowing user namespaces on Ubuntu, so bubblewrap works |
| `io.github.Carlton2001.Oxyde.desktop` | Desktop entry |
| `io.github.Carlton2001.Oxyde.metainfo.xml` | AppStream metadata |
| `icons/` | Hicolor PNGs 16–512, generated from the SVG logo and kept in Git |

Files under `linux/` are forced to LF by `.gitattributes`, PNGs excepted.
