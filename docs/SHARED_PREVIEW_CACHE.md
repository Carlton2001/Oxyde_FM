# Preview and thumbnail caches - 14 October 2026

Tooltip image/PDF previews are RAM-only: each LRU retains at most 32 entries / 64 MiB of decoded pixels. Animated image accounting includes all frames. Asset lifetimes, cancellation, source signatures and immediate handover for cache hits remain unchanged. Restarting the application requires regenerating previews.

Tooltip decoding does not read or write thumbnail/preview disk files. Large raster images are reduced once with Gaussian to the existing 1024 px envelope; no intermediate 768 px resize or JPEG encoding is performed. GPUI still scales these pixels to logical display dimensions. Exact DPI-dependent decoding is not implemented; the 1024 px envelope covers typical DPI and keeps worker budgets unchanged. PDFs retain the existing isolated Windows renderer.

Disk thumbnails are generated only by grid cards when `show_grid_thumbnails` is enabled. They reuse valid RAM preview pixels opportunistically without loading a preview on a miss. Otherwise the isolated image worker decodes directly to 128 px with Triangle and one animation frame; PDF thumbnails use native rendering then downsize. Small source images are not enlarged. A thumbnail never seeds a tooltip preview.

## Disk storage

- Existing `dirs::cache_dir()/oxyde/thumbnails` directory and versioned source-signature keys.
- Opaque pixels: JPEG quality 75; alpha: PNG; maximum 24 KiB encoded per entry. Oversized entries are displayed without persistence.
- Native atomic publication, bounded reads, corruption fallback and per-key generation locks remain in place.
- Windows global quota remains 256 MiB / 4096 entries.
- Old preview files are deliberately ignored and left for the user's Clear cache action. No automatic deletion occurs.

## Grid asset lifetime — A03, 16 October 2026

GPUI grid assets are keyed by the listed file revision as well as the path. Ordinary filesystem listings and searches capture a full-resolution `FileStamp` (length and modification time) on their workers; display dates remain in milliseconds. File links use their target's stamp. Entries without filesystem metadata keep their listed size/date as a fallback. The stamp is a cache revision, not an object-identity or content-hash proof; changes preserving both length and the exact filesystem timestamp are outside this signature's guarantees.

Each visible thumbnail holds a lease in GPUI element state. Identical source revisions share an asset across panes. Rebinding a card releases its previous revision; leaving the virtualized viewport, switching tab/view, disabling thumbnails or closing a pane releases the lease. The final lease removes the GPUI asset, including pending loads and cached failures. Loaded images are tracked with weak references; final release and explicit eviction also call `App::drop_image`, removing their sprite-atlas entries across windows (including the current window during a card rebind). Clear cache evicts every registered grid source and advances a generation, so a previous lease cannot evict a replacement load. Existing element IDs, geometry, fallback icons and tooltip behavior are preserved.

Retained grid pixels are proportional to the visible working set and its outgoing frame, rather than to all paths visited. Each output is one frame of at most 128 × 128 RGBA pixels (64 KiB). Frame/render references can outlive cache removal until GPUI releases them. This is separate from the tooltip pixel budget, and is not a fixed global RSS/GPU cap. Blocking work that already started retains the existing admission permit until it finishes. GPUI's pinned asset-cache implementation drops removed pending publications; workers additionally compare filesystem stamps before and after reading/decoding so a changed source is rejected. Rendering performs no filesystem metadata queries.

Readable sources without modification times retain the previous decode fallback; their length is still compared before and after loading. A previously known stamp becoming unavailable is rejected, as is a missing or deleted source.

## Validation

Persistence fixtures cover alpha/channel preservation, independent reads, invalidation, corrupt data, failed writes, byte budget, small JPEG size and generation locks. Decoder fixtures check direct Triangle thumbnail output and Gaussian preview pixels without JPEG roundtrips. Existing image/PDF LRU tests cover reuse, source changes, eviction, byte accounting and stale generation rejection.

A03 regressions cover submillisecond listing stamps, matching search stamps, shared card ownership, pending/error release, replacement generations after clearing, a simulated 10,000-path scroll, the correct final-owner image being retained just long enough for atlas removal, changed-source rejection before and during decoding, deletion/recreation and the 128-pixel output envelope. Pixel lifetime tests use real `RenderImage` values and weak references; they exercise the ownership policy without an interactive GPUI window. The GPUI adapter is compiled against the pinned implementation, whose `drop_image` and current-window contract were checked. These tests do not measure process RSS or GPU texture retention in a live application.

Visual quality and timings in a real Oxyde session still require user testing. Linux compilation remains unverified on this Windows host.
