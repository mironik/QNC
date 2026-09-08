# Incremental Select and immediate preview

User-approved change, 2026-09-08. Project code and settings stay unchanged.

- Re-selecting a source reconciles its catalog; it does not clear the board.
- Existing final clip records are compared through a lightweight public DB
  inventory. They are not decoded, probed or published again.
- A clip loaded into memory is displayed immediately. A bounded background
  writer batches publication. Pending/failed publication is not durable success;
  selection/import cannot consume an uncommitted clip. Probe acquisition claims
  and raw evidence remain durable before probe/interpretation, as required by
  the once-only probe contract.
- Existing clips are labelled `Postojeci` on the thumbnail using the current
  font/palette. This is the only approved visual addition; board geometry stays
  unchanged. The marker means present in the DB before this Select, not imported.
- Missing source originals are removed from the detected catalog only after a
  successful scan and explicit read-only NotFound confirmation within the
  selected scope. Unreadable/offline sources never imply deletion. Imported or
  queued work is retained. Source files and acquisition evidence are not deleted.
- All DB operations use the same typed Local/LAN/Intranet contract. No schema
  migration, new Project setting, separate catalog or app-to-app call is added.
- Camera indexes are mutable source files. The public media-records module
  assigns each captured XML body a SHA-256 evidence URI under the same QNC
  authority, retaining the original source identity/path in that URI. New
  camera recordings can use a newer index without overwriting old evidence.
  Validation binds the URI to the exact body and confirmed camera document.

## Verification

- Targeted tests cover background publication with a blocked writer, atomic
  batch rollback, source differences, unchanged poster/probe reuse, selection
  durability, incomplete scans and scope boundaries, and mutable XML versions.
- Contract tests exercise Local/LAN/Intranet using authenticated loopback
  endpoints. Conformance and the standalone Windows build pass.
- Eight repeated concurrent Select tests pass. Container validation now uses
  a read transaction so simultaneous schema bootstrap cannot mix snapshots.
- Windows live: G: (serial de666c9f), 98 clips. Selected Mironik 1483.MXF,
  re-selected the same card, and observed the board/selection remain in place
  during Select. Closed/restarted Ingest; all 98 posters and that selection
  were restored. `Postojeci` is painted above, not over, the selection checkbox.
- Actual project DB still has 98 clip/probe/proxy rows. Probe acquisitions and
  snapshots remain 196/196. All ten original Project tables and global Project
  registry rows compare equal before/after; only Ingest selection was changed.
- The original card was read-only throughout. Adding/removing source files was
  tested only in isolated camera fixtures, never on G:. No exact live speedup
  ratio is claimed because the previous executable was not benchmarked.

Not run: physical LAN/Intranet deployment, Linux/macOS GUI, full workspace test
suite, new real-camera recordings added to G:. Import execution, Filmstrip,
Wave and Player remain outside this step. Project/Shell source is unchanged.
