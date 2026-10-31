# Previews, thumbnails and icons

Hover previews stay in memory; grid thumbnails have a separate disk cache. Loading, file validation and decoding stay on workers. Linux specifics: [Linux previews](linux-previews.md).

[Office search](search.md#office-content-search) uses its own LPAC mode of `oxyde-image-worker`, a separate admission and a text response bounded to 8 MiB. It does not reuse hover previews' truncated output.

Starting a file ghost immediately closes the hover preview, pending preparation included. Hovers stay disabled during drag and drop, even when no drop target accepts.

## Hover and presentation

`components/tooltip/hover.rs` shares a session per window, panel and tab/folder. Initial delay: 750 ms; tolerance between items: 300 ms. Leaving, clicking, scrolling, navigating or a menu closes the session. Cancelable timers and revisions discard stale results.

With no hovered item, the bubble hides immediately and frees its preview. The 300 ms tolerance only keeps the session: a new item in the same region skips the initial delay. Moving directly between two items keeps the bubble shown.

On first hover, the view and its preview are prepared during the 750 ms, with nothing shown early. The cancelable task owns this view until publication; leaving the item frees it. 750 ms is a minimum: if the preview is still running, publication waits for its content (first frame for a video) or definitive error. Text and preview appear together, with no "Loading preview…" step. This extra wait does not apply to replacements in a session already visible.

A replacement with a new generation immediately shows the new metadata and the old preview in grayscale at 15% opacity, when a previous image exists. Opacity and grayscale apply at once at GPU rendering, with no animation, CPU conversion or pixel copy. This temporary preview's dimensions are kept during the wait. The final preview replaces it after 150 ms of stable hover; a result ready in cache publishes immediately after source validation. If pixels are already in RAM, the previous bubble stays sharp during this asynchronous validation to avoid a dimming flash. This presence hint does not read the file, block on the cache lock or ever allow publishing unvalidated pixels. With no previous image, the previous bubble stays until replaced. Videos and tooltips without preview change immediately. A dimmed transition waits for content or definitive error, with no overlaid loading message. Outside dimmed transitions, "Loading preview…" appears only after a second.

Logical dimensions follow zoom: max height 12.5 rem for images/videos, 18.75 rem for PDF, max width 50 rem. Keep proportions and do not enlarge small sources. A ready preview sets the width; the name wraps within it.

The integrated fallback anchors the name/metadata block at the cursor (0.75 rem offset, adjustable 3.125 rem edge margin). The preview appears above without moving this mark or reserving empty room. `tooltip/position.rs` and `placement.rs` carry these calculations. Fixed tooltips keep their anchor; native transport: [windows](windows.md).

## Viewer

### Tab and selection

The title bar's eye button opens or closes the single `viewer://` tab, alone in its panel; its tooltip follows this state. Drops on tab bars refuse any merge both ways; split zones move the Viewer with its own panel. The session restores this tab without listing or watching its internal location.

`app/viewer` follows the selection of the last active explorer tab and shows one file: the keyboard cursor's (or anchor's) if selected, otherwise the first of the selection in list order. On an image or video, side arrows (or ← / →) go to the previous or next media of the same type in the source panel's list, which selects it alone and scrolls it into view; the Viewer then follows. With no selection, preview and player are freed. Large lists are prepared in the background; a source revision discards stale results.

Before a deletion, `viewer/deletion` blocks reopening the concerned paths until the operation ends. The Viewer frees its content and waits for committed loads and renders before handing the deletion to the operation engine. The PDF session explicitly closes its document in the background, even if render references remain. The worker frees the document before acknowledging the close, then exits; a bounded wait keeps forced stop as a fallback. Active renders keep their acknowledgment until their native reference is freed. An error or cancel allows showing the kept file again.

The panel never becomes the active panel: its internal commands act on it, and the explorer panel keeps selection, keyboard and toolbar. Only the detached window takes keyboard focus. Folders, virtual archive entries and unsupported types have no preview. Unviewable files get a Viewer-specific message and an Open button, through the existing Open action in the source panel.

