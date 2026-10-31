# Security: protections and limits

This summarizes shipped protections and open points. It is neither an expert assessment nor a certification.

## Windows trust model

The parent is trusted; a worker may be compromised by a codec. Kernel, launch and runtime are part of the trust base; a hostile administrator or compromised parent is out of contract.

Archive Decode/Encode, listing/index, images/SVG, Office text and audio tags/covers use capability-less LPAC workers, with no permissive local fallback. The parent keeps sources, checks and writes. Suspended launch attaches jobs before resuming; only explicitly allowed IPC and source handles are passed.

This includes Office content search: decompression and extraction stay in a dedicated worker, with a cumulative XML and response cap; a limit or failure is reported as unverified content. The parent receives only bounded text. Details: [Office search](../architecture/search.md#office-content-search).

ACL exceptions were accepted by the user: explicit access to the worker's exact SID or to `ALL RESTRICTED APPLICATION PACKAGES` may remain possible. Confinement is not independent of ACLs.

PDF thumbnails and previews reuse a native session outside LPAC, separate from the Viewer and icons. The parent holds the source during each render and validates bounded responses. Recycling after 32 requests/60 s shortens engine retention, without restoring per-document isolation or removing its ordinary rights.

PDF, video, the Viewer's audio/video player and native Shell/thumbnail/icon services stay outside LPAC. Their jobs and timeouts do not remove ordinary permissions. The Viewer uses system codecs in a separate self-worker, with no media decoding in the parent; its heartbeat watches the IPC response, not codec progress. Reusable image and audio cover sessions may keep a compromised decoder's state between requests; their sessions are distinct: [previews](../architecture/previews.md).

## Tracking R1–R9

| ID | Shipped protection | Remaining limit |
| --- | --- | --- |
| R1 permissions | LPAC for pure codecs, private runtime, explicit inheritance, native checks | Native services outside LPAC; production network and universal inheritance unproven. COM/registry permissions studied in tests are not enabled. |
| R2 deletions | Descendant receipts, partial effects, restoration and resume after conflict | Windows Trash kept by user choice; path races, no atomicity on the initial object, providers not validated. |
| R3 resources | Aggregate job 2.5 GiB / 4 processes / CPU 70%, shared admission, retained pixels 256 MiB / 256 images, pressure monitoring | No universal instantaneous cap on the parent, global RSS/GPU, external COM or physical disk. Cache unknown after a crash is not automatically included. |
| R4 identity/provenance | Rejection of null identity/foreign object, held handles, BLAKE3 check and `Zone.Identifier` | Native matrix on local fixed C: NTFS; other volumes, cloud/SMB, all ADS and immutable snapshots unproven. Another hard link can be renamed. |
| R5 Linux | [All decoders in workers](../architecture/linux-previews.md) confined by bubblewrap and seccomp, parent owns files; freedesktop trash without overwrite, transfers through the shared engine | [Confinement](../architecture/linux-previews.md#confinement-bubblewrap) less strict than LPAC, reduced to Landlock and seccomp if bubblewrap is unusable; no archive group publication; file operations do not freeze names, Linux only guarantees acting on the verified object ([contract](../architecture/linux-held-effects.md)). Windows guarantees do not carry over. |
| R6 cancellation | `Platform::watch_broker_io`, `CancelSynchronousIo` attempt, preparation and broker scopes | Cooperative: a driver may refuse. No parent thread is killed and no guard abandoned during I/O. |
| R7 recovery | Windows cache v2, owner v4, DPAPI/MAC authenticated journal, crash recovery; Linux cache v1 in the user's folder, eviction under a write lease ([contract](../architecture/linux-open-cache.md)) | v1 cache/v3 preparations without proof kept, outside the new storage quota. Lost key, foreign/modified objects and insufficient proof forbid adoption/global purge. |
| R8 archive groups | All archives prepared, originals held then group restored on failure | No atomic visibility or durable multi-file transaction after a crash; unrestorable backups kept and reported. |
| R9 fidelity | Independent check of content/inventory/defined metadata for uncompressed TAR | Other formats, listing/index and pixels/text have no general independent reference. Features kept; this risk is neither accepted nor closed by a user's silence. |

## Where to work

- Confinement/launch: `platform/windows_worker_sandbox.rs`, `windows_process_launch.rs`, `src/bin/`.
- Budgets/pressure: `platform/windows_worker_budget.rs`, `windows_resource_pressure.rs`, `ui/preview_budget.rs`.
- Deletions: `platform/windows_shell_delete*`, `systems/file_ops/delete.rs`, `execution.rs`.
- Archives/recovery: `systems/archive_io.rs`, `archive_encode.rs`, `archive_workspace/native_commit.rs`, `archive_open_cache/recovery.rs`, `utils/tar_fidelity.rs`.

Before resuming a point, pick its ID and define the expected proof. A refactor or visual check does not close a security limit. Any new validation states revision, target and scope. See also the [dependency decision](dependencies.md) and [shutdown contract](../diagnostics/shutdown.md).
