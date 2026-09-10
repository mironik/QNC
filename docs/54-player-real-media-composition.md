# Real media player composition

Scope before implementation, 2026-09-08. Follows docs/53 and the user's
direction to keep the v4 composition simple, without another decoder owner.

One qnc-player-runtime owner composes the existing core, saved-input contract,
media-stream, decoder, converter and native output modules. No new packet
mailbox, decode-job wrapper, application state or form playback loop.
The decoder's existing bounded receiver gains a nonblocking read operation.
The core retains partial AV packets while that operation is pending.

The first native integration exercises an actual DB-selected representation,
initial preparation, Play, pause/resume and the exclusive end. It supports
one progressive SDR video stream and native audio streams from the selected
representation. Other layouts fail explicitly, never by dropping channels or choosing another media.
No original/proxy policy override, probe, DB writes or Project changes.
Native packet PTS are checked against saved source timing for every decoded
frame. Unknown frame-rate mode is preserved, not relabeled constant. Non-grid
timestamps are rejected, not rounded into another source frame.

Audio packets are sliced using rational source FPS and saved sample rate;
video bounds trim codec padding. No resampling/downmix/clock in UI. The
runtime owns the reference clock. The example creates only a native surface,
with all preparation and execution on one player owner thread.

This is a public composition library and explicit standalone diagnostic, not
a completed network worker or UI integration. Session command transport,
remote output receiver, async seek and broadcast A/V clock
discipline remain unverified. The full worker manifest remains unavailable.
The supplied MediaStream opener can bind local or remote transport; a local
card diagnostic does not prove physical LAN/Intranet deployment.

Verification will include input/timestamp/sample boundary tests, core pending
buffer tests, existing decoder tests, real read-only card playback and scoped
conformance. v4 service.rs/runtime.rs/build_runtime are structural references;
no active v4 code is copied and no existing application layout is changed.

## Discrete audio correction, 2026-09-08

The inspected original has four mono PCM streams; its proxy has one two-channel
AAC stream labelled stereo in saved metadata. A two-channel device or container
layout is not evidence that the original is a stereo recording. The previous
live test exercised the proxy only, not the original's four independent tracks.

Keep the DB representation policy unchanged. The runtime now assembles every
native stream in saved stream/channel order without summing, truncating or
inventing a proxy-to-original correspondence. Each existing decoder retains its
own bounded queue; there is no new decoder worker/mailbox layer. Partial PCM
from one track is not consumed while another track is pending.

Device monitoring requires an explicit ChannelMap from the public audio-output
module. For example, 2,3 listens to the third/fourth native channels, and 0,0
listens to the first mono channel on both outputs. This is device routing, not
source metadata or a stereo downmix. No project setting is created or changed.

The first real-media run previously underrran; a subsequent run passed. Those
results did not prove stable playback. Readiness now retains eight AV frames
independently of the single-frame per-tick work budget (160 ms at 50 fps).
This is bounded in the existing core cache/output pool, not another queue or
a retry. Live stability must be remeasured; no physical sync claim is made.

The public video pool accepts up to 16 slots, still under its existing 512 MiB
byte ceiling. This composition reserves 12 slots for eight prepared frames
and upload/release slack. The old eight-slot ceiling rejected this plan before
opening playback; the pool-boundary test now covers the new ceiling.

## Verification

- Original audio adapter: all four saved mono PCM streams, 48 kHz/24-bit
  source. Each yielded exactly 462720 sample frames. Peaks were respectively
  0.07071328, 0.04735422, 0.00038731098 and 0.31989193, measured separately
  before device routing. The device listened to channels 3/4, without mixing.
  The final repeated run drained completely; first callback 7.087 ms,
  driver-reported delay 10 ms. Source hash and saved settings unchanged.
- Earlier audio-only runs failed with empty output queues. Immediate muxer
  packet flushing alone did not fix them. Instrumentation observed a 113 ms
  scheduling gap with the old 80 ms reserve, and a test loop that slept even
  while its queue needed replenishing. The final adapter test catches up
  immediately after a delay, waits when pending/full, and uses the same bounded
  160 ms preparation target as the runtime. Two eight-frame runs completed;
  this is not a guarantee against arbitrary OS/network stalls.
- Full saved-input live test: DB-selected proxy, 482 frames at 50 fps.
  Ready 749 ms; Play 975 us; resume 253 us. First callbacks 8.007/8.250 ms,
  driver delay 10 ms. Pause at frame 50; exclusive end after frame 481;
  481 position changes. Maximum tick 17.991 ms, scheduling gap 19.563 ms.
  Native window screenshot showed the real clip with intact aspect ratio.
  The runtime held the final frame without trying to decode frame zero again.
- The original-audio adapter test explicitly reads the original saved record.
  It does NOT change playback.input, substitute the original into the proxy
  descriptor or claim an original-video/proxy-video channel correspondence.
  The full AV test continues to obey the DB policy.
- No probe, source writes, settings writes, application UI changes or new
  Project data. Clippy and conformance pass. Broader regression results are
  recorded below after their final run.

Final verification: 115 focused unit/contract tests passed across player-input,
runtime, core, media-decode, audio-output and video-output. Four explicit FFmpeg
integration tests passed, including cancellation/full queues, exact EOF,
corrupt media and nonblocking receive. Clippy (`--all-targets --no-deps`,
warnings denied), conformance and `git diff --check` passed. Project/app/UI
freeze paths remain unchanged.

Second final-build AV pass: Ready 1043 ms, Play 584 us, resume 240 us;
first callbacks 6.790/7.382 ms with 10 ms driver-reported delay. Last submitted
frame 481, 481 position changes, maximum tick 14.208 ms and scheduling gap
15.957 ms. It completed without underrun or reinitialization. No source/settings
changes were detected. Preparation varies; these command/submission/callback
measurements are not claims of physical scanout or acoustic synchronization.

Native seek/replay was subsequently implemented and verified in docs/55.
Still incomplete: production out-of-process command/AV transport, Ingest UI
attachment, audio-master synchronization, full original AV
live test, physical sync and LAN/macOS/Linux/ARM verification. The public
worker remains marked unavailable. Do not present this diagnostic as a finished
Broadcast Player or a finished Ingest live test.

Explicit local diagnostics (run from the QNC root, supplied card stays read-only):

```powershell
.\target\release\examples\live_saved.exe C:\Users\miron\Projects\QNC G:\ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b 0,1
$env:QNC_LIVE_ROOT = 'C:\Users\miron\Projects\QNC'
$env:QNC_LIVE_SOURCE = 'G:\'
$env:QNC_LIVE_CLIP = 'clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b'
cargo test -p qnc-player-runtime --release --locked real_saved_original_mono_tracks -- --ignored --nocapture --test-threads=1
```

Those paths are this machine's private diagnostic bindings, not public module
identities or OS-specific application defaults. The diagnostics read the
currently active project through the existing public settings/DB reader.
