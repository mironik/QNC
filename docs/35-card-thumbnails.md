# Existing card thumbnails

The Select runtime already persists explicit original/proxy/related-file links
in source-index DB. Thumbnail display must consume those persisted links, not
guess filenames, scan again or run media probe/generation.

Public SourceReader adds bounded read-only binary transport. The public
qnc-image-assets module decodes JPEG/PNG bytes with allocation/dimension limits,
without filesystem, DB or process access. Sony's reader identifies an explicitly
linked JPG thumbnail; an ambiguous or absent link produces no replacement image.

The Ingest component loads thumbnails in its background Select job, including
when final media records already exist. Passive qnc-ui-kit paint handles RGBA
texture upload/cache. The form only paints the supplied image in the existing
v4 thumbnail rectangle; no geometry or styling changes.

This is not filmstrip/poster generation, playback or a new probe pass.

SourceReader wire version 0.3.0 adds `source.bytes.read` (maximum 4 MiB), using
the same URI binding, confinement, authorization and bounded-response checks as
text reads. Local/LAN/Intranet clients share the operation. Remote peers must
use the matching version; no compatibility fallback is introduced.

## Verification, 2026-09-07

- 42 targeted tests passed across Ingest components, SourceReader, image-assets
  and UI kit, including local/LAN/Intranet binary reads and repeat Select without
  new acquisition. Targeted clippy with warnings denied passed.
- The texture upload test caught an egui context lock re-entry. Upload now runs
  outside the UI memory lock; the test completes normally.
- Both qnc-app and standalone qnc-ingest binaries built successfully.
- In the rebuilt Windows qnc-app live run, the user selected G: again. The grid
  displayed real, distinct camera thumbnails and 98 clips. Read-only DB checks
  before/after both found 196 acquisitions and 98 final/complete records;
  quick_check remained ok. No additional probe acquisition was created.
- Rebuilt conformance passed all checks; git diff --check passed.
- The subsequent full-workspace test build could not finish: the MSVC linker
  reported insufficient disk space and PDB errors. Only about 6 MiB remained
  on C:. A request to remove the regenerable incremental cache was blocked by
  the execution tool, so no cache was removed. Do not report this run as a full
  workspace test pass. The earlier 373-test run predates thumbnail wiring.
- No physical LAN/Intranet host, Linux/macOS live run, or exhaustive per-card
  pixel inspection was performed. Missing/ambiguous camera thumbnail references
  still produce no replacement image; thumbnail generation is separate work.
