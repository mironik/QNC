# Replaceable decoder adapters

Approved scope: 2026-09-09. No Project, form, source selection, DB, clock,
audio routing or media processing policy changes. No MLT/VLC integration.

## Boundaries

- qnc-media-decode owns the saved request/packet contract, bounded process
  transport, validation and cancellation. It has no FFmpeg dependency.
- qnc-ffmpeg-decode owns FFmpeg command construction, container aliases and
  FFmpeg packet-record parsing. Normal decode still uses one continuous
  FFmpeg process per selected native stream, without an extra wrapper process.
- qnc-decoder-catalog reads an explicit deployment catalog, resolves the
  selected installed adapter and injects it at the player process entry.
  The player runtime does not load configuration or select a technology.
- Additional processes implement the QNC packet protocol, not an FFmpeg CLI.
  Adding one requires a catalog entry, not a player/GUI source change.

The catalog is private deployment configuration, never project/workflow
truth. Project and saved media facts still determine input, timing, channels
and policy. Adapter capabilities only accept or reject that request. No
silent replacement, media discovery, format conversion or new probe.

## Verification gate

Before claiming completion: negative catalog/contract tests, adapter command
equivalence, actual bounded packet protocol tests, dependency conformance,
build of player and existing Ingest, then live Ingest verification. New
external decoder technology itself remains unqualified until tested for
actual codecs, precise seek, AV timing, memory and cancellation. Local byte
bridges can serve existing Local/LAN/Intranet MediaStreams; they do not
implement a remote decoder service. Linux/macOS and LAN live runs must not be
claimed from Windows-only tests.

## Deployment selection

`catalogs/decoders/catalog.json` is shipped as configuration, not embedded
into the engine. Its `selected` entry currently names `qnc.ffmpeg`. Selection
is loaded once at the player process entry and remains fixed for its session.
No extra FFmpeg wrapper executable/process was added.

Default lookup starts beside the player executable and walks its ancestors
for `catalogs/decoders/catalog.json`. A packaged distribution must include
that file beside its executable tree, or configure `QNC_DECODER_CATALOG`.
An explicit override is authoritative: missing/malformed files, unknown
selection, absent executable or unsupported OS/CPU fail without fallback.

An external adapter is registered with its own id, version, installed
executable, OS/CPU list and explicit container/codec/pixel-format lists.
Its driver is `{ "protocol": "qnc_packets_v1", "args": [] }`.
Executable selection is either `{ "kind": "command", "name": "name" }`
from PATH or `{ "kind": "path", "path": "relative/installed-executable" }`
relative to the catalog directory. Absolute executable paths are private
host configuration, never public media identities. No shell expansion occurs.

`Catalog::available` filters registration by executable presence and current
OS/CPU. Capability lists describe what the adapter declares; they do not
certify a particular binary's codec support or frame accuracy. They cannot
override saved facts, conversion policy or project settings. The selected
adapter rejects undeclared formats before starting its process.

There is no new UI chooser in this step. A customer/admin installs a trusted
compatible adapter and selects its registration in this JSON. VLC or another
arbitrary executable is not automatically a QNC adapter; it must implement
the protocol and pass qualification. No additional decoder binary is bundled.

## External process protocol

Public Rust types are in `qnc-media-decode` 0.2.0 (`external.rs`, `adapter.rs`,
`model.rs`). The catalog schema and process protocol each have version `1`;
these are distinct from the saved decode request/packet version `0.2.0`.
Registration `version` identifies the installed adapter package, not a
runtime verification of that binary's provenance.

1. Host starts the configured executable with its registered args and
   `--qnc-decode-v1`. One process handles one native media stream/session.
2. Host sends one bounded `ProcessOpen` JSON line on stdin, then closes stdin.
   It contains the saved request, expected output format, a unique request
   id, private byte-bridge URL/token, storage stamp and maximum packet size.
   The media identity remains the saved QNC URI. Credentials are not arguments.