### Content types

The Viewer opens the full file (`app/viewer/content`):

- **Image**: full resolution through an LPAC session specific to the Viewer (`image_session`, type 2: 1 GiB job, 30 s, side up to 16384 px and 256 MiB of pixels, outside the tooltip budget; started when the Viewer is created, reused at about 35 ms per image, freed as soon as the Viewer shows something else or closes). Shown without enlarging.
- **PDF**: all pages (`viewer/pdf`: a self-worker `pdf-session` keeps the document open, 1 GiB job and 15 s per page). Only pages near the view are rendered, requests out of view are dropped, six decoded pages at most. Page frames are measured up front, so the scrollbar keeps its size.
- **Text**: whole, up to 512 MiB, loaded in memory (`viewer/text`). Wraps at spaces, computed in the background for the column width of JetBrains Mono, wide characters counting double, four-column tabs. Continuations take the initial indent, capped at half the available width in narrow panels. Each row keeps its byte range and logical line; numbers on the left, not repeated on wrapped lines.
- **Syntax**: Syntect detects the language by name or extension, then by first line (shebang or mode line) only if no type was found, with no general detection on the body. `.txt` stays plain. Analysis is off the UI thread; its categories use the theme's semantic colors. Coloring is limited to embedded syntaxes; XAML uses the XML grammar (lexical, no WPF interpretation). `ui/file_types.rs` centralizes extensions, compound suffixes and Code file names for families, icons and previews, including Oxyde formats and Syntect 5.3's pack, without loading grammars on the list render path. Specialized families take priority (SVG in Images, MD in Documents), so a Code file does not necessarily have an embedded grammar. SCSS and PowerShell have extra grammars pinned in `vendor/syntaxes/README.md`, with Sass as a SCSS dependency; MIT notices ship in `licenses/syntaxes/`. Syntect `yaml-load` reads only embedded grammars, once, off the UI thread. PowerShell covers `.ps1`, `.psm1`, `.psd1`, in the Scripts family with Shell/Batch. Variables use the semantic accent color, also in Markdown blocks. Coloring limits: files up to 2 MiB, 200,000 ranges; stops before a line over 16 KiB or between lines after 200 ms of analysis, continuing as plain text. This delay is cooperative and bounds neither a running line analysis nor syntax initialization.
- **Office**: complete Office Open XML and OpenDocument (`office-document` in the Office worker: Word/Writer, each Excel/Calc sheet, each PowerPoint/Impress slide; `tag<TAB>text` lines). Sheet values show as stored, without formats or formulas. Old binary formats (doc, xls, ppt) keep the existing preview.
- Long contents are virtualized by `list` with the shared scrollbar. Text uses the `sidebar` background like its previews; other contents take active panels' background. Images use the whole frame with no margin. Text/code, PDF and Office/ODF lists have 1 rem top and bottom padding, part of the scroll; the scrollbar's range includes these margins when content scrolls, whereas GPUI's exposed maximum for blocks alone excludes them.

These paths are specific to the Viewer: tooltip previews keep their modes and limits.

### Markdown

`md`, `markdown`, `mdown`, `mkd`, `mkdn` use `pulldown-cmark` with native GPUI rendering: headings, inline styles, lists, quotes, tables and code blocks colored by Syntect. Rendering is on by default. The toolbar offers a Rendered/Code toggle for Markdown and a "Syntax highlighting" button for other text files with actual colored ranges; both independent choices persist in `AppConfig`. Markdown source stays uncolored; code blocks in rendered Markdown are always colored by language.

Images relative to the document decode on demand through the isolated preview path and its memory budget, with at most 32 requests per document. Missing or non-local images show alt text. No remote URL or absolute/UNC image path is loaded. Links show as colored text; raw HTML shows literally.

