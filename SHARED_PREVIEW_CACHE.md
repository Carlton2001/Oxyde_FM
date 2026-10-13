# Compact preview cache - 14 October 2026

Grid thumbnails and tooltip previews now use separate disk renditions. Browsing a grid persists only a 128 px rendition, matching the former Tauri thumbnail size. A tooltip persists its own rendition, up to 768 px, only when requested. An existing large rendition can seed a small thumbnail; a small thumbnail never seeds a tooltip.

Opaque images and PDF renders use JPEG quality 75. Transparent images use PNG to preserve alpha. Both retain proportions; small tooltip sources are not enlarged. Resizing uses Triangle. Image decoding and native PDF rendering retain their existing isolated worker limits (1024 px intermediate); grid generation never persists this intermediate. Animations keep their original tooltip playback; only a first frame is persisted.

## Storage

- Existing `dirs::cache_dir()/oxyde/thumbnails` directory.
- Versioned BLAKE3 keys include absolute path, media type, rendition size, source length and modification time.
- Grid entries: maximum 24 KiB; tooltip entries: maximum 160 KiB. Oversized encoded results are displayed but not persisted.
- JPEG files use `<hash>.jpg`; transparent PNG files use `<hash>_large.png`. A ten-byte trailer preserves animation flags. Standard image readers still read the images.
- Native atomic publication and Windows cache quota remain in place (256 MiB, 4096 entries).
- Corrupt or stale entries are misses. Per-rendition locks prevent duplicate generation. Failure to write does not prevent display.
- Old version PNGs are ignored and deliberately left for the user's Clear cache action. No migration or automatic cleanup is performed.

## Validation

Temporary-fixture tests cover alpha/channel preservation, independent reads, animation flags, invalidation, corruption, write failure, separate rendition keys and locks, 128 px JPEG dimensions and encoded size, and 768 px JPEG output. Visual quality and the total size of the user's actual folder still require an application trial; synthetic size checks do not predict every photograph's compressed size.