3. Adapter emits exactly one `ProcessRecord::Ready` JSON line on stderr before
   any packet. Host verifies protocol version, adapter id, request id and
   exact output format. This handshake is not the player's Play Ready state.
4. Before each raw packet on stdout, adapter emits its `PacketHeader` on
   stderr: sequential ordinal from zero, signed integer PTS, rational
   timebase and exact byte size. Stdout carries bytes only; stderr carries
   protocol records only. Metadata-before-bytes avoids pipe deadlocks.
5. End requires closed pipes, successful exit and valid packet completion.
   An error record, timeout, wrong size/order/PTS or unexpected frame count
   is an error, not a successful short clip or decoder fallback.

The existing bounded queues, packet validation, cancellation, kill/reap and
reader joins apply to both drivers. Bounds cover host buffers; adapters must
also bound their own internal allocation and not leave descendant processes.
This is a trusted-plugin boundary, not a security sandbox for arbitrary code.

`start` is a relative source timestamp, not a claimed frame index. Decoder
output keeps actual PTS; the existing engine resolves the exact requested
frame after preroll. An adapter may not fabricate PTS, reinterpret a seek
ordinal as a source frame, run a clock, duplicate/drop frames or probe media.
Video remains in the requested native format; audio remains native channel
order/rate expressed as finite interleaved f32le PCM. Project output routing
and output device behavior remain outside the decoder.

## Verification, 2026-09-09

- 48 targeted unit tests passed across decoder, FFmpeg adapter, catalog,
  player runtime and conformance. Capability rejection is checked before
  process launch; no fallback and unsupported OS/CPU are covered.
- Four explicit FFmpeg integration tests passed: continuous packets/exact
  EOF, nonblocking queue, bounded cancellation/session isolation, corrupt
  media rejection. Test media is generated, not probed.
- Three explicit external-process tests passed: handshake/packet protocol,
  wrong version/count/exit/timeout, cancellation/full queues/session
  isolation. `protocol_fixture` is synthetic protocol test equipment, not
  evidence of another decoder technology's media accuracy.
- `cargo check --all-targets` passed for all six affected decoder/player/
  pixel-conversion packages, including their diagnostic examples.
- Player and Ingest executable builds passed. Conformance passed, including
  the new prohibition of FFmpeg command construction in the neutral engine.
- Windows live test used the actual Ingest form, existing DB and read-only
  Sony card. Startup restored 98 clip records without Select. Mironik 2679
  showed its thumbnail on selection, then video after keyboard-catalog Space.
  The first Play reused the prepared player and five native-stream decoder
  processes (one picture and four original mono streams). No ffprobe process
  was observed during these checks.
- A first Pause/resume observation was inconclusive. The diagnostic repeat
  explicitly confirmed `Paused ready=true carrier=591`, followed by `Playing`
  and advancing carriers 660, 760. Project output was two channels at 48000 Hz,
  selected from four saved original channels. No new channel policy was added.
- Selecting Mironik 2680 replaced the previous session and displayed its
  thumbnail without carrying Play into the new selection. Old child process
  ids disappeared. No form/layout or Project code was changed in this step.

The live run verifies adapter integration and basic controls, not a full
professional playback certification. End-to-end AV offset, sustained frame
delivery under load, full-clip EOF, LAN/Intranet and Linux/macOS were not
qualified in this step. External decoder technology still needs its own
codec/seek/AV/EOF/cancellation qualification before being offered to users.

Reproduction commands (from the QNC root):

```text
cargo test -p qnc-media-decode -p qnc-ffmpeg-decode -p qnc-decoder-catalog -p qnc-player-runtime -p qnc-conformance
cargo build -p qnc-media-decode --example protocol_fixture
cargo test -p qnc-media-decode -p qnc-ffmpeg-decode -- --ignored
cargo check -p qnc-media-decode -p qnc-ffmpeg-decode -p qnc-decoder-catalog -p qnc-player-runtime -p qnc-player-runner -p qnc-pixel-convert --all-targets
cargo run -p qnc-conformance
cargo build -p qnc-player-runner -p qnc-ingest
```
