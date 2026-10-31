# Linux previews and decoders

Windows is unchanged.

## Principle: like Nautilus

Nautilus delegates thumbnails to external programs: `papers-thumbnailer` (poppler) for PDFs, `gst-video-thumbnailer` (GStreamer) for videos. A booby-trapped file can only bring down the thumbnailer. Oxyde uses the same libraries in a worker, its own binary run as a subprocess (`--oxyde-preview-worker-v1`), so decoding stays out of the interface process.

## Worker

- `platform/linux_library.rs`: `library!` macro shared by GIO, poppler and GStreamer, loading a system library on first use through `libloading`. Without it only its feature is missing; no `-devel` package is needed to compile.
- `platform/linux_worker.rs`: launches the worker with piped input and output, start byte `0xA7` as on Windows, time-bounded reads through `poll`, forced stop (`SIGKILL`) and reaping on release.
- Memory: the worker's private memory is capped by `RLIMIT_DATA`, playing the role of the Windows job's commit limit. Address space (`RLIMIT_AS`) would count mapped code: the debug binary weighs 1 GiB. `MALLOC_ARENA_MAX=2` stops glibc reserving 64 MiB per decoding thread.
- Dying with the parent: Oxyde holds the other end of the worker's stdin until it releases it. A worker thread waits for this pipe to close (`poll` without reading) and then kills itself. `PR_SET_PDEATHSIG` does not fit: the signal follows the thread that launched the worker, and a pool may retire it. The real parent is not visible either, since the sandbox has its own process namespace.

Three executables: `oxyde` for previews, `oxyde-archive-list-worker` and `oxyde-archive-io-worker` for archives, without GPUI. [The packages](../guides/linux-distribution.md) install them all, and a launch failure never triggers local decoding.

## Confinement (bubblewrap)

GNOME confines its thumbnailers this way, and glycin its image loaders. `platform/linux_sandbox.rs` launches each worker through `bwrap`:

- Namespaces: user (no nested namespace, `--disable-userns`), process, network, IPC, hostname and cgroup. New session: no terminal injection.
- Files: empty root, read-only `/usr` and its links (`/lib64`…), `/etc/ld.so.cache`, minimal `/proc` and `/dev`, empty `/tmp`. Oxyde's binary is mounted alone if outside `/usr`. The document is opened by Oxyde and mounted alone through its descriptor (`--ro-bind-fd`) at `/oxyde/source.<extension>`; the extension serves formats without a signature. The worker sees no other user file. Environment variables are cleared, then set one by one.
- By mode:
  - Pure codecs (archives, Office, audio tags and covers, raster thumbnails): nothing more.
  - PDF and images, whose SVGs use fonts: `/etc/fonts`, fonts, and the user's fontconfig configuration and cache, read-only.
  - Video hover: Oxyde's GStreamer registry, read-only.
  - Viewer player: the registry, plus PulseAudio sockets (`$XDG_RUNTIME_DIR/pulse/native`, served by PipeWire) and PipeWire, with `/etc/pulse`, `/etc/pipewire` and `/etc/machine-id`. No GPU access: decoding is software.
- Seccomp filter (`linux_sandbox/seccomp.rs`), compiled by libseccomp loaded at runtime, then installed by bubblewrap. Like Flatpak, a denylist returning `EPERM`: kernel administration, other processes' memory (`ptrace`, `process_vm_*`), mounts and namespaces, `bpf`, `io_uring`, `userfaultfd`, `perf_event_open`, keys, `TIOCSTI`/`TIOCLINUX`, `personality` changes. Any non-local socket (not `AF_UNIX`) returns `EAFNOSUPPORT`. Calls from another architecture kill the worker. glycin prefers an allowlist; it would break with GStreamer, plugin and glibc versions.
- GStreamer registry: a full plugin scan costs 2.5 s. Oxyde keeps its own registry in `<cache>/Oxyde/gstreamer-1.0/registry.bin`. Before the session's first media worker, a `media-registry` worker updates it: it opens no document and only this folder is writable for it. Other workers read it without being able to change it (`GST_REGISTRY_UPDATE=no`). A compromised worker can thus neither change the plugins later ones load nor touch other programs' registry. User plugins (`~/.local/share/gstreamer-1.0`) are not loaded.
- Without usable bubblewrap (missing, or user namespaces forbidden), the log says so once and the worker confines itself before starting (`linux_sandbox/landlock.rs`). Landlock limits files to the same accesses as bubblewrap, without `/proc` or `/tmp`, and since Linux 6.12 signals and abstract sockets to its own processes. The seccomp filter adds to this, refusing any socket except for the Viewer player. Environment is cleared as with bubblewrap. Still less strict: the player can reach any local socket by path, the session bus included, and before Linux 6.12 a worker can signal the user's other processes. Without Landlock only the filter remains; without libseccomp, confinement remains with no syscall filter. To try: `user.max_user_namespaces=0`. Ubuntu forbids these namespaces without an AppArmor profile: [the DEB installs one](../guides/linux-distribution.md#ubuntu-and-apparmor).
- Cost: 40 ms for a PDF thumbnail, 80 ms to the first frame of a 1080p video.

