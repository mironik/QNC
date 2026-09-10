# Broadcast Engine stability repair

Date: 2026-09-10

Scope: Broadcast Player / Broadcast Engine only. Project, Ingest workflow, DB
contracts, probe rules and media records were not changed.

## Changed

- Renamed the internal playback engine crate from `qnc-player-runtime` to
  `qnc-broadcast-engine`.
- Renamed the module manifest from `player-runtime.module.json` to
  `broadcast-engine.module.json`.
- Kept `qnc-broadcast-player.exe` as the out-of-process executable entry.
- Replaced the monitor's single last-frame slot inside the engine with a small
  ordered queue of submitted monitor frames.
- Runner publishes only the latest submitted monitor frame from each drained
  batch through the local latest-frame map. A slow UI may skip stale visual
  frames, but it cannot block playback.
- Monitor frame records now carry the saved source clip timebase. A client
  rejects a monitor frame whose timebase differs from the prepared source clip.
- Frame responses are served by a bounded frame worker, separate from the main
  control request loop.
- Player client now reads monitor frames on a separate bounded worker, so
  Play/Pause/Cue and clip changes do not wait for a large RGBA frame transfer.
- Player control and monitor transport now use a persistent `qnc-player+tcp`
  socket wire protocol instead of HTTP. Control messages still carry the same
  JSON session contract; monitor frames are returned as length-prefixed binary
  RGBA packets without HTTP headers or chunking.
- Broadcast Engine now opens only the saved audio streams needed by the active
  project's audio channel count. On a Sony four-mono source with a two-channel
  project, only A1/A2 are decoded. This is not a stereo mix and not a fallback;
  the channel count still comes from the project DB snapshot.
- Audio clock fallback no longer jumps to the anchor when driver timing is
  temporarily unreadable; it reuses the last stable playback position.
- The previous fixed `16 ms` Ingest repaint loop has been removed. Playback
  cadence remains the saved source timebase plus player/audio clock; display
  refresh rate is not a QNC playback rule.

## Not Changed

- No ffprobe/probe path was added.
- No Project dependency was added.
- No Ingest workflow or Project settings model was changed.
- No stereo mix or local audio-channel default was added.
- No new decoder technology was selected.

## Verified

- `cargo check -p qnc-player-runner -p qnc-broadcast-engine -p qnc-conformance`
- `cargo test -p qnc-audio-output -p qnc-broadcast-engine -p qnc-player-runner -p qnc-player-client --quiet`
- `cargo build -p qnc-player-runner -p qnc-ingest --quiet`
- `target/debug/qnc-conformance.exe C:\Users\miron\Projects\QNC`

## Still Needs Live Acceptance

The previous user-visible failure was stutter/skipping and imperfect A/V sync
through Ingest. This repair removes two confirmed code defects, but the final
acceptance remains a real Ingest playback test on the Sony card.
