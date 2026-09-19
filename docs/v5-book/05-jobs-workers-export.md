# 05 — Red poslova, workeri, export, playback cache

Kod: `qnc-host/src/jobs.rs`, `ingest/db.rs`, `ingest/import_pipeline.rs`, `background_work.rs`, `export_*.rs`, `playback_cache.rs`, `qnc-worker/src/{lib,main,media_probe}.rs`, `qnc-service-contracts`. v5 kod je izvor potreba (00).

## 1. Model

- **Host** je jedini pisac baze i jedini vlasnik reda poslova. **Worker** je vanjski proces koji preko HTTP-a traži posao, izvršava ga i vraća rezultat; ne otvara bazu.
- Poslovi su retci `ingest_jobs` **u projektnoj bazi** (isti red za sve vrste). Worker se vezuje na aktivni projekt hosta (bez aktivnog projekta nema poslova: `no_active_project`).
- Raspored workera: `local_workstation` (laptop) ili `intranet_shared_media` (dijeljeni medij); zadano lokalno.

## 2. Tablica `ingest_jobs` (stupci)

`job_id` (PK), `job_type`, `source_id`, `clip_id`, `status`, `error`, `attempts`, `payload_json`, `queued_at`, `started_at`, `finished_at`, `updated_at` + stupci usluge poslova (`ensure_job_service_schema`): `worker_id`, `lease_id`, `lease_until_ms`, `heartbeat_ms`, `result_json` (`idx_ingest_jobs_status`, `idx_ingest_jobs_clip`, `idx_ingest_jobs_external_lease`).

Stanja: `queued` → `processing` (lease) → `done` | `error`; povratak u `queued` pri istjeku lease-a ili pri `retryable` neuspjehu.

## 3. Životni ciklus posla

| Korak | Ruta / funkcija | Zapis |
|---|---|---|
| Stavljanje u red | `queue_ingest_job(_payload)`, `queue_ingest_artifact_job_once` (bez duplikata; `requeue_terminal_ingest_artifact_job` ponovno stavlja završen posao) | `ingest_jobs` INSERT (`status='queued'`) |
| Preuzimanje | `POST /api/jobs/claim` (`worker_id`, `capabilities[]`, `project_id?`, `max_jobs`, `lease_ms`) → `claim_jobs_for_project` | `status='processing'`, `worker_id`, `lease_id`, `lease_until_ms`, `attempts+1`, `started_at`; za neke tipove host prvo računa **payload** (npr. plan uvoza) |
| Otkucaj | `POST /api/jobs/heartbeat` (`heartbeat_jobs`) | `lease_until_ms`, `heartbeat_ms` |
| Završetak | `POST /api/jobs/complete` (i `complete-batch`) → `begin_job_completion` → tip-specifična primjena rezultata → `finish_active_job_done` | `status='done'`, `result_json`, lease očišćen; nakon toga `advance_selected_import_pipeline` |
| Neuspjeh | `POST /api/jobs/fail` (`retryable?`) | `retryable` → `queued`; inače `error` + `error`, `finished_at`; lease se provjerava (`lease_not_active`) |
| Istek | `requeue_expired_leases` | `processing` s isteklim lease-om → `queued` |
| Status | `GET /api/jobs/status` | – |

Konstante: zadani lease 30 s (min 5 s, max 300 s), do 256 poslova po zahtjevu, zadano 1; worker odgovara svakih 500 ms.

**Prednost reprodukcije:** dok Broadcast Player svira (`BackgroundWorkGate`, lease 5 s; `POST /api/shell/background/playback`), host dodjeljuje **samo `export_hires`**; svi ostali tipovi čekaju.

## 4. Tipovi poslova

| Tip | Tko stavlja u red | Što worker radi | Što host zapisuje |
|---|---|---|---|
| `media_probe` | `MediaProbeScheduler` (nakon discover) | ffprobe (brzi) | `ingest_assets` probe polja + `probe_json`, root `virtual_shots`, `media_probe_batch` |
| `ingest_media_prepare` | `queue_import` / pipeline | link ili kopiranje u komadima (`.partial` → konačno) u `original/`, `proxy/` ili `audio/` | `complete_imported_clip` (I2.8) |
| `proxy_generate` | `queue_import` (GenerateProxy) | transcode u `proxy/` | `complete_imported_clip` |
| `original_archive_copy` | nakon uvoza uz arhiviranje | kopija originala u `original/` | `mark_original_archive_copied_conn` (klip `original_ready`) [zaključak] |
| `thumb_copy` | `queue_import` / pipeline | kopija kartične sličice u `poster.jpg` | `apply_poster_copy_result` (`thumb_status/thumb_path`) |
| `thumb_proxy` | odobrenje korisnika | poster iz proxyja | `apply_thumb_proxy_job_result` |
| `waveform` | pipeline (faza `waveform`) | valni oblik | `audio_waveforms` |
| `audio_wrap` | nakon uvoza (`queue_project_audio_wrap_jobs`) | audio-omot | `ingest_assets` (`record_audio_wrap`) |
| `playback_cache_prepare` | nakon uvoza ako postavke traže `on_import` (i medij nije u projektu) | predmemorija u komadima | `playback_cache` |
| `export_hires` | `POST export/hires/submit` | render iz montažne liste na originalima | `ingest_jobs` (status, `result_json`), izlazna datoteka u export direktoriju |
| `discover` | `discover()` | – (samo evidencija u redu) | `ingest_jobs` |
| `qnc_worker_smoke` | test | – | – |

## 5. Export HI-res

1. `POST /api/story/export/hires/submit` (ili `/api/render/hires/submit`) → `submit_hires_export_job`.
2. `export_id = hires_<uuid>`; izlazni direktorij iz postavki (`export.directory`, inače standardni `exports/` projekta); putanja `<projekt>_<export_id>`.
3. `prepare_hires_flat_payload`: `build_editorial_playlist_with_broker` → flat program s `MediaAccessKind::OriginalMaster` → `ExportHiResJobPayload` (`timeline_timebase`, `duration_frames`, `items[]`; ekstenzija izlaza iz stavki).
4. `queue_hires_render_job` → `ingest_jobs` (`export_hires`, izvor `story_export`).
5. `GET export/status` / `render/hires/status`, `POST export/cancel`.
6. Worker (`ExportHiResJobHandler`, `export_hires_reencode`) renderira stavku po stavku (filter po stavci, `ffconcat`, A1/A2, kodek/kontejner po presetu; validacija timebaseova).
[nije pročitano do kraja: render, presetovi]

## 6. Playback cache
- Politika u postavkama projekta (`playback_cache_policy_from_settings`, način `on_import`, broj komada).
- `playback_cache(source_id, clip_id, status, priority, input_locator_json, ready_start_frame, ready_end_frame, duration_frames, total_chunks, ready_chunks, cache_path, error, requested_at, updated_at)`, PK `(source_id, clip_id)`.
- Ne radi za medij koji je već unutar direktorija projekta.

## 7. Potrebe (bez v5 izvedbe)
1. Red poslova u bazi vlasnika, s lease-om i ponovnim pokušajem.
2. Worker bez izravnog pisanja u bazu.
3. Prioritet reprodukcije nad pozadinskim poslovima.
4. Idempotentno stavljanje u red i atomski izlaz datoteka.
5. Rezultat posla vidljiv svim aplikacijama kroz bazu (status klipa, artefakti).
