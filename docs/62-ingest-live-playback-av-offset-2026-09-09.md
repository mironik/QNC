# Ingest live playback and A/V offset, 2026-09-09

## Scope

Actual standalone `target/release/qnc-ingest.exe`, not a helper player.
Mironik 2002 was selected by its thumbnail and started with Space through
the existing `play_pause` shortcut. The project's saved preset is `default`;
the existing catalog maps Space to that action. No shortcuts were added.
Other preset propagation was not verified by this measurement.

Project settings and media metadata were read from the existing project DB.
The saved proxy has 1920x1080 video at 50/1 fps and 48000 Hz AAC audio,
both starting at PTS zero. Original and proxy are one clip, not two clips.
No scan, probe, import, source-card write, Project code change or layout change
was performed for this test. The active project was bbvvcx.

## Reproduced stall and limited change

Before moving conversion, actual Ingest stopped at frame 2024 (40.48 s).
The recorded failure was an audio queue underrun, with a 93.339 ms tick,
71.873 ms maximum tick gap and 11.507 ms average CPU pixel conversion.

CPU pixel conversion was moved to one bounded worker inside the existing
public player runtime. It has one in-flight job, reuses the existing RGBA
pool and rejects old-generation completion after seek. The player owner
thread no longer performs that conversion synchronously. Queue sizes were
not increased. This is not an MLT integration or a replacement engine.

A subsequent actual Ingest run progressed past two minutes but still failed
at frame 6188 (123.76 s), with an empty audio queue. This change is therefore
NOT a completed stability fix.

## A/V measurement

The user reported desynchronization. Diagnostic observation was added to the
public audio output, player runner/client and passive UI raster renderer.
It does not alter playback scheduling or insert an audio delay.

- `AV_A`: the first PCM sample of a driver callback buffer, its sample rate,
  generation and driver-estimated playback time.
- `AV_F`: session/output-generation/sequence to actual source-frame mapping
  observed by the monitor client.
- `AV_V`: the same identity at CPU paint submission, after texture update.
- Driver observations are read without waiting on the real-time callback.
  The callback does no logging, I/O, allocation or blocking synchronization.
- Logging is opt-in through `QNC_PLAYER_DIAGNOSTICS`.

All processes in this measurement ran on the same Windows host. Timestamps
are correlated on that host. This is NOT a cross-host clock-sync protocol.
The nearest audio observation within 75 ms was used for each mapped UI paint:

```text
audio_seconds = first_sample / sample_rate
              + (video_paint_unix_ns - audio_playback_unix_ns) / 1e9
video_seconds = source_frame / 50
offset_ms     = 1000 * (audio_seconds - video_seconds)
```

Positive offset means audio is ahead of the frame submitted for display.

| Measurement | Run 1 | Run 2 |
| --- | ---: | ---: |
| Matched UI observations | 1927 | 1660 |
| Median offset | +74.82 ms | +74.29 ms |
| Mean offset | +78.97 ms | +77.75 ms |
| 5th percentile | +47.11 ms | +41.76 ms |
| 95th percentile | +116.21 ms | +120.65 ms |
| Maximum observation | +417.50 ms | +520.98 ms |
| Last matched video frame | 1610 | 1517 |
| Failure frame | 1618 | 1519 |

Excluding the first five seconds still gives medians +73.95 and +73.39 ms.
The result is approximately 74-75 ms, or 3.7 video frames, of audio lead at
this measurement boundary. The offset is not constant.

Run 1 ended at 32.36 s with `audio output failed or underrun`; maximum tick
gap was 21.554 ms. Run 2 ended at 30.38 s with `due AV frame is not prepared`;
maximum tick gap was 38.602 ms. Neither was a successful full-clip playback.

Raw logs (local, ignored build artifacts):

- `target/ingest-playback-live.stderr.log`: initial synchronous-conversion run.
- `target/ingest-playback-async.stderr.log`: bounded-worker run.
- `target/ingest-av-measure.stderr.log`: both A/V measurement runs.
- Run 1 session: `9493f2d5-b2fc-427f-8f41-5aba863ff7f8`.
- Run 2 session: `c3b3bd5c-41d4-4ca8-96b7-d20c01691b38`.

## Limits and next issue

This measures driver-estimated audio timing versus CPU paint submission,
NOT sound leaving a speaker versus photons leaving a display. CPAL 0.17.1
WASAPI itself estimates playback time from callback/buffer duration
(`src/host/wasapi/stream.rs`, `output_timestamp`). GPU presentation, display
scanout, speaker latency and any original recording lip-sync error were not
measured. Frame granularity is 20 ms. Diagnostic logging can affect jitter;
the maxima must not be treated as a calibrated hardware offset.

The current HTTP monitor path has no final display acknowledgement
(`presented=None`). Healthy audio queues or matching engine counters are
not proof of synchronization with the Ingest monitor. A fixed 75 ms audio
delay is not justified by these variable observations. The next playback
change must address output pacing, monitor delivery and the owner clock,
with another actual Ingest measurement. No such synchronization fix was
made in this measurement step.

## Verification

- `cargo test -p qnc-audio-output -p qnc-player-runtime -p qnc-player-client
  -p qnc-ui-kit --lib`: 36 passed, 2 explicit hardware/helper tests ignored.
- `cargo build --release -p qnc-ingest -p qnc-player-runner`: passed.
- Conformance with absolute QNC root: all checks passed, including v4
  keyboard catalog extension and public player boundary.
- Actual Ingest: thumbnail before Play; Space starts playback; two measured
  runs reproduce audio lead and playback failure.
- Not verified: complete clip playback, calibrated acoustic/display sync,
  LAN/Intranet A/V sync, Linux/macOS or physical broadcast outputs.

The player is not ready to be declared stable or synchronized.