The source is read off the UI thread, up to 512 MiB, with a warning beyond. Then a single continuous Markdown analysis publishes the first block immediately, then batches of at most 32 blocks through a bounded queue. The Viewer appends them without resetting scroll; each batch is measured whole before showing, offscreen included, to give the scrollbar its full height without walking intermediate blocks. Rendering stays virtualized; measuring triggers no image decoding, which is reserved for painted blocks. Arriving images invalidate their block's measure while keeping the scroll anchor. Long blocks split into portions of at most 16 KiB, at UTF-8 boundaries, losing neither text nor inline styles. Code fences, lists and references keep their context across batches. Source and prepared blocks stay in memory: the source cap does not bound global RSS. Changing file or mode interrupts reading between 64 KiB chunks, then analysis between events and portions; a running parser or Syntect call is not preempted. Changing mode invalidates earlier tasks and frees their textures; images decoded after a change are not published.

### Toolbar and detaching

The toolbar at the top, at the explorer's navigation bar height and with its metrics (1.5 rem buttons, 1 rem icons, 0.25 rem spacing, 0.5/0.25 rem margins), holds the file name, display options and the detach button. The Viewer has no status bar. The integrated panel shows the name alone; the secondary window shows the full path, with a tooltip if truncated. Loading says "Loading file…", distinct from previews. Content fills the remaining room and the scrollbar touches the right edge.

The top right button opens a resizable secondary window owned by the main one; its position, size and maximized state are remembered. The tab and its panel keep their place, with a button in the panel to find the secondary window. Only this window renders preview and player while detached. Selection tracking stays active. Closing the secondary window frees its session and restores the preview in the kept panel. Closing the main window also closes detached Viewers. Detaching or returning resumes at the same position and state; lists and pages keep their scroll.

### Audio

The Viewer shows text tags recognized by Lofty and the embedded cover, front cover first. With no decodable cover, the music icon stays; a metadata error does not prevent playback. A centered card groups cover and title, then three columns Album / Track / Year. The playback and progress bar follow the card at the same width. Only title, track, album and year show; it all scrolls if height runs short. If the player fails, for example with no system codec, a message and the open button replace the bar. Values are limited to 4096 characters, 256 fields and 256 KiB of text.

On Windows, extraction and decoding go through `oxyde-image-worker` in LPAC with only the source handle inherited, 512 MiB and 15 s. The JSON response is bounded to 8 MiB; the parent validates BGRA dimensions and pixel length (512 × 512 at most) without redecoding. Embedded images over 16 MiB and those over 64 MiB of source pixels are skipped. Stale responses are discarded by generation and textures freed on content change. No external cover file or URL is loaded.

### Windows media player

Audio and video use system codecs via IMFMediaEngine (HWND mode) in the self-worker's `viewer-media` mode. One native player syncs image and sound and receives play/pause (or Space). The toolbar offers "Autoplay", on by default and persisted in `AppConfig`. Off, new media open paused; running playback and resume after detaching keep their state.

Opening the file outside Oxyde pauses its player without changing position, also in the detached Viewer or during loading. Native "Open with" and playback commands follow the same rule when identified by their Shell verb; the "Open with" submenu without a verb is recognized by its system label in all five languages. Resuming in Oxyde stays manual.

The worker sends 25-byte reports (state, position, duration in ms and video dimensions); the bar allows clicking or dragging to seek (`seek_id`/`seek_ms`). Opening settings only hides the native video surface; playback keeps its state, and a black frame at the video's computed dimensions shows the display is paused. End of playback stays stopped until a Play action.

Video and its bar form a centered block: the bar follows the image's bottom, in the panel and the secondary window. Native dimensions come from the worker; the block adapts proportions to available room in the current frame.

The worker owns a passive native video surface, owned by the Viewer window, with no global topmost. It stays enabled to receive the mouse, without taking activation or focus (`WS_EX_NOACTIVATE`, `MA_NOACTIVATE`). Left presses and releases are posted to its GPUI owner in physical client coordinates; GPUI counts clicks for fullscreen. Right clicks and the context menu are absorbed, with no native action. It follows the measured, bounded content rectangle in physical pixels, also when its window moves. This keeps GPUI's composition, above child HWNDs, from hiding the video. Audio alone shows no surface. Geometry changes wake the supervisor then the native player; periodic waits remain for reports, visibility and window moves. Playback never falls back to the parent on failure. This path is separate from muted hover videos and uses none of their admissions: two Viewer sessions at most, then a cancelable wait.

