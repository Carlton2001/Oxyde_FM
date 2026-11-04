# Archive paths

In Windows production, codecs run in workers; the parent owns the files and decides effects. A worker failure must never trigger less restricted local parsing.

## Paths and responsibilities

| Use | Entry point | Contract |
| --- | --- | --- |
| Navigation and search index | `Platform::list_archive_directory`, worker `oxyde-archive-list-worker` | Validated metadata, relative internal paths; 512 MiB, 120 s, 32 MiB response at most |
| Extraction | `systems/archive_io.rs`, `archive_io::decode` | Decode broker; bounded reads/writes and parent-owned outputs. ZIP reads its source through a 256 KiB window kept across seeks |
| Creation and rebuild | `systems/archive_encode.rs`, `archive_encode::run` | Same encoder via `Platform::run_archive_io_worker` |
| Modifying an archive | `archive_workspace/native_commit.rs` | Preparation, checks of the original, publication with backup and recovery |

Decode/Encode and listing/index workers run in capability-less LPAC. The parent validates paths, types, sizes and sequences, and holds sources and outputs. Typed errors are translated at render time, without exposing internal codes.

The Decode/Encode transport wakes the supervisor on receipt or write completion, with capacity-one queues and one credit per frame read. Helpers may block on pipes; the supervisor keeps its pause, cancel and timeout checks. Its idle wait asks for at most 5 ms before rechecking, with no absolute guarantee on scheduling or native callbacks. A partial frame does not block this monitoring. Helpers are never joined without a bound at shutdown.

Local `cfg(test)` variants only serve unit tests, which do not prove the production path (`cfg(not(test))`). `tests/archive_io_pipeline.rs` and `tests/linux_archive_workers.rs` link the library without `cfg(test)` and exercise the real Windows and Linux workers.

## Formats and mutations

ZIP, 7z, TAR, TAR.ZST, TAR.GZ, TAR.XZ, TAR.BZ2 and TAR.LZ4 can be created and modified through a workspace. TAR.LZMA and the other browsable formats stay read/extract only. `ArchiveFormat::from_path` checks compound TAR suffixes (`.tar.gz`, `.tgz`, `.tlz`…) before the last extension; the codecs live in `utils/archive_codec.rs`.

A single compressed file (`.gz`, `.xz`, `.bz2`, `.zst`, `.lz4`, `.lzma`) is browsed as an archive holding one member named after the file without its suffix. Its size is only known by decoding: listing decodes it under the listing budget, and the worker decodes it twice, once to declare size and hash, then to stream the content checked against that hash. The parent passes the member name in `Operation::Decode::stream_name`, because the worker never sees the path; the Linux listing sandbox names the document `source.gz`, so the parent restores the name. Extraction decodes into a hidden private folder of the destination, then renames the member beside the archive without replacing anything (`notes (1).txt` on conflict) and removes the folder. Decoders accept concatenated members, refuse an empty or foreign prefix, and cap LZMA-alone dictionaries at 1 GiB. These files are never created or modified. ISO and IMG ISO9660 are virtual and read-only; on Windows, unrecognized images and VHD/VHDX go through native mounting via `Platform`.

Windows mounting uses `virtdisk.dll` directly (`OpenVirtualDisk`, `AttachVirtualDisk`, `DetachVirtualDisk`). Image tracking queries disk/CD-ROM devices with `GetStorageDependencyInformation` and matches drive letters by device number and type. It also detects images mounted outside Oxyde, without PowerShell. These calls stay off the UI thread. ISO/IMG mount read-only, and mounts persist after the handle closes.

### Compression and Turbo on Windows

Turbo keeps the chosen quality level and raises parallelism, on top of process priority. The job's CPU limit stays active: Turbo does not promise 100% CPU.

7z creation/rebuild reserves a 2 GiB job, even without Turbo so it can be turned on midway. Other Decode/Encode paths keep 1 GiB. The global budget stays 2.5 GiB, with 512 MiB reserved for small navigation workers; another targeted archive read needing 1 GiB may wait up to 30 s, then be refused if its reservation stays unavailable.

