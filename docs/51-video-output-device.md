# Video output device

Scope recorded before implementation, 2026-09-08.

## Boundary

The next public component is `qnc-video-output`, not an Ingest renderer or a
second player. It owns a bounded GPU texture pool and an output surface. It has
no playback clock, decoder, source reader, DB, application or UI dependency.
The caller supplies the prepared frame identity and explicit output pixels.
All devices, pipelines, textures and bindings are prepared before presentation.
Pause retains them. A new generation invalidates old prepared-frame tokens.

The first output format is opaque RGBA8 sRGB with square display pixels.
Native YUV, 10-bit, HDR and interlaced decoder output must NOT be silently
treated as this format. A separate explicit pixel/color adapter is still
required before connecting native camera decode to this output.

The frame header is OS-neutral and serializable; private GPU tokens are not
wire handles. A future remote output receiver uses the same validated pixels
and header after transport. This component alone does not implement LAN or
Intranet output transport or an out-of-process player worker.

## Output evidence

Upload completion means GPU-ready, not presented. Submission to a surface means
scheduled for presentation. GPU completion means submitted GPU work completed,
not that a display scanout or photon was measured. None of these acknowledgments
may be relabeled as the player's physical `FramePresented` evidence.

The v4 reference is `qnc-player-monitor/src/lib.rs`: its
`accept_frame_buffer` changes `presented_frame` on buffer receipt. That behavior
is not copied as proof of display. No existing form/layout is changed. A native
diagnostic window is a test harness, not a replacement Monitor/Ingest UI.

## Verification Plan

- Reject invalid dimensions, payload sizes, generations and stale/cross-output
  tokens; bound queued textures and outstanding submissions.
- Render on the actual GPU and read back pixels, including orientation and
  letterboxing. Test release, reuse, resize and generation reset.
- Open a native diagnostic surface, show different prepared images and inspect
  screenshots. Measure submission latency separately from GPU completion.
- Run boundary checks, focused tests, formatting and clippy.
- Do not call this an end-to-end instant Play test. Player worker, A/V sync,
  native media conversion and network output remain separate work.

API semantics are checked against the pinned wgpu 24.0.5 source documentation
(`api/surface_texture.rs`, `api/queue.rs`), not inferred from UI receipt.

## Implementation

- `open` allocates the device, pipeline, bindings, render bundles and 1-8
  texture slots. Pipeline warmup renders offscreen, never an unsolicited frame
  to the visible surface. Upload completion includes that queued warmup.
- `prepare` accepts only exact tightly packed RGBA8 sRGB payloads matching the
  configured session/generation and dimensions. Increasing packet sequence is
  separate from source frame number. Source frame numbers can move backwards.
- `submit` uses a prepared token. There is no pixel upload, pipeline/device
  creation, source access or explicit GPU wait in this path. Wgpu still creates
  a submission command buffer, and the platform compositor can delay surface
  acquisition/presentation. This is not a zero-allocation or zero-latency claim.
- At most one output submission remains outstanding until `poll` consumes its
  GPU acknowledgment. No unbounded output event queue. Release/reset/resize
  reject outstanding work rather than discarding unconfirmed output.
- Source texture pool is capped by the caller's budget and 512 MiB; a target
  image is separately capped at 512 MiB. Driver/compositor allocations are not
  included in the source pool byte budget.
- Reset/resize invalidate tokens; release/reuse cannot revive old tokens.
  Tokens from a different output instance are rejected. Zero-size resize
  suspends output. Device loss is terminal and never silently reopens a device.
- Winit exists only in dev dependencies for the diagnostic. The public library
  does not depend on window/UI, player, decoder, DB, network or application
  crates. Conformance checks that dependency boundary.

## Verification 2026-09-08

Windows x86_64, Intel Iris Xe, Vulkan backend, driver 101.7080:

- Focused suite: 107 passed, two opt-in device tests excluded by default
  (audio and video). Packages: video-output, audio-output, broadcast-player,
  player-contract and conformance. Audio driver test was run in docs/50.
- Video GPU test explicitly run separately: PASS. Reads actual GPU pixels,
  checks all 64 pixels including top/bottom bars and exact 128-gray sRGB
  round-trip, renders a different second image, reuses a prepared image,
  verifies stale/cross-instance tokens, reset, resize, suspension and device loss.
- Final GPU run: prepared submit API 0.959 ms; GPU completion observed 2.152 ms.
  These are diagnostic measurements, not a sustained playback benchmark.
- Native diagnostic ran 45 seconds and exited normally: 15 surface submissions,
  preparation/upload queueing 532 ms; surface submit API 0.402-2.439 ms.
  GPU completion observation includes the diagnostic's 5 ms polling interval.
- Clippy with warnings denied, formatting and conformance passed.

The native screenshot attempt returned the Windows lock screen. No further UI
input was attempted. Visual screen confirmation is therefore OPEN, not passed;
the user must unlock Windows before that live check can be completed. The
diagnostic process has exited. No application forms, project data or source
card were modified by this step.

Reproduce the device checks from the QNC root:

```powershell
cargo test -p qnc-video-output --lib live_gpu_pixels_and_lifecycle -- --ignored --nocapture
cargo run -p qnc-video-output --example live_video
```

Not verified: physical scanout timing, real camera playback through this
adapter, sustained frame cadence, A/V sync, network output, Linux/macOS/ARM.
No end-to-end player readiness claim. After closing the visual check, next is
the explicit native-pixel/color conversion edge and worker composition with
the saved-input, decoder and audio output contracts. Form/timeline integration
must not precede the real out-of-process player runtime.

## Visual Recheck 2026-09-08

The earlier lock-screen block above is historical. With Windows unlocked, the
native color-pattern window was visually confirmed: four correctly oriented
color quadrants, marker and letterbox bars. That 45-second diagnostic exited
normally after 15 submissions; preparation/upload queueing measured 288.767 ms
and surface submission 0.400-2.076 ms.

The subsequent saved-pixel diagnostic is recorded in docs/52. Its first
Computer Use attempt was stopped by the user; after the user's continuation,
both a saved proxy and a saved 10-bit original produced visible native images
through the separate public converter. Those test processes also exited
normally. The native nonblank-image visual check is now closed. This does not
close sustained playback, A/V sync, physical scanout timing or network output.