The native job stays outside LPAC, with a 1 GiB cap and the existing aggregate admission. Startup and missing IPC heartbeat are bounded to 15 s; this heartbeat does not prove codec progress. JSON commands are bounded to 512 bytes and responses to one-byte states. File change, closing and last owner cancel the player, remove its surface and reap its process. Old previews cannot publish in a new Viewer.

### Fullscreen and overlays

Double-clicking an image or video preview toggles fullscreen; Esc leaves. Hovering the media shows a `maximize-2` button at its top right, replaced by `minimize-2` in fullscreen, whose background is always black. Near the content's left and right edges, previous/next arrows appear if a neighboring media of the same type exists, in panel, secondary window and fullscreen. They use the back-to-top button's fade (`motion::presence`).

`app/viewer/media_overlay` draws them in the Viewer for images. For video, placed above GPUI's composition, they are transparent owned GPUI popups with no activation, like the fullscreen bar, created at first hover, kept across files (a still pointer on an arrow can chain jumps) and removed on window change. Like the bar, they are owned by the visible video surface (found by its class `OxydeMediaSurface`), so Windows keeps them above it whatever the reordering. GPUI creates its PopUps topmost; this status is removed, so they stay visible even when Oxyde is not in front, without covering other applications. A hidden HWND receives no `WM_PAINT` and their rendering places them, so a command to show whose popup is hidden is forcibly redrawn. Before the surface is destroyed they return to the GPUI window. The surface's class declares the arrow cursor, which replaces a splitter's hovered just before.

In fullscreen, video uses the largest rectangle keeping proportions on screen, reserving no height for controls. A floating bar overlays the bottom, in a transparent owned GPUI window with no activation, above the worker's native surface, with no global topmost. Mouse moves on the video go to the owner; the bar appears then hides after two seconds of inactivity, except during its hover or a progress seek. Fade and six-pixel slide use the shared short springs (entry about 110 ms, exit 85 ms). Leaving fullscreen, changing file and closing remove the controls window and its timer; keyboard focus stays in the Viewer.

The exit button appears at the slightest move and fades after two seconds still, except under the pointer. A click anywhere in an arrow's side band jumps to the neighboring media, like photo library viewers; these clicks never toggle fullscreen. Hover forwarded by the surface does not signal leaving the window: the real pointer position is reread every 100 ms, and controls stay visible while it remains in their zone. A popup's hover only restarts this tracking, since its leave event may be missing.

Reused tooltips and hover previews go to the top of their owned window family at each appearance, above this surface. From the integrated panel, a temporary window hosts fullscreen and returns content to the panel on close. An existing secondary window keeps its role on leaving fullscreen.

The wheel on an image's pixels zooms around the pointer (1 to 32 times the fitted size); returning to 1 resets panning. Once enlarged, holding the left button pans. Changing file resets this framing. On Windows, native cursors use the official Lucide `search.svg` and `hand-grab.svg`: a magnifier briefly during zoom, a hand during drag. Ctrl+wheel keeps interface zoom.

## Caches and lifetime

Coloring grammars load at the first analysis request, off the UI thread; opening the Viewer on images does not preload them. Opt-in pixel and cache measures: [memory diagnostics](../diagnostics/memory.md).

Hover caches keep only reduced pixels, never file bytes or the source raster: 4 MiB at most for a still image at 1024 × 1024, 32 MiB of frames for an animation. Decoding may temporarily load the whole source in the worker (64 MiB encoded, 128 MiB of source pixels for images); this peak and decoder allocations are separate from the cache. RGBA buffers move to BGRA conversion with no extra copy.

