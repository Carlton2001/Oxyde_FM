# Retained dependencies and audit

User decision: keep `paste 1.0.15`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`, `bincode 1.3.3`, `yaml-rust 0.4.5`, `zvariant 5.15.0`, `zvariant_derive 5.15.0` and `zbus_names 4.3.4` until a concrete reason justifies migrating. A maintenance advisory does not prove a vulnerability; it stays visible.

| Dependency | Use | Migration path |
| --- | --- | --- |
| `paste` | Compile-time macro via OpenEXR/pulp | `pastey` once the compatible published chain adopts it; do not remove OpenEXR to hide the advisory |
| `rustybuzz` | SVG text shaping via GPUI/usvg | `harfrust`, with a consistent SVG stack update |
| `ttf-parser` | GPUI glyph checks and SVG dependencies | Fontations/`skrifa`, checking API and rendering changes |
| `bincode` | Syntect's bundled syntax and theme dumps (`default-syntaxes`) | A Syntect release that drops it; Oxyde does not call it directly |
| `yaml-rust` | Syntect's `yaml-load`, which compiles the vendored Sass, SCSS and PowerShell grammars at run time (`ui/app/viewer/text/syntax.rs`) | Precompile these grammars into a dump at build time and drop `yaml-load`, or a Syntect release on a maintained YAML parser |
| `zvariant`, `zvariant_derive`, `zbus_names` | zbus 5 on Linux only: Oxyde's UDisks2 calls and GPUI's Linux backend (ashpd, AccessKit, oo7) | zbus 6, which merges them as its `wire` module, once GPUI's Linux backend adopts it; moving Oxyde alone would add a second zbus |

Adding an alternative to the manifest does not replace an indirect dependency. Do not implicitly introduce a GPUI fork or Git source to clear advisories. The lockfile may contain packages absent from the active graph: do not confuse the two inventories.

[`rustsec-windows-2026-10-09.json`](rustsec-windows-2026-10-09.json) is the historical report behind this decision: three maintenance advisories, no published vulnerability for the Windows x86_64 filters. It is not a current analysis.

## When to reconsider

- A published vulnerability affects a version and path actually used.
- A compatible GPUI/OpenEXR version adopts the replacements.
- A concrete functional defect justifies a migration or backport.

Before a typography migration, check accents in all five languages, ligatures, complex scripts, bidirectional text, fallback fonts, emoji/missing glyphs and SVG dimensions/colors. Pixel tests do not replace these checks.

## Running the Windows audit

```powershell
./scripts/audit-security.ps1
```

On Windows the script uses the official x64 `cargo-audit` 0.22.2 binary, installed in `target/security-tools/bin`. The installer checks pinned SHA-256 hashes of the archive before extraction and of the binary before running it, then its version; a binary with the expected hash is reused. Other systems use `cargo install`.

The RustSec database and yanked index refresh at each audit, and nothing is cached by the workflow. It applies Windows x86_64 filters, blocks `yanked` versions, shows maintenance advisories and keeps the JSON report as an artifact, even after failure, through `.github/workflows/security.yml`. It does not migrate dependencies.

To test the installer offline with an intact copy of the official ZIP:

```powershell
./scripts/tests/install-cargo-audit.tests.ps1 -ArchivePath C:\path\cargo-audit-x86_64-pc-windows-msvc-v0.22.2.zip
```
