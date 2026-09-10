# One Broadcast Player process entry

Scope before implementation, 2026-09-08. The application's public playback
entry is Broadcast Player, not a second runtime service. The existing native
composition stays inside that process. No application, Project or UI changes.

Use the existing CommandEnvelope/EventEnvelope and JSON transport. One process
owns one immutable saved source descriptor, one session, clock and native output.
Play/Pause/Stop/CueFrame dispatch directly to the verified player code. Requests
cannot select another session, generation or source; replayed commands fail.
State reads return player-derived snapshots, not a second client clock or state.

The small runner executable is qnc-broadcast-player. Its private bootstrap is
bounded JSON on stdin: saved PreparedInput, session identity, source binding,
explicit device channel map and transport credentials. No DB/probe work occurs
in the runner. A caller obtains the descriptor through the existing public
read-only input reader. Private local paths/tokens never appear in command/event
payloads; HTTP media uses the existing resolver/media-stream adapter.

Control uses POST /v1/player through qnc-json-transport. The process binds only
loopback; a configured TLS gateway can expose the same authenticated endpoint
on LAN/Intranet. There is no application caller allowlist. Read credentials
cannot execute commands. Bounded command handoff keeps HTTP body reads/writes
off the player owner thread. Expired queued commands are not executed.

This first process output is explicitly a native test window/device on the
player host, using the same diagnostic geometry as docs/54-55. It is NOT the
finished embedded Ingest monitor or remote AV delivery. That boundary and
physical sync remain open; the full module must not claim runtime_available.
No production application is connected to this diagnostic output.

The external keyboard catalog already supplies `play_pause` and frame-step
actions; Ingest already dispatches those action IDs. This step adds no keyboard
binding, duplicate action or UI keyboard handler. The existing low-level Play
and Pause protocol commands are executor operations, not new shortcuts. The
Ingest action remains disconnected until its public player attachment is tested.

Normal shutdown or loss of the controlling caller releases decoder/output
resources. The bootstrap supplies a bounded inactivity timeout, independent of
project workflow. No reconnect, replay, auto-restart or fallback engine.

Verification: request isolation/replay/permission tests, real separate-process
read-only saved-media test with native video/audio, and conformance. Do not
equate loopback success with physical LAN/Intranet or other OS verification.

## Implemented entry

- `tools/qnc-player-runner` builds the single `qnc-broadcast-player` executable.
  Winit only owns the native test surface. The player owner thread calls the
  existing Runtime; HTTP I/O has a bounded handoff and no playback clock.
- Public `qnc-player-contract::session` defines State/Command requests and
  replies. State reads are full snapshots assembled from the player's current
  state. They do not consume command sequence numbers or maintain another
  media state. The snapshot is not a lossless historical event subscription.
- Source identity is fixed at process startup. Unsupported source/program
  commands are explicitly rejected; there is no fake LoadSource success.
  Shutdown is process lifecycle, not a new keyboard shortcut. Other action IDs
  and all existing keyboard bindings are unchanged.
- Accepted cue means preparation has been requested, not that its target has
  been presented. Readiness remains false until native preparation completes.
  GPU submission remains distinct from physical presentation.
- HTTP command timeout is not permission to replay. Sequence replay is rejected;
  expired queued requests are rejected before execution. Invalid auth/session
  messages cannot keep the player alive. An authenticated controlling caller
  keeps the session alive with State requests during playback.
- `--native-output` is required. The bootstrap version is 0.3.0, with a 4 MiB
  limit, bounded session identity, positive source generation, explicit native
  channel map, read/command tokens, listen_port (0 for an assigned port), and
  idle_timeout_ms in 1000..300000. Startup stdout announces the private loopback
  endpoint and output scope, never credentials. Normal control JSON is 64 KiB
  maximum and uses the existing authenticated JSON transport.

## Verification

Windows, 2026-09-08. Read-only G: card, saved clip
`clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b`, active saved project
`novi-cjeloviti-1`. The public input reader selected the proxy from DB policy;
the native runtime used its actual saved audio inventory and explicit 0,1 device
routing. This is not a claim that the original is stereo; its four mono channels
were verified separately in docs/54-55.

- Final rebuild finished before the final live run. Earlier, a still-running
  test executable blocked replacement on Windows; that run was not treated as
  final-build evidence. The processes exited, then build and live were repeated.
- Final real processes: controller -> HTTP -> player PID 20912, with independent
  player PID 8300. Verified Play/Pause/Stop, source-frame cue, exclusive end,
  replay from zero, command replay rejection, wrong session/generation rejection,
  read-only command rejection and no cross-session position changes.
- The second player exited after its 2-second control inactivity timeout.
  The first acknowledged Shutdown and exited normally. No player/diagnostic/
  FFmpeg process remained afterwards.
- Prepared Play HTTP roundtrips: 3680/2785/1419/2214 us. These include local
  control transport and are not first physical scanout/acoustic latency.
  A native screenshot from the final build showed the actual clip after seek,
  with intact aspect ratio. This test is not physical A/V synchronization proof.
- Selected source SHA-256 and read-only saved settings were unchanged. No DB
  writes, probe, source mutation, application UI changes or keyboard edits.
- 119 focused tests passed: core 69, contract 21, native composition 9,
  existing keyboard 5, HTTP handoff/auth 2, conformance tests 13. Clippy with
  warnings denied and conformance passed. The audio-only ignored live test
  was not rerun in this process-control step; see its prior actual results.

Explicit local test after building the runner and example:

```powershell
cargo build -p qnc-player-runner --release --bins --examples --locked
.\target\release\examples\live_control.exe C:\Users\miron\Projects\QNC G:\ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b 0,1
```

Those paths are private test bindings. No deployment credentials are saved in
the repository; the example creates fresh temporary credentials per process.

Remaining: embedded/client-side AV output, Ingest attachment of the EXISTING
keyboard action, source changes/preload/program playback, physical A/V sync,
headless output and actual LAN/Intranet/Linux/macOS/ARM tests. The full module
therefore remains `runtime_available=false`; `native_control_available=true`
describes only the verified process control and host-native output.
