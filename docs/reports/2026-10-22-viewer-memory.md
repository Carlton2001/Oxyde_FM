# Viewer memory: 22 October 2026

Read-only observation of the instrumented Windows process (PID 14120), kept open
by the user after two identical image browsing passes. Tooltip previews disabled.

## Measurements

The current session starts at 16:40:18 UTC (18:40:18 Europe/Paris).
The idle sample at 16:40:49 reports 114.80 MiB Private Bytes; the idle sample at
16:43:28 reports 248.19 MiB. The increase is about 133.39 MiB.

After clearing the Viewer, all of these report zero:

- Viewer live, pending and retired pixel bytes;
- untracked registrations (coverage did not saturate or miss a registration);
- shared preview pixel budget;
- tooltip image/PDF caches, audio-cover cache and parent native icon cache.

The syntax initializer was not observed in this session. Its independent cold
initialization measurement was 24.3 MiB; removing its eager warmup does not account
for the remaining increase.

## Native allocations

Read-only VirtualQueryEx and ReadProcessMemory inspection found two private,
committed allocations, each 64,028,672 bytes (61.0625 MiB):

| Allocation base | Pointer to pixel allocation | Resource descriptor |
| --- | --- | --- |
| 0x22b8e1a0000 | 0x22b819d0b88 | 0x22b81fc9cf0 |
| 0x22b92000000 | 0x22b819d5658 | 0x22b8191d1f0 |

The sampled allocation contents have a repeated opaque alpha lane consistent
with 4-byte pixels. Both allocation descriptors point to resource descriptors
whose first pointer resolves to `igdgmm64.dll + 0x3a7368`. Each resource descriptor
contains `0x3d09000` (64,000,000 bytes) at offset 0x30, matching the decoded payload
size of one 4000 x 4000 image. Together, the two payloads account for 122.07 MiB;
the committed regions including rounding account for 122.125 MiB.

This connects the two large buffers to Intel graphics memory-manager resource
descriptors. It explains the size of most of the observed increase, while the
Viewer's tracked RenderImage objects and measured caches are no longer live.
No image content was printed or exported. Inspection opened the process for
query/read access only; it did not suspend threads, write memory or force a trim.

## Limits

This identifies a native graphics ownership path. It does not prove whether
GPUI/D3D11 still retains live textures, an asynchronous graphics operation holds
references, or the Intel driver keeps allocations for reuse. In particular, it
is not proof that the driver is faulty, nor that the retained memory is necessary.
About 11 MiB of the baseline-to-idle increase is not attributed by these two
buffers; the baseline and final snapshots also include other process activity.

Toolhelp heap walking did not cover the owner blocks and is not used as evidence
for allocated-versus-free heap totals. No new source-code fix follows from this
native inspection alone. A further graphics investigation must measure texture
ownership/bindings in GPUI/DirectX before changing renderer cleanup behavior.

The temporary inspection scripts and numeric outputs are under
`target/memory-diagnostics-*` and are not versioned.

## Controlled DirectX and GPUI probes

The subsequent isolated D3D11 probe uses the same DEFAULT texture upload and
shader-resource bindings as GPUI, waits for GPU completion, clears both VS/PS
bindings, flushes, and waits 30 seconds. It compares UpdateSubresource with
CreateTexture2D initial data. All source pixels and explicit texture/view COM
references are dropped before the idle measurement. A small GPU readback also
checks that color and alpha bytes survive the initial-data path.

In a real GPUI window, eight successive 4000 x 4000 RenderImages were displayed,
retired through Viewer::replace_content, then cleared for 30 seconds. Logs show
all eight atlas textures reaching zero keys and being destroyed in both runs.
The CPU weak-reference checks pass in both runs.

| GPUI atlas upload | Private Bytes after clearing and 30 s |
| --- | ---: |
| New texture then UpdateSubresource | 261,808,128 (249.68 MiB) |
| CreateTexture2D with initial pixels | 133,226,496 (127.05 MiB) |

Difference: 128,581,632 bytes, approximately 122.6 MiB. No working-set trim or
forced graphics cleanup was used by the GPUI probe. This reproduces the observed
remainder even when GPUI retires its texture objects, and avoids most of it by
changing the first-upload path. It supports identifying retained native upload
storage, rather than a surviving Viewer RenderImage, as the principal cause on
this adapter. It does not establish memory behavior on other GPU drivers.

The final source patch vendors only gpui_windows from the exact existing Zed
revision; package versions/features and all other GPUI sources remain pinned.
New full-size atlases get initial data at creation. Existing atlas subregions
retain UpdateSubresource. Geometry, byte length and tile origin checks prevent
using partial source pixels as a full texture. See
[patch provenance](../../vendor/gpui_windows/README.oxyde.md).

The initial-data API is documented by Microsoft:
https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture2d

## Final integrated validation

With the permanent vendored backend selected by Cargo.toml, the real GPUI probe
again passed and measured 133,558,272 Private Bytes after clearing/30 s
(127.37 MiB); all eight full-size textures were created with initial data and
retired. The 393 ordinary UI tests passed (9 native/manual tests ignored), the
two pure upload-safety tests passed, and the D3D11 initial-data diagnostic passed
its GPU color/alpha readback on Intel(R) UHD Graphics, vendor 0x8086.

The three auxiliary workers were rebuilt. As the user's existing oxyde.exe was
still open, a separate application binary was produced as
`target/debug/oxyde-memory-fixed.exe` using `cargo rustc --bin oxyde -- -o ...`.
That special output command emits two rustc output-selection warnings; the
normal all-target cargo check is warning-free. The application itself was not
interrupted or replaced while running. The new executable must be launched in
a fresh process to compare its upload behavior; existing driver caches inside
the old process are unaffected by a code rebuild.
