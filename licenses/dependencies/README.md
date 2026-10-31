# Dependency licenses

`THIRD-PARTY-NOTICES.md` holds license texts and copyright notices, with the crates and versions concerned. `cargo-about` uses the sources resolved by `Cargo.lock`, including GPUI and GPUI Platform from the pinned Zed revision. All targets and build dependencies are included, so this inventory may exceed the crates used in a Windows binary.

`build-distribution.ps1` regenerates these notices at each distribution, before compiling and assembling the ZIP or Setup, even with `-SkipBuild`. Install the generator once locally; the GitHub workflow installs it automatically. The [distribution guide](../../docs/guides/distribution.md) covers installing the official Windows binary. To refresh manually:

```powershell
./scripts/install-cargo-about.ps1
./scripts/update-third-party-licenses.ps1
```

Generation reaches upstream repositories to find licenses missing from Cargo archives. `-Offline` is available when the needed sources are cached, but can lose copyrights specific to licenses fetched remotely. Prefer online generation for a release.

The same file also gathers the NOTICE, COPYRIGHT and AUTHORS files in the crates' local sources, in a supplementary section giving the package, version and source path. Texts are kept unaltered. Cargo authors are not treated as a list of rights holders. `cargo-about` merges identical license texts, with the list of crates concerned; texts whose copyrights differ stay separate.

`inputs.json` records the hashes of the manifests, configuration, template and report. After generation, the distribution checks these hashes and stops on failure. It copies this folder into the portable ZIP and the Setup; grammar notices stay in `licenses/syntaxes/`.

GPUI and GPUI Platform declare Apache-2.0; their licenses are in the single report with the other crates of the pinned Zed revision.

`LUCIDE-LICENSE.txt` keeps the official ISC and MIT/Feather notice; its source is recorded in `LUCIDE-SOURCE.txt`. The original revisions of the project's SVGs were not recorded.

`Inter-LICENSE.txt` and `JetBrainsMono-LICENSE.txt` keep the SIL Open Font 1.1 license of the fonts embedded in the executable, which must ship with them.
