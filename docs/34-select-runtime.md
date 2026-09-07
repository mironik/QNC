# Select runtime

Implementation boundary, 2026-09-07. No layout or Project changes.

`ingest_dir_confirm` starts a finite background Select job after a read-only
refresh of active work settings. Navigation, another Select and reload cannot
replace an in-flight job. The form only dispatches intents and polls view state.

The owner configures source and DB bindings in `data/ingest-transport.json`
(or QNC_INGEST_TRANSPORT_CONFIG). These are deployment bindings, not editable
project work settings. Registered source identities stay constant when browsing
children. Private paths are never media IDs. Remote bindings require configured
authority endpoints and credentials; no network URL is inferred from UI text.

The public Dir Browser also supports registered SourceReader roots, with the
same passive BrowserState and no confirm/cancel buttons. Its existing API remains
unchanged. Local and remote listing use the same source contract.

The Ingest Select component composes public scanner, camera readers, source-index
DB, media-record DB, metadata composer and Media Probe. Camera-specific parsing
stays in the camera module. Only confirmed original/proxy groups become clips.
Unsupported/unresolved structures are reported, never guessed by extension.

Source and camera records commit before probe. Every probe request requires a
durable acquisition claim. Results commit before interpretation. Existing final
records and stored acquisition evidence are read without another probe. Failed,
uncertain or unfinished attempts are never retried. Scope: one owner media DB
and stable source URI; separate databases/aliases are not global deduplication.

UI updates are batched and bounded, not per decoded frame. Worker threads end
after completion; closing the component cancels new work, without clearing claims.
This step does not implement media copy, poster generation, filmstrip, waveform
or playback. Existing card thumbnail display is documented in docs/35.

Passive UI wiring: LAN/Intranet paint the same registered-source table as Local,
instead of unconditional empty placeholders. The existing status label shows
compact Select progress/warning count, with detail in its tooltip. The existing
empty-grid message shows a Select error when no clips can be displayed. No panel
geometry, font, control order or button size changes.

Initial verification: 373 workspace tests and conformance passed. The user's
Windows live Select on G: completed with 98 final/complete clips, 196 stored
original/proxy acquisitions, 196 raw JSON and 99 XML documents, with no failed
or uncertain acquisitions. The acquisition window was 30.715 seconds. SQLite
quick_check returned ok. The card was used read-only.

LAN/Intranet contract tests use authenticated loopback source/DB endpoints;
no physical remote server or Linux/macOS live run was tested. Sources require
explicit owner bindings; hot-plug registration and unsupported camera grouping
remain separate work. Missing local serial information cannot verify identity.
