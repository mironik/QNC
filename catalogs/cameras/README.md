# Camera profiles

One JSON file per camera model (`<manufacturer>/<model>.json`). The application reads every file at start and
before Select, and keeps the valid ones in the camera database of this computer (`data/qnc-cameras.db`). A new
camera of a known record format is one new file; no code changes. A file with an error is not taken: Ingest
names it and the last valid record of that profile stays.

- `id`, `version`: the profile identity; raise `version` when the procedure changes. The id and version are
  written into every clip record read with the profile, so a project knows how each clip was read.
- `manufacturer`, `model`: the camera. `default_for_reader: true` marks the profile used for a card of the
  record format whose model no profile recognises.
- `reader`: the record format reader (`camera.sony.index.read`: Sony MediaProfile index with NonRealTimeMeta
  sidecars). A new format is a new reader block; a new camera is not.
- `layouts`: the card layouts of the camera catalog (`catalogs/camera-patterns`) this camera writes.
- `recognise`: the camera is recognised when one of the rules matches an element attribute in the card's
  records (`equals` or `prefix`).
- `procedure.metadata`: `declared_when_complete` (the card record is the final record when it states what a
  probe would; otherwise one probe in Ingest) or `probe_once` (always one probe in Ingest).
