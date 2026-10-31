# Examples

Development tools, not API samples, run with `cargo run --offline --example <name>`. None ships in a distribution.

| Example | Purpose | Guide |
| --- | --- | --- |
| `archive_compression_benchmark` | Production compression against the same codec used directly | [Archive compression](../docs/diagnostics/archive-compression.md) |
| `archive_extraction_benchmark` | Production extraction without and with Turbo | [Archive compression](../docs/diagnostics/archive-compression.md) |
| `copy_concurrency_benchmark` | Turbo copy worker counts | [Copy](../docs/diagnostics/copy.md) |
| `delete_tree_benchmark` | Permanent deletion without and with Turbo | [Deletion](../docs/diagnostics/delete.md) |
| `generate_application_icons` | Regenerate raster icons from the SVG logo, through `scripts/distribution-icon.ps1` | [Distribution](../docs/guides/distribution.md) |
