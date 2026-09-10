# Player output integration boundary

Scope recorded before implementation, 2026-09-08. Continuation of docs/52.

Review of the existing core before worker composition found three integration
gaps that mock outputs did not expose:

1. Decoded video plus an opened output was sufficient for Ready. There was no
   explicit check that the anchor image had completed output preparation.
2. Playing audio refill called begin/commit preroll again, which resets the
   real audio adapter's queue/generation. Refill must append without restart.
3. Core required FramePresented synchronously. A surface submission must not
   be relabeled as physical presentation merely to satisfy that check.

This step changes only public core/output traits and player data contracts.
The v4 reference is the active qnc-player-runtime and qnc-player-runner plus
AGENTS Jedinstveni model: player owns clock/lifecycle; no app/form clock or
private media monolith is copied. No Project, Ingest workflow, DB or UI edits.

- A required output preparation hook checks the specific decoded anchor before
  Ready. Pending preparation keeps bounded CPU payloads and does not begin audio
  preroll repeatedly. Play never calls that hook or performs opening/upload.
- Normal audio refill calls a separate append operation. No begin, commit,
  reset, silence gate or output/device restart in that path.
- VideoFrameSubmitted and submitted_frame describe an actual output handoff.
  FramePresented/presented_frame remain separate evidence of presentation.
  An adapter with only GPU/surface submission emits only the former. Carrier
  position is an execution position, not a claim about physical scanout.
- Version 0.3.0 rejects the earlier player protocol. No migration/fallback.

Verify bounded deferred readiness, no work on ready Play, pause/resume, queue
continuity during refill, wrong/unconfirmed frame rejection and the distinction
between submission and presentation. Add an explicit native-device diagnostic
using the real public outputs, kept outside production dependencies and UI.

Worker composition remains gated on these corrections. A native test harness
is not an out-of-process runtime and must not make runtime_available true. A/V
clock discipline, asynchronous presentation feedback, sustained refill/seek,
source mapping and Local/LAN/Intranet output transport still need worker-level
verification before form/timeline integration.

## Implemented

- Required `prepare_start_frame` on both output traits. The core invokes it
  after the bounded initial decoded buffer exists, before audio begin/queue/
  commit and before Ready. Pending image preparation retains the same cache;
  it neither redecodes nor repeatedly resets audio. Failure blocks automatic
  retry and readiness. The hook is absent from ready Play.
- `append_playout_audio` is separate from initial preroll. Split output maps
  append to the existing device queue operation, without begin/commit/start.
- `VideoFrameSubmitted` crosses the 0.3.0 protocol without becoming
  `FramePresented`. The core keeps submitted and presented frame fields
  separate, rejects mismatched frame acknowledgments and clears the output
  position on source replacement/unload. No asynchronous scanout claim is added.
- Core production dependencies remain only player-contract and serde. Native
  device dependencies are dev-only for `examples/live_ready.rs`, not imported
  into any application or form. No source, probe or DB dependency was added.

The preparation hook validates the initial anchor's output readiness. A future
worker still owns continuous scheduling/refill of prepared video, output health
and asynchronous completion handling. This is not a claim that every cached
decoded frame is already uploaded to the GPU or that seek is now asynchronous.

## Verification 2026-09-08

- 126 focused tests pass: 63 core, 19 contract, 12 converter, 9 video-output,
  11 audio-output and 12 conformance. Two explicit device unit tests remain
  excluded from this count. The native integration diagnostic below was run
  separately on the actual devices.
- Added tests for deferred image readiness without cache growth/redecode,
  preparation failure, append-only continuous audio refill, append failure,
  submission versus presentation, mismatched acknowledgments and rejection of
  the pre-0.3.0 wire protocol. Existing ready-Play tests forbid all cold calls.
- Scoped all-target clippy with warnings denied, formatting and conformance
  pass. Frozen Project paths have no diff. Workspace formatting has the known
  unrelated source-reader test assertion layout reported in docs/52.

Windows x86_64 native diagnostic, real public WGPU and CPAL outputs, release
build. Synthetic 1280x720 SDR pattern, explicit 50 fps / 100-frame fixture,
48 kHz stereo low-level tone. These are fixture values, never media defaults.

| Measured boundary | Initial Play | Resume |
| --- | ---: | ---: |
| Core Play command (ms) | 1.220 | 0.250 |
| Surface submission call (ms) | 1.189 | 0.238 |
| Start to first audio callback (ms) | 6.654 | 8.886 |
| Driver-reported first-buffer delay (ms) | 10.000 | 10.000 |

Ready followed 659.608 ms of device/fixture preparation; six seconds of idle
Ready left the device's media-submitted sample counter at zero. The diagnostic
then played, paused, prepared from retained resources and resumed. It reached
the exclusive end with frame 99 acknowledged as submitted and presented_frame
still None. Audio append checks retained the same generation while running.
Cold-operation guards stayed enabled during each Play command. A screenshot
confirmed the actual colored image, white marker and letterbox bars. The
40-second diagnostic window closed and the process exited normally.

An initial diagnostic attempt stopped on GPU Busy because the test adapter
tried to free a texture during an in-flight submission. Its failed process
required targeted termination before rebuilding. The test adapter now polls
completion before scheduling another submission and collecting unused slots;
the public GPU safety guard was not weakened. The subsequent full run above
passed and no diagnostic/decoder process remained.

These measurements distinguish command time, output handoff, callback time and
driver delay. They do not measure photons, acoustic latency or synchronized
media A/V. This fixture exercises real output but no camera decode, saved DB
input, network or separate worker process. No media or project data was read or
written; there was no probe. No sustained-camera-playback claim is made.

Reproduce from the QNC root (opens a native test window and plays a quiet tone):

```powershell
cargo run -p qnc-broadcast-player --release --example live_ready --locked
```

## Next

The public runtime manifest deliberately retains runtime_available=false.
Next is the actual worker, with isolated session/generation/sequence transport,
saved-input binding, bounded asynchronous decode/refill and output receivers.
Use the existing core and corrected traits, not another application-specific
player. Real-clip Play/seek/pause and Local/LAN/Intranet tests remain mandatory
before timeline or form integration. The synthetic harness must not be promoted
to the production worker by copying its fixture, UI loop or diagnostic clock.