## Images, Office and audio

`platform/linux_preview.rs`: the parent launches one worker per request and validates its response; the worker reads the mounted document and returns only bounded pixels or text. No reusable session: a compromised decoder never sees the next document, at the cost of one launch per image.

- Modes: `image`, `image-thumbnail`, `image-viewer` (`ui/image_preview.rs`, fonts loaded only for SVG), `thumbnail`, `office`, `office-text`, `office-document`, `office-search`, `audio-metadata` (JSON), `audio-cover`, `audio-cover-thumbnail`.
- Caps: 512 MiB and 15 s; 1 GiB and 30 s for a Viewer image; 256 MiB for a cover. Responses bounded as on Windows (32 MiB, 256 MiB for the Viewer, 8 MiB for Office search). Canceling a search kills the worker.
- A named pipe or folder is refused without blocking on open.

## Archives

- Navigation and search index (`platform/linux_archive_list.rs`): the worker sees only the archive; the parent rebuilds each virtual path from validated metadata. 512 MiB, 120 s, 32 MiB response.
- Extraction, creation, rebuild and opening a file (`platform/linux_archive_io.rs`): the Windows protocol. The worker sees no file; it requests each block to read and hands over each block to write. The parent keeps sources and destinations (`OwnedTree`), checks names, types and sizes, and removes what it created on failure. 1 GiB, 2 GiB to create a 7z.
- Supervisor: a turn-based conversation over 1 MiB pipes. Non-blocking writes and 20 ms waits keep cancel and timeout active whatever the worker does. Only a confirmed pause suspends the timeout.
- Differences from Windows: no per-file `fsync` on extraction, like Nautilus (2000 small files: 5 s more on Btrfs); Turbo raises parallelism without changing priority; archive modification by local rebuild, without group publication (R8).

## PDF (poppler)

- `platform/linux_pdf.rs` (worker side): `poppler_document_new_from_fd`, then cairo rendering on white in premultiplied ARGB32, which is BGRA in memory, the format the interface sends to the GPU. Poppler's page size accounts for rotation. Sides match Windows: 128 px for the grid, 1024 px for hover, 2048 px for the Viewer. At most 100,000 pages listed.
- `platform/linux_pdf/session.rs` (parent side): one worker per thumbnail (`pdf-grid`, `pdf`). For the Viewer, one worker per document (`pdf-session`) keeps it open and renders a page per request. The protocol is Windows's: page sizes, then one frame per page, and `u32::MAX` to close, with a close acknowledgment.
- Caps: 1 GiB and 15 s per response; closing waits 2 s. Timeout, crash or unreadable stream retires the worker. A page error keeps the session open.
- A thumbnail costs one process launch, about 50–90 ms in debug. A reusable session like Windows's `pdf-previews` is not implemented. Password-protected documents show poppler's error.

## Video on hover (GStreamer)

