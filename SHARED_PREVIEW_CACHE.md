# Preview and thumbnail caches - 14 October 2026

Tooltip image/PDF previews are RAM-only: each LRU retains at most 32 entries / 64 MiB of decoded pixels. Animated image accounting includes all frames. Asset lifetimes, cancellation, source signatures and immediate handover for cache hits remain unchanged. Restarting the application requires regenerating previews.

Tooltip decoding does not read or write thumbnail/preview disk files. Large raster images are reduced once with Lanczos3 to the existing 1024 px envelope; no intermediate 768 px resize or JPEG encoding is performed. GPUI still scales these pixels to logical display dimensions. Exact DPI-dependent decoding is not implemented; the 1024 px envelope covers typical DPI and keeps worker budgets unchanged. PDFs retain the existing isolated Windows renderer.

Disk thumbnails are generated only by grid cards when `show_grid_thumbnails` is enabled. They reuse valid RAM preview pixels opportunistically without loading a preview on a miss. Otherwise the isolated image worker decodes directly to 128 px with Triangle and one animation frame; PDF thumbnails use native rendering then downsize. Small source images are not enlarged. A thumbnail never seeds a tooltip preview.

## Disk storage

- Existing `dirs::cache_dir()/oxyde/thumbnails` directory and versioned source-signature keys.
- Opaque pixels: JPEG quality 75; alpha: PNG; maximum 24 KiB encoded per entry. Oversized entries are displayed without persistence.
- Native atomic publication, bounded reads, corruption fallback and per-key generation locks remain in place.
- Windows global quota remains 256 MiB / 4096 entries.
- Old preview files are deliberately ignored and left for the user's Clear cache action. No automatic deletion occurs.

## Validation

Persistence fixtures cover alpha/channel preservation, independent reads, invalidation, corrupt data, failed writes, byte budget, small JPEG size and generation locks. Decoder fixtures check direct Triangle thumbnail output and Lanczos preview pixels without JPEG roundtrips. Existing image/PDF LRU tests cover reuse, source changes, eviction, byte accounting and stale generation rejection.

Visual quality and timings in a real Oxyde session still require user testing. Linux compilation remains unverified on this Windows host.
