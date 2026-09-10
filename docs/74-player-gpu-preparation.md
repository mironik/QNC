# GPU raster preparation: scope and verification

2026-09-09, live continuation 2026-09-10. Continuation approved after docs/73.
This is the first measured part of docs/72 section 5, not a completed
GPU-to-monitor transport. LIVE ACCEPTANCE FAILED: the user still observes
stutter and skipped video and rejects continued patching in this direction.
Do not treat faster conversion or reaching the clip boundary as acceptance.

The separate public `qnc-gpu-raster` module implements GPU preparation of
the same saved SDR pixel description. The existing `qnc-pixel-convert`
keeps its no-GPU executor boundary; it only exposes pixel layout/validation
and fit math used by both implementations. GPU raster scales and converts outside the
engine/UI/audio thread, in the existing bounded conversion worker. The
monitor path explicitly selects this implementation before Ready. Unsupported
GPU/format produces a preparation error, not an automatic decoder fallback.
The native output retains its existing CPU adapter in this step.

The existing packed RGBA monitor contract is retained temporarily, including
GPU readback. No UI/layout changes, no native handle crossing the network,
no new playback clock, no probe, no Project/settings/card writes. This narrow
measurement separates preparation cost from the already observed HTTP
delivery stalls. It does not claim to remove those stalls or implement the
final direct GPU monitor. Readback, full conversion time and live completion
must be assessed before choosing the next output/transport change.

Saved DB settings and their read-only application path were rechecked:
project bbvvcx, playback.input=proxy_if_available, audio.channels=2,
audio.sample_rate=48000. V4 GPU monitor and its Jedinstveni model were read
as references; active v4 code is not copied.

## Build and targeted verification

168 tests passed: 145 across the engine/runtime/audio/pixel/input/client
libraries, 4 runner tests, 16 conformance tests and 3 GPU raster tests.
The two normally ignored real-GPU tests were explicitly run on hardware.
Two other real-device tests remained ignored in the library batch.
GPU checks cover saved 420/422/444 layouts at 8/10 bits, full/limited range,
sRGB/BT709 transfer, odd dimensions, invalid payloads, buffer reuse and
scaled color bars with different top/bottom content. They do not prove
bit-identical resampling of arbitrary images or full broadcast certification.

The initial prototype incorrectly put wgpu in qnc-pixel-convert. Conformance
rejected it. GPU execution was extracted to qnc-gpu-raster; the original
CPU module's no-GPU boundary remains enforced and has a regression test.
The rebuilt conformance executable passes, including Project freeze.

`cargo build -p qnc-ingest -p qnc-player-runner -p qnc-conformance` passed.
No application/UI code, Project settings, keyboard catalog, source card or
probe logic was changed in this step. Earlier unrelated worktree changes
are not part of this statement. No compile or probe process ran alongside
the measured playback. Process snapshots confirmed the expected sibling
player and five existing FFmpeg decode children, not ffprobe.

The first verification build ran out of disk space (LNK1318), followed by
an invalid incremental pixel test link. Package-scoped `cargo clean` for
runtime/GPU/pixel build artifacts and a clean rebuild resolved these build
failures. Databases, card contents and playback logs were not deleted.

## Actual Ingest results

Inputs came from the existing cached catalog, without Select/probe. On
selection the monitor showed the thumbnail; Play was sent using the
existing keyboard shortcut after Ready. Output used Intel Iris Xe Graphics,
the saved proxy picture and original audio with two project channels at 48 kHz.

| Run | Engine observation | Display acceptance |
| --- | --- | --- |
| 2679, 9 Sep | Boundary 11178 reached, final frame 11177, 223.56 s at 50 fps | Not certified smooth; HTTP stalls remain |
| 2002, 9 Sep | Incomplete, about frame 2211 | Invalidated by laptop standby |
| 2002, 10 Sep | Boundary 10194 reached, final frame 10193, 203.88 s; explicit Pause/Play at frame 2652 | FAILED: user reports stutter/skipped video |

Windows System log records Modern Standby due to Lid at 23:04:12 on 9 Sep.
The interrupted overnight run is not counted as a successful full playback.
The morning run used the same built player, without another code change.
Pause held frame 2652 and Ready; resume retained player PID 11920 and all
five decoder PIDs (20672, 20716, 20176, 12760, 20136). The 1-frame step and
switch-during-Play tests were not completed in this step after the user's
rejection of the playback result. Earlier CPU lifecycle tests are not a
substitute for completing them on this build.

## Measurements and remaining defect

| Measurement | 2679 | 2002 morning run |
| --- | --- | --- |
| Complete 100-frame conversion batches | 111 | 101 |
| Mean conversion including GPU readback | 5.435 ms | 6.336 ms |
| Largest conversion time in those batches | 49.639 ms | 49.450 ms |
| Largest complete HTTP frame-transfer measurement | 324.729 ms | 334.425 ms |

The earlier CPU observations in docs/73 averaged about 12.2 ms for 2679
and 11.3 ms for 2002. These are observations on different live runs, not a
controlled benchmark or proof of maximum frame latency. For 2679, 100
complete GPU detail batches averaged upload/enqueue 2.015 ms, GPU execution
plus readback wait 2.585 ms and mapped copy 0.813 ms. They are CPU wall times,
not hardware GPU timestamps. Interleaved incomplete stderr lines are excluded.

Confirmed structural issue, inspected after the user's 10 Sep rejection:

- `tools/qnc-player-runner/src/control.rs`: FrameSlot holds only one picture;
  set_frame overwrites it. respond_frame returns the current slot, not an
  ordered sequence of frames awaiting presentation.
- `crates/qnc-player-client/src/connection.rs::poll`: state/commands and the
  blocking post_binary frame request share the same client flow.
- `crates/qnc-player-client/src/lib.rs`: publish replaces the latest View.
  An egui repaint is not a guaranteed presentation of each supplied frame.
- Runtime diagnostics still say presented=None for this monitor path. There
  is no confirmed display-presentation acknowledgment here. Engine completion
  therefore does not demonstrate that the monitor showed every due frame.

A 334 ms delivery stall spans about 17 periods at 50 fps. Faster GPU
conversion does not correct latest-frame replacement or presentation timing.
Changing just the protocol name, increasing buffers, or adding UI timing is
not an accepted repair. This step must not be called a finished player.

Before more implementation, agree on the output contract: bounded ordered
frame ownership, presentation deadlines from the existing engine clock,
independent control delivery, and observable output failure/confirmation.
The form remains passive. Local shared memory is only one transport adapter,
not proof of LAN/Intranet behavior. Do not add a second engine or an app-owned
clock. The GPU code remains in the worktree for inspection; no rollback or
further playback implementation was performed after the rejection.

Evidence: target/player-gpu-preparation-20260909.log and
target/player-gpu-resume-20260910.log. Physical A/V synchronization, sustained
display cadence, Linux/macOS, LAN/Intranet, multilayer editing and sleep/wake
recovery are NOT verified. The live criterion remains open.