- `platform/linux_video.rs` (worker side): `playbin3 flags=video`, which selects only the video stream, with no audio track or subtitles decoded or queued. `playbin` kept ignored audio queued: 770 MiB against 205 MiB for a 4K HEVC video. The sink is a clockless `appsink` after `videoconvert ! videoscale`, with BGRA caps, square pixels and 640 px per side at most. `videoscale` keeps proportions and does not enlarge small videos. Each frame carries its timestamp, which the interface uses for pacing. At end of stream, a 200 ms pause then a restart from READY loops the video, like reopening on Windows. End of stream and stream selection are read from the bus by exact type: GStreamer's extended types share bits with EOS and ERROR. A file with no video stream is refused once `playbin3` has chosen its streams. A stream ending without a frame is an error.
- `platform/linux_video/stream.rs` (parent side): one video worker at a time. Packets have Windows's format (length, seconds, nanoseconds, frame) and are checked: 2 MiB and 640 × 640 at most. Cancel is checked every 50 ms; with no frame for 15 s, the preview has expired.
- Codecs: GStreamer's installed ones. Fedora has no free software HEVC decoder: without RPM Fusion codecs, an H.265 video (iPhone, 4K) shows "Preview unavailable", like Totem and Nautilus's thumbnailer. Oxyde does not offer to install a codec.

## Viewer audio and video player

On Windows the worker shows video in a native surface above the window. Wayland does not let GPUI insert a foreign surface: on Linux the worker sends frames and GPUI paints them. Sound goes straight from the worker to the desktop's default output (PipeWire).

- `platform/linux_gstreamer.rs`: `Player` shared by hover and player (`playbin3`, `appsink` named `frames`, optional `size` caps filter, bus read by exact type, position, duration, seek to nearest keyframe).
- `platform/linux_media/worker.rs` (`viewer-media` mode): `playbin3` with image and sound, no subtitles. With `native-video`, `playbin3` leaves the decoded image intact until Oxyde's output, which converts and scales in one pass (`videoconvertscale`) and reads the source size on its input. Reports give this source size, not the scaled image's: rounding of the scaled image changed the ratio, hence the frame the interface computed, hence the requested size, and the video and its bar trembled. The sink follows the audio clock and drops a frame rather than delay sound if Oxyde falls behind (`max-buffers=1 drop=true`). Frames are scaled to the displayed size in physical pixels (3840 px at most, no enlarging), renegotiated 150 ms after a resize ends. In pause, the preroll image is sent once; an image is identified by buffer and timestamp, since pools reuse buffers. Reports every 100 ms in Windows's format (25 bytes), followed by the last applied seek. End of playback stays stopped; a seek restarts from there.
- `platform/linux_media.rs` (parent side): 21-byte commands (pause, seek, size), sent on each change and picked up every 20 ms. Frames are checked (3840 × 3840 at most, exact length) then placed in `ViewerMediaControl::frame`, with a `Notify`. A position before the last seek is not published, so the bar does not go back. With no report for 15 s, the session fails.
- Interface: the session task waits for a frame or 80 ms, creates the `RenderImage`, and frees the previous from the GPU (`drop_image`), also after the session ends. `VideoLayout` paints the image in the video frame. In fullscreen the floating bar is painted in the Viewer window, over the image, rather than a separate window Wayland cannot place; it disappears without a fade-out. Buttons overlaid on the video are also painted in the window, as for images. Their visibility follows GPUI mouse events: the global pointer position Windows polls every 100 ms does not exist on Wayland.
- Caps: 1 GiB of private memory for the worker. By default software decoders start one thread per core, each holding frames: on 32 cores, 760 MiB for a 4K HEVC video, then refused allocations, corrupted frames and a failed playback. `Player` therefore sets threads to 4 (`max-threads`, `n-threads`) on each element `playbin3` adds (`element-setup`), hover included.

## Limits

- Confinement is not Windows LPAC. The kernel stays exposed to allowed syscalls. The Viewer player talks to the sound server, which also accepts capture: the same access as a Flatpak application allowed to use PulseAudio. With no usable sandbox at all, workers have the user's rights.
- Without a sandbox, video mode requires a UTF-8 path, as on Windows.
- Player frames cross a pipe then a new GPU texture per frame: about 110 MiB/s at 720p 30 fps.

## Tests

`cargo test --offline --lib -- linux_sandbox linux_pdf linux_video linux_media linux_preview linux_archive` and `cargo test --offline --test linux_archive_workers`, the only one exercising the production archive path: unit tests compile the local codecs. Videos are encoded by `gst-launch-1.0` during the test (skipped without an encoder). Parent-side tests launch the real worker and first require `cargo build --offline --bins`.