| Format | Parallel work and limits |
| --- | --- |
| ZIP | Up to 16 files compressed at once depending on CPU, assembled in order without recompression. Admission estimated at 768 MiB for sources, results and encoders; a file over 256 MiB streams on one thread. A single file is not split into parallel DEFLATE. |
| 7z | Multithreaded LZMA2, large files included; thread count reduced by dictionary and a 1.5 GiB encoder estimate. Small files grouped in solid blocks of 128 MiB at most, separated by large files. Fast/normal/best dictionaries: 1/16/32 MiB. |
| TAR.ZST | Up to 8 Zstandard threads, 2 at best level, 8 MiB window. Thread count fixed at stream start; toggling Turbo later only changes priority. |
| TAR.XZ | Multithreaded xz (presets 1/6/8), threads reduced until the encoder estimate fits 512 MiB. Fixed at stream start like TAR.ZST. |
| TAR.GZ, TAR.BZ2, TAR.LZ4 | Single-threaded streams (gzip/bzip2 levels 1/6/9; LZ4 has no level, 4 MiB blocks with a content checksum). Turbo keeps priority only. |
| TAR | No compression: sequential traversal and writing, Turbo priority kept. |

Since sevenz-rust2 0.24 (lzma-rust2 0.22), the multithreaded LZMA2 encoder keeps at most one pending segment per thread, so memory no longer grows with file size and the 256 MiB single-thread limit only concerns ZIP. Directories are recorded immediately, even if earlier files still await their solid block, so internal 7z entry order may change. Admission estimates do not replace the job's hard memory limit. ZIP and 7z parallelism is reassessed at file/block boundaries; running tasks may finish computing after Turbo is turned off or during a pause, but effects stay blocked by the parent.

A control conversation stays active during I/O-free computation and finalization. Codecs and their threads stay in the isolated worker; the protocol and parent publication are unchanged.

A mutation rebuilds in the original's folder. The parent checks identity, size, date, BLAKE3 and provenance before replacing; an external change aborts publication. Links inside the archive are not followed. Pause and cancel stay active during reading and rebuilding.

For a group, prepare all archives then publish while keeping originals until complete success; restore the group on failure. **This is neither atomic visibility nor a durable transaction after a crash.** Keep and report unrestorable backups.

A move removes only sources actually copied; skipped conflicts stay. Archive mutations add no filesystem Undo/Redo. An external open uses a temporary copy, with no automatic reintegration of edits.

## Encrypted archives

ZIP (ZipCrypto, AES) and 7z encrypted with a password are read, browsed, opened and extracted; creating and rebuilding them is not supported yet. `utils/archive_password.rs` holds the passwords:

- The parent remembers each password in memory for its archive path until Oxyde exits, never in configuration, workspace or logs. `ArchiveSecret` hides its value from `Debug` and clears its memory on drop.
- A listing worker receives a `u16` length and the UTF-8 password (1024 bytes at most, empty without one) on its standard input right after the start gate, never in its arguments. Decode receives it in `Operation::Decode::password`, accepted for ZIP and 7z only.
- A refusal is the typed `ArchivePasswordFailure` (`Required`, `Wrong`), carried by `FailureKind::PasswordRequired`/`PasswordWrong` or by the listing's error text. 7z with encrypted headers refuses even its listing; ZIP and 7z with clear headers list without a password. A ZipCrypto password passing its one-byte check but failing the CRC counts as wrong.
- The interface (`ui/app/archive_password.rs`) asks for the password when a listing, an opening or an extraction (or a copy out of an archive) is refused, then runs the refused action again. A wrong password is forgotten and asked again; cancelling leaves the translated error.

Known gap: a file opened from an encrypted archive stays decrypted in the open cache under the cache's usual rules (up to 7 days on Linux), not until exit.

## Guarantees to preserve

- Exclusive publication, destination checks and partial-effect recovery in the parent.
- No adoption or purge of a foreign object or old cache without sufficient proof.
- The recoverable Windows cache uses authenticated persistent proofs; lost/corrupted keys and incomplete states stay protected.
- The Linux cache evicts only with a write lease on each file, proof no reader holds it open: see [Linux cache](linux-open-cache.md).
- Independent fidelity of content/metadata is only established for the uncompressed TAR scope of R9. Other formats and worker results have no such general guarantee.
- Linux runs the same codecs and protocol in [bubblewrap-confined workers](linux-previews.md#archives); Windows confinement and its validations do not apply there.

Read the [R1–R9 limits](../security/status.md) before extending a guarantee.