`tooltip/pixels` tracks the image each preview entity shows. A replacement (video frames included), an error or freeing the last display removes old textures from the atlas through a deferred action; pixels shared with another visible preview and CPU caches stay available. Hiding the native window also empties its two GPUI frames without waiting for the next hover: a hidden HWND gets no `WM_PAINT` and could keep old entities, pixels and video cancel owner. The periodic check also closes a hover session gone invalid, even with no pointer leave event.

Replacing or closing Viewer content removes its images from the GPUI atlas through a deferred action. During an interaction GPUI temporarily removes the current window from its list, so an immediate `drop_image(image, None)` may miss it. The deferred action waits for its return and also covers detached windows. This does not guarantee an immediate drop in Private Bytes: frames, allocator and graphics driver may keep resources.

The same deferred removal applies to PDF pages evicted during scrolling. Pending requests for an evicted page or closed PDF are canceled before decoding; a native render already started may finish. Each response must still own its request: an old canceled request does not remove a new one for the same page number.

| Use | Limit and policy |
| --- | --- |
| Image pixels | RAM LRU: 32 entries / 64 MiB; all frames count |
| PDF pixels | Separate RAM LRU: 32 entries / 64 MiB |
| Hover decoding | Fixed 1024 px envelope; Gaussian for large images, no intermediate JPEG; native PDF at 1024 px |
| Grid thumbnail | One frame, 128 px, Triangle; opportunistic reuse of RAM pixels, no preview loaded on miss |
| Persisted thumbnail | JPEG quality 75 if opaque, PNG if alpha; 24 KiB per file at most |
| Windows storage | `oxyde/thumbnails` under the user cache, or portable cache; quota 256 MiB / 4096 entries |

Hover neither reads nor writes the disk cache. A thumbnail does not fill the preview cache. Outputs too heavy stay displayable without persisting; small images are not enlarged. Old preview caches are ignored until the Clear cache action.

Keys use path, type, size and date; workers check the source before reuse and around decoding. Errors are not cached. Clearing the cache invalidates running generations. The size/date signature is no proof of identity or content.

`ui/preview_disk` and grid assets tie each thumbnail to the listed revision (`FileStamp`). Cards share a lease; leaving the viewport, changing card/view or closing frees it. The last owner removes the asset and its pixels from the GPUI atlas through `App::drop_image`. Frame references may survive temporarily: this contract does not bound global RSS/GPU. No metadata call in rendering.

### Audio covers

Audio files in the grid use their embedded cover (front first), extracted through the same `Platform::audio_metadata` path as the Viewer. Validated pixels are reduced to 128 px with Triangle then kept in the thumbnail cache under a separate audio key. With no cover or an unsupported format, the card keeps its icon. Recognition uses the extension or native audio type. Files recognized by extension also show their cover on hover, with no disk cache access.

Covers have two independent RAM quotas: 256 grid thumbnails at 128 px (16 MiB at most) and 32 hover previews at 512 px (32 MiB at most). Disk cache reads also feed this RAM cache. File revisions are checked before reuse; file change and clearing the cache invalidate entries. Audio hover uses the images' transition: a RAM hint with no I/O avoids dimming the old cover during validation, and only a cache hit validated by the worker publishes without the replacement delay.

A BLAKE3 hash of dimensions and pixels shares identical images across tracks and identical thumbnails across cards, not relying on album name. On disk, a 40-byte reference per track revision points to a shared thumbnail named by its hash, checked on read. References and thumbnails stay in the existing quota; removing a shared thumbnail means a cache miss, regenerated at next load.

Hover and grid covers use a dedicated, reusable LPAC audio session of `oxyde-image-worker`, separate from image sessions and the Viewer's full extraction. Requests pass only the source handle and expected resolution; responses hold binary BGRA pixels, with no tags or JSON. The grid reduces directly to 128 px in the worker, hover to 512 px. The job is capped at 256 MiB, with 15 s per request, 15 s idle and recycling after 32 requests or 60 s. A valid file without a cover causes no restart; a parse/decode error, crash or timeout retires the session, with no local fallback. The worker keeps only the last decoded cover, by hash of the embedded bytes and resolution (1 MiB at most). A large audio source may be walked by seek rather than fully loaded.

