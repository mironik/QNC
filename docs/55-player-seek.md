# Native player seek

Scope before implementation, 2026-09-08. Continue the existing native runtime;
no new application, UI loop, queue layer, DB field or Project change.

Reference: v4 CueFrame -> transport sync_range_runtime, and its decoder's
25-frame input preroll. The new core stages a cue, keeps the last confirmed
position until the target is prepared, and rejects Play while not ready.
The native adapters reopen only their decode sessions during seek preparation;
the GPU device, surface, converter and audio device stay open. Ready Play still
does no media open, decoder start, DB read or preroll.

Use the saved source timebase to seek slightly before the target, discard video
by exact source PTS and trim PCM by sample positions. Output ordinals are never
source frame identities. Preserve every native audio stream and explicit device
channel routing. No probe, media repair or representation fallback.

A superseding cue replaces pending work. Failed preparation stays not ready,
does not confirm the target and does not retry itself. GPU generations reject
old prepared tokens. Seeking while paused stays silent; only Play starts audio.

Verification: core pending/failure/supersession boundaries, rational seek/sample
tests, then real read-only saved-input forward/back/end/replay with retained
devices. This does not finish the out-of-process host or Ingest UI attachment.

## Verified result

2026-09-08, Windows local read-only camera card, active saved project
`novi-cjeloviti-1`, clip `clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b`.

- Core cue dispatch only stages work. The player owner performs decoder
  repositioning on subsequent preparation ticks, retaining output devices.
  Invalid/exclusive-end requests are rejected before interrupting playback.
  Pending work may be superseded; a failed preparation is not retried on ticks.
- Runtime uses the saved representation and stream metadata through the
  existing media-stream binding. No DB read or source discovery is added to
  seek or Ready Play. GPU generation changes invalidate old frame tokens.
- Four saved-input AV runs completed forward/back/end/replay, with explicit
  supersession after a preparation tick. Targets were 241, 0, 481, 160, 0
  in the DB-selected proxy (482 frames, 50 fps). Preparation across these
  runs was 272-651 ms; Ready Play command/submission was 183-353 us.
  Last run: preparation 440/272/387/438/301 ms; Play 310/353/218/209/205 us.
  These are not physical display/acoustic latency measurements.
- The strengthened diagnostic verifies silence before Play, actual audio
  callback/sample delivery after Play, the exact submitted target, and final
  frame retention at the exclusive end. A native screenshot after the last
  seek showed the actual Dubrovnik Forum clip with preserved aspect ratio.
- Original audio was tested separately without changing playback.input:
  four independent mono PCM streams, each 462720 sample frames. Seeking to
  241, 481 and 0 reproduced exactly the same PCM samples in all four channels
  as sequential decoding, taking 646/639/258 ms. Device monitoring remained
  explicit channels 3/4, not an invented stereo downmix.
- Source SHA-256 and saved settings stayed unchanged. No source or DB writes,
  ffprobe, application UI/Project changes or extra decoder worker layer.
- 119 focused unit/contract tests passed across core, runtime, player-input,
  media-decode, audio-output and video-output. Four explicit FFmpeg integration
  tests passed; the real original-mono playback/seek test also passed. Clippy
  with warnings denied and conformance passed. Diagnostics exited and reaped
  their decoder processes.

Run the saved-input seek diagnostic after building the release example:

```powershell
.\target\release\examples\live_saved.exe C:\Users\miron\Projects\QNC G:\ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b 0,1 seek
```

These are private test-machine bindings, not public module identities. The
diagnostic selects the representation from saved project settings; it does not
override that policy to choose a convenient fixture.

Remaining: production out-of-process command/AV transport, Ingest UI attachment,
audio-master synchronization, full original AV test, physical sync and actual
LAN/Intranet/Linux/macOS/ARM verification. Seek preparation is not instantaneous;
this closes native correctness for the tested saved media, not the entire player.
