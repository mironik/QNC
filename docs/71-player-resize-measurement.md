# Player resize measurement

2026-09-09. Scope: existing public pixel conversion and player worker. No
Project, Ingest form, selection, probe, source metadata, clock or audio-policy
changes. Source card remains read-only. This is not playback certification.

## Measurement

`RasterConverter::convert_timed` measures resizing separately from color
conversion. The player conversion worker measures queue wait and total job
time, including dispatch to its prepared workers. With
`QNC_PLAYER_DIAGNOSTICS`, it logs averages and maxima every 100 conversions.
Timing does not change pixels, source PTS, frame selection or output routing.
Worker total excludes waiting for the player thread to collect a result.

The serial baseline used the actual Ingest form and its existing saved clip
catalog, Mironik 2679, proxy picture and original audio. Project settings
read from the active database specify two output channels at 48000 Hz.
No Select or probe was run. Source rate is 50 fps, not a substituted project
rate. The UI monitor output is the existing 960x540 RGBA preview.

`target/player-resize-measured.log` contains 71 intact 100-frame summaries:

| Stage | Mean of window averages |
| --- | ---: |
| Resize | 7.518 ms |
| Color conversion | 2.156 ms |
| Queue wait | 0.061 ms |
| Conversion worker total | 9.680 ms |

The longest measured conversion was 40.313 ms. Playback reported audio
underrun at carrier 8107, not EOF. A compiler was active near this failure,
so this run is not an isolated causal test of resize or a controlled
performance comparison. Earlier live runs also reproduced underrun without
this instrumentation. A 20 ms frame period does not mean a 9.7 ms conversion
slows playback by 49 percent: throughput, buffering and scheduling matter.

## Rejected parallel experiment

The experiment used three independent 8-bit Y/U/V planes with separate reusable
resizers and a private pool of at most two threads prepared before Ready.
It kept the existing Bilinear filter, dimensions, chroma geometry, color
conversion, one in-flight frame and bounded frame pool.

The live test did not establish a benefit. The experiment and both added
Rayon dependencies were removed. The final code retains serial conversion
in the existing worker plus timing diagnostics, not the extra thread pool.

The experiment's tests matched all output bytes for 4:2:0, 4:2:2 and 4:4:4,
including odd-sized preview chroma and repeated buffer use. The retained
regression checks reused scratch against fresh conversion. Worker tests retain generation,
frame index and buffer-slot identity, reject concurrent submissions and
verify shutdown without a result consumer.

## Separate transport finding

The existing HTTP monitor response now declares its complete in-memory
payload length and disables chunk framing. A real 960x540 response test
checks exact headers and bytes. This framing change did NOT remove the
roughly 320 ms monitor transfer stalls observed in live diagnostics. It
must not be described as a playback or transport-latency fix.

Continuous monitor delivery, actual audio-to-display offset and scheduling
under load remain separate verification requirements. No frame skipping,
timestamp relabelling, fake Ready or relaxed underrun guard is introduced.
Windows/local evidence cannot certify Linux/macOS or LAN/Intranet playback.

## Live result and stop decision

`target/player-resize-parallel.log` records the actual Ingest test of Mironik
2679. No compiler or ffprobe process was present when Play began. The saved
proxy is 1920x1080 YUV420p at 50 fps; the preview remains 960x540. The initial
audio preroll was 24000 sample frames. Playback failed at carrier 5254 with
an audio underrun, before the 11178-frame clip ended.

Fifty intact 100-frame summaries averaged 6.581 ms resize, 3.044 ms color,
0.259 ms queue wait and 10.422 ms total conversion-worker time. Maximum total
was 50.428 ms. These sequential live runs are not a controlled performance
benchmark, but neither throughput nor stability justified retaining the
extra workers. No complete-clip success is claimed for this experiment.

The user challenged the playback model rather than requesting another local
optimization. Further optimization stopped. The current engine prepares
video before corresponding audio (`decode_frame_to_buffer`), and continuous
audio top-up follows the shared refill path (`tick_playing`). As a result,
slower picture preparation can exhaust audio readiness. A faster resize or
different HTTP framing alone is not evidence of a sound multi-clip editing
engine. The next design review must address independent bounded preparation,
one presentation clock, compositing and audio deadlines before new code.

After removal, 31 focused unit tests passed (one explicit device test remained
ignored) and the player executable was rebuilt successfully. Conformance
passed. The experimental Ingest instance and its own player/decoder children
were stopped. The rebuilt serial diagnostic version was not given a new
complete-clip live qualification; architecture review superseded that test.