Audio hover and Viewer tags each have a dedicated single-request admission, so they do not wait for the whole thumbnail queue. These admissions also apply to all hover and to the Viewer's initial loads (images, text, Markdown, Office and PDF opening). Markdown's relative images keep their background admission. Viewer tags keep their one-shot worker and full response, but do not take the global fixed-preview lock. Aggregate budgets stay shared: this split does not guarantee immediate availability under memory pressure or when all process slots are taken.

### PDF

Grid PDF thumbnails and PDF previews share a dedicated native session of `oxyde.exe` (`pdf-previews`), separate from the Viewer's and Shell icons'. A document is opened, rendered then freed per request; the process is reused. Native rendering is limited directly to 128 px for the grid and 1024 px for hover. The transport shares the bounded protocol of native path sessions: a command, a length and a UTF-8 path; responses identified and bounded, one BGRA frame at the expected resolution. The parent holds the source for reading during the request. Caps: 512 MiB, 15 s per request, 15 s idle, 32 requests, 60 s of session and 4095 cumulative command bytes. Paths exceeding this transport keep a one-shot worker at the same output resolution. Error, crash or timeout retires the process, with no PDF decoding in the parent. This session stays outside LPAC and may keep PDF engine state between documents: no per-document isolation. Image, audio and PDF sessions no longer wait for the Office one-shot lock. Requests already committed in a session stay sequential and are not preempted by a hover request.

## Windows sessions

- Images/SVG: `windows_preview_worker/image_session.rs`, sequential LPAC worker on demand. Stops after 15 s idle, recycled after 32 requests or 60 s; active timeout 15 s, 512 MiB memory, shared admission.
- The parent holds the source and duplicates only its read handle. Commands and responses are bounded, identified and validated; error/crash/timeout retires the session, with no local fallback or automatic retry of the faulty file.
- Reuse potentially keeps a compromised decoder's state between a session's images. Recycling narrows this window without restoring per-document isolation.
- Shell icons: `icon_session.rs`, separate native session outside LPAC, 256 MiB memory, stops after 15 s idle, 64 requests or an exhausted cumulative request budget. Response bounded to 1 MiB and 256 × 256; paths too long go through a one-shot worker.
- Icons have two loading slots, separate from the four fixed-preview slots. The Shell session stays sequential; cache access does not depend on the preview queue. Parent and worker each keep a pixel cache bounded to 32 MiB / 1000 entries. PNG serves persistence only: a disk read decodes once, a native extraction passes its pixels directly after dimension validation.
- The icon worker does not persist PNG. The parent puts validated pixels in RAM and offers them to `windows_icon_persistence`: one thread, at most 32 requests / 4 MiB of pixels pending, plus the active image (256 KiB at most). Admission waits for no write; saturation or a publication failure leaves the icon available with no persistence guarantee. PNG encoding, controlled handles, quota, sync and rename stay off the pixel availability path. RAM/disk cache hits are not republished.
- Clearing the cache suspends admissions, invalidates revisions and removes pending requests. Background cleanup waits for a committed publication and keeps the suspension while deleting files. A response started before clearing cannot refill the RAM cache or schedule an old write. On exit, pending requests are dropped; the active publication has at most two seconds before the final native exit, so an immediate stop may lose the last persisted icons, which are regenerable.
- Ordinary icons share their source by extension/size; generic folders too. Executables, shortcuts, drives and special paths keep their exceptions. Arbitrary `desktop.ini` customizations are not detected by this heuristic.
- PDF, video and Office text keep separate paths. Video uses a bounded queue, last-owner cancel and the PTS clock; only streams ending normally after a frame loop.

[Shutdown](../diagnostics/shutdown.md) stops the image, icon and PDF preview supervisors before the final native exit. The [security limits](../security/status.md) apply. Pixel ownership tests do not measure a real GPUI session.
