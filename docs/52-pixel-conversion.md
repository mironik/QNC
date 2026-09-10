# Public pixel conversion

Scope recorded before implementation, 2026-09-08.

The missing edge between native decoder bytes (docs/48) and RGBA8 sRGB output
(docs/51) is a pixel/color converter, not a player, monitor or second decoder.
`qnc-pixel-convert` has no source/DB/network access, process, playback clock or
application dependency. Its only QNC dependency is the public media metadata
data contract. It transforms caller-owned frame bytes using an explicit spec
derived from the already saved video metadata. No probe or guessed color.

First supported path: progressive planar YUV420/422/444, 8-bit and 10-bit LE,
known BT.709 primaries/matrix, known limited/full range, BT.709 or sRGB transfer.
Output is opaque RGBA8 sRGB at unchanged raster dimensions. Native source data
and DB evidence are never changed. The 10-bit path retains ten bits through
matrix conversion before explicit final 8-bit display quantization.
HDR, other primaries/matrices, interlace and other storage formats are rejected,
not silently treated as Rec.709. No gamut mapping, rotation, scaling or deinterlace.
SAR/rotation/PTS remain the caller's original metadata, not converter state.

The v4 reference is `qnc-player-workstation-monitor/src/yuv420.wgsl` and
`src/lib.rs`: Rec.709 limited-range conversion and inverse transfer are coupled
to monitor callbacks. That active code is not copied. The new library handles
matrix conversion with pinned [yuv](https://github.com/awxkee/yuvutils-rs), which
provides runtime SIMD selection for x86/ARM without per-application kernels.
The initial upsampling policy is explicit block replication, not chroma-phase
resiting. This is not a broadcast-reference chroma reconstruction claim.

Transfer lookup tables are prepared once from the
[BT.709 transfer definition](https://www.itu.int/rec/R-REC-BT.709) and
[sRGB conversion definition](https://www.w3.org/TR/css-color-4/#color-conversion-code).
No per-pixel power function or worker/process spawn at Play. Conversion belongs
to preparation/refill, not the ready Play command. Memory is bounded before
allocation; output buffers are supplied by the caller and reused.

Same serialized conversion spec and explicit bytes apply behind Local/LAN/
Intranet receivers. This library does not implement a remote receiver or claim
that the out-of-process player is complete. Application UI remains unchanged.

Verification: metadata/size/bit-depth negatives, known pixel values, range and
transfer differences, source preservation, repeated conversion without scratch
growth, actual saved original/proxy decode through the public modules, GPU
submission and native diagnostic display. No new media or DB writes.

## Verification 2026-09-08

Windows x86_64, Intel Iris Xe / Vulkan, driver 101.7080:

- 120 focused tests passed across pixel-convert, video-output, audio-output,
  broadcast-player, player-contract and conformance. This includes all 12
  converter tests; the two opt-in device tests are excluded from that count.
- Video device test separately passed: actual GPU pixel readback, two images,
  retained-frame reuse, token isolation, resize and device-loss handling.
  Submit API measured 0.882 ms, GPU completion observed 7.168 ms in that run;
  neither measurement is physical display scanout.
- Conformance passed, including the converter's transitive dependency boundary.
  Scoped all-target clippy with warnings denied and scoped formatting passed.
- Workspace-wide formatting is NOT clean: an existing assertion layout in
  `crates/qnc-source-reader/src/tests.rs` is reported. It was not changed as
  part of the pixel conversion step.
- SIMD fixed-point rounding permits a one-code tolerance for the full-range
  neutral midpoint. Limited-range black/white and alpha remain exact in tests
  for all six supported layouts. This is not a bit-exact reference-matrix claim.

The real-card diagnostic reads the active project's saved settings and clip
`clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b` (Mironik 1522) through
`qnc-work-settings` and `qnc-player-input`. Saved playback policy selects the
proxy; `--original` is an explicit diagnostic override only, never a DB change
or automatic fallback. Both representations are saved as progressive BT.709
limited-range video, 1920x1080.

| 32-frame release diagnostic | Proxy YUV420p8 | Original YUV422p10le |
| --- | ---: | ---: |
| Conversion minimum (ms) | 3.361 | 13.674 |
| Conversion median (ms) | 4.581 | 16.094 |
| Conversion maximum (ms) | 11.485 | 49.731 |
| Diagnostic elapsed (ms) | 683 | 1556 |

These resumed live runs are spot measurements, not sustained performance
guarantees. The original run overlapped regression work. An earlier quiet run
measured a 14.103 ms conversion median for the original. Diagnostic elapsed
includes decoding, hashing, cancellation and verification; it is not Play
latency or decoder-only throughput.

Each run checks media identity, saved/native format agreement, advancing source
PTS and timebase, and unchanged native bytes. SHA-256 of the source file before
and after matches; saved settings before and after match. There are no DB
writes, new probe calls or source writes. The decoder process is reaped before
the native display opens. Two different converted frames are retained in
bounded memory and prepared on the GPU before submission.

The previously interrupted Computer Use check was resumed. Proxy and original
both showed the expected conference footage in native diagnostic windows,
without a blank image or inverted raster. The diagnostics alternated two
prepared images for 60 seconds and exited normally. Surface-submit API ranges
were 0.176-1.132 ms (proxy) and 0.196-1.320 ms (original). Screenshots establish
visible still-image output, not sustained video cadence, calibrated color,
per-frame physical presentation timing or A/V synchronization.

Reproduce from the QNC root (owner directory stays private to the local test
adapter; all public media references come from the saved QNC URI):

```powershell
cargo run -p qnc-pixel-convert --release --example inspect_pixels -- C:\Users\miron\Projects\QNC G:\ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b --show
cargo run -p qnc-pixel-convert --release --example inspect_pixels -- C:\Users\miron\Projects\QNC G:\ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b --original --show
```

## Remaining Boundary

This closes the first SDR conversion/display edge, not Broadcast Player.
Next is the public out-of-process worker composition: saved input, bounded
decode/conversion/output preparation before Ready, then Play with no opening,
DB read or initial decode. Real A/V start/pause/seek and session isolation must
be verified there before any form/timeline integration. No application or
frozen Project code changed in this step.

Not verified here: sustained 50 fps original playback, physical output latency,
A/V sync, remote output delivery, Linux/macOS or ARM execution. HDR, interlace,
other color primaries/matrices and chroma-phase resiting remain unsupported
and are not silently substituted. Conversion timing spikes must be considered
when sizing bounded refill buffers; this diagnostic cannot certify instant Play.
