# 02 — Ingest: izvor, otkrivanje, probe, odabir, uvoz

Kod: `qnc-host/src/ingest/{api,db,store,scanner,asset_row,import_actions,import_finish,import_pipeline,poster_copy,thumb,proxy_*,audio_wrap,orchestrator}.rs`, `ingest_probe/`, `ingest_proxy/`, `ingest_posters/`, `ingest_audio_wrap/`, `media/resolve.rs`, `jobs.rs`.
Baza: projektna `qnc_project.db`; tablice se stvaraju pri prvom `open_ingest` (`init_schema`) uz `ensure_ingest_dirs` (`ingest/thumbnails/`).

## 1. Zapisi Ingesta (sažetak; potpuni rječnik u 06)

- `ingest_assets` — jedan red po klipu `(source_id, clip_id)`. Ključna polja: `import_status`, `status`, `selected`, probe polja, putanje (`source_path`, `original_path`, `proxy_path`, `project_proxy_path`, `thumb_path`, `card_thumb_path`), `thumb_status`, `virtual_name`, `has_video`, `probe_json`.
- `ingest_meta` (`key`,`value`): `active_source_id` (zadano `local`), `browse_path`, `source_scan_root`, `card_root`, `card_locked`, `media_probe_batch` (`pending`/`processing`/`done`), `selection_revision`, `archive_original`, `poster_proxy_approved`.
- `ingest_jobs`, `ingest_import_batches`, `ingest_import_batch_items`, `playback_cache`, `audio_waveforms` (05, 06).

## 2. Procedure

### I2.1 Odabir izvora — `POST /api/ingest/source` → `set_active_source`
- Piše: `ingest_meta.active_source_id`.
- Ako je `browse_path` postavljen: odmah `discover` (I2.3). Zatim `after_discover` (I2.4).
- `POST /api/ingest/browse` → `set_browse_path` (`ingest_meta.browse_path`) pa `discover`.

### I2.2 Ručna registracija datoteka — `POST /api/ingest/register-files`
- `register_media_paths` (I2.3, koraci 5–7) nad popisom datoteka; zatim `after_discover`.

### I2.3 Otkrivanje (`discover`)
1. `queue_ingest_job('discover', source, '_')` pa `mark_ingest_job_processing` (posao u `ingest_jobs`, samo za evidenciju).
2. Korijen skena: `ingest_meta.browse_path` ako postoji, inače `<projekt>/incoming`.
3. `scanner::scan_inventory`: rekurzivno do dubine 8; `media_files` (video/audio po ekstenziji), `card_root` (`resolve_card_media_root`), `thumb_files` (`THM`/`JPG` sličice kartice, npr. mapa Thmbnl).
4. Piše `ingest_meta`: `source_scan_root`, `card_root`, `card_locked` (`1` za breaking news projekt, inače `0`).
5. `group_media_files`: grupira original + proxy + sličicu istog klipa.
6. Za svaku grupu `upsert_media_group` (`INSERT … ON CONFLICT(source_id, clip_id) DO UPDATE`):
   - novi red: `import_status='detected'`, `status='on_source'`, `selected=0`, `thumb_status` = `ready` ako poster u projektu već postoji, inače `pending`;
   - pronalazi sličicu kartice (`card_thumb_path`, `poster_source` = `card_thm`/`card_jpg`) i proxy kartice (`proxy_path`);
   - `virtual_name` = `virtual_name_for_root_clip(clip_id, ekstenzija)`;
   - pri ažuriranju **čuva** `selected`, `import_status`, ready poster i `virtual_name` odabranih klipova; `status` ne prepisuje ako je klip uvezen.
7. `reconcile_source_assets`: **briše** iz `ingest_assets` klipove izvora kojih više nema u skenu (samo ako sken nije prazan); `purge_non_video_clips`.
8. `ingest_meta.media_probe_batch = 'pending'`; posao `discover` → `done` (ili `error` uz poruku).
- Odabir se ovdje ne dira: odabir je odluka projekta (I2.6).

### I2.4 Zakazivanje probea — `after_discover` → `MediaProbeScheduler.enqueue(project)`
- Host-side scheduler (blokira se po projektu). `queue_missing_media_probe_jobs` (u kapiji projekta):
  - za svaki klip izvora čiji `probe_json` nije „spreman” (`media_probe_snapshot_ready`: valjan timebase i trajanje) i čiji izvor postoji: `queue_ingest_artifact_job_once('media_probe', source, clip)` (bez duplikata);
  - `ingest_meta.media_probe_batch` = `processing` ako ima nedovršenih, inače `done`.

### I2.5 Probe posao (worker)
- Worker preuzima `media_probe` (`ffprobe`, brzi način: `probesize` 1 MiB, `analyzeduration` 100 ms), vraća točan probe (`MediaProbe`: format, timebase, `duration_frames`, kanali, …).
- Host `complete_media_probe_job` (jedna transakcija, provjera lease-a):
  1. `record_media_probe_result_conn` → odbija nepotpun probe (`frame timebase/duration required`);
  2. `write_media_probe_result`: `ingest_assets`: `duration_sec`, `fps`, `resolution`, `codec`, `has_audio`, `audio_channels`, `audio_sample_rate_hz`, `audio_sample_format`, `field_order`, `interlaced`, `source_class`, `proxy_recipe`, `source_fps_num/den`;
  3. `ingest_assets`: `source_width`, `source_height`, `source_frame_count`, `source_duration_frames`, `has_video`, `probe_json`;
  4. **`virtual_shots`: korijenski virtualni kadar** (`apply_root_media_probe_prepared`): `reserve_root_virtual_shot` (id `root_<clip_id>`, `kind=import_root`, zaključan) i upis probea (`in=0`, `out=trajanje`, `fps`, timebase, `source_probe_json`, oznaka trajanja i boja) — vidi 03;
  5. `ingest_jobs` → `done` (`worker_id/lease_id` očišćeni, `result_json`);
  6. `finish_media_probe_batch_if_idle_conn`: ako nema nedovršenih probea, `media_probe_batch='done'`.
- Zaključak: **klip dobiva „source virtual” zapis već pri probeu, prije uvoza.**

### I2.6 Odabir klipova (trajni podatak u bazi)
| Ruta | Funkcija | Učinak |
|---|---|---|
| `POST /api/ingest/selection` | `save_selection` | u transakciji: `selected=0` za cijeli izvor, pa `selected=1` za poslane; `selection_revision` (stariji zahtjev se odbacuje) upisuje `ingest_meta.selection_revision` |
| `POST /api/ingest/selection/toggle` | `toggle_clip_selection` | preokreće `selected` jednog klipa |
| `POST /api/ingest/selection/select-all` | `select_all_clips` | ako je odabrano < ukupno → sve odabrano, inače ništa |
- Ne provjerava se `import_status`; odabir je moguć u svakom stanju.
- `virtual_name` odabranih klipova popunjava `backfill_virtual_names_for_selected` (migracija/pomoć).

### I2.7 Uvoz — `POST /api/ingest/import` → `queue_import` (u kapiji projekta)
- **Ulaz je samo baza:** `SELECT clip_id FROM ingest_assets WHERE source_id=aktivni AND selected != 0` (poslani `clip_ids` se ignoriraju).
- Čita: `project_effective_settings` (postavke), `ingest_meta.card_root`.
- Za svaki odabrani klip:
  1. `import_status` `imported`/`done` → preskoči (`skipped_imported`); `queued`/`processing`/`generating_proxy` → preskoči (`skipped_active`).
  2. `resolve_import_plan(meta, postavke)` (vidi 3.1). Greška → `row_import_error` (`import_status='error'` + poruka), `failed++`.
  3. `ImportActionPlan`: `CopyCardPosterIfAvailable` pa `PrepareMedia` ili `GenerateProxy`.
     - Poster: ako `ingest/thumbnails/<clip>/poster.jpg` postoji → `thumb_status='ready'`, `thumb_path`; inače `enqueue_existing_poster_copy` (posao `thumb_copy`); nema li sličice na kartici → `thumb_status='no_card_thumb'`; greška → `thumb_status='error'` + poruka.
     - Medij: `PrepareMedia` → `import_status='queued'`, posao `ingest_media_prepare`; `GenerateProxy` → `import_status='generating_proxy'`, posao `proxy_generate`.
  4. `UPDATE ingest_assets SET import_status=… WHERE … AND import_status NOT IN ('imported','done')`.
  5. Prvi klip serije stvara `ingest_import_batches(batch_id='import_<uuid>', status='preparing')`; svaki klip: `ingest_import_batch_items(batch, source, clip, media_job_type)` i `queue_ingest_job`.
- Odgovor: `queued`, `skipped_imported`, `skipped_active`, `failed`, `poster_queued`, `poster_missing`, `actions_queued`, `batch_id`. Nakon toga asinhrono `advance_selected_import_pipeline` (I2.10).

### I2.8 Izvršenje uvoza: `ingest_media_prepare` / `proxy_generate`
- **Claim** (`claim_jobs_for_project`): za `ingest_media_prepare` host računa plan i payload (`payload_for_ingest_media_prepare_claim`): izvor, odredište (`original/` za kopiju originala, `proxy/` za proxy, `audio/` za audio), operacija (`Link`/`CopyOriginal`/`CopyProxy`/`CopyAudio`), `asset_status`, `read_from_card`, `card_locked`; **`import_status='processing'`** (samo ako je bio `queued`).
- Worker izvršava (kopiranje u komadima kroz `.partial`, zatim preimenovanje; ili transcode za proxy).
- **Host završetak** (`apply_*_result` → `complete_imported_clip` / `complete_imported_audio_clip`, u kapiji projekta, jedna transakcija na `ingest_assets`):
  - `import_status='imported'`, `status`=`asset_status` (`linked`/`ready`);
  - `project_proxy_path` (samo ako je medij pod `proxy/` projekta), `original_path`, `thumb_path` (ako poster postoji), `read_from_card`, `card_locked`;
  - konačni probe ako je dostavljen (trajanje, fps, raster, kanali, timebase, klasa izvora, proxy recept);
  - `bump_project_data_revision('ingest')`.
- Nakon toga: `queue_project_audio_wrap_jobs`; `playback_cache::enqueue_on_import_if_configured`; za `ingest_media_prepare` uz `archive original` i original na kartici: posao `original_archive_copy` (izvor `original_archive`).
- Kvar: `fail_job` (05): `retryable` → posao natrag u `queued`; inače `error`; klip ostaje u `processing`/`generating_proxy` dok host ne označi grešku [zaključak: `row_import_error` pri planiranju; tijekom izvršenja stanje klipa nije pročitano do kraja].

### I2.9 Poster
- `thumb_copy` (kopija kartične sličice u `ingest/thumbnails/<clip>/poster.jpg`) i `thumb_proxy` (poster iz proxyja, tek uz izričito odobrenje: `POST /api/ingest/thumbs/from-proxy` → `ingest_meta.poster_proxy_approved`, `enqueue_proxy_generate`). Rezultat: `ingest_assets.thumb_status`, `thumb_path`, `thumb_error` (`apply_poster_copy_result`, `apply_thumb_proxy_job_result`). `GET /api/ingest/thumbnail`, `…/thumbnails/batch` samo čitaju. [nije pročitano: unutrašnjost `poster_copy.rs`/`thumb.rs`]

### I2.10 Serija uvoza (`advance_selected_import_pipeline`)
- U kapiji projekta, za svaku otvorenu seriju (`preparing`/`filmstrip`/`waveform`):
  - `preparing`: dopuni nedostajuće poslove medija i `thumb_copy`; kad su **svi klipovi serije** `imported`/`done`/`error` **i** `thumb_status ∈ {ready,no_card_thumb,error}` → za klipove `imported` s videom stavlja u red `waveform` (jednom) i prelazi u `waveform`;
  - `filmstrip` (naslijeđena faza): odmah prelazi u `waveform`;
  - `waveform`: kad su svi `waveform` poslovi `done`/`error`/`failed` → serija `done`.
- Piše: `ingest_jobs` (novi), `ingest_import_batches.status`.
- Poziva se i nakon `complete_job`/`fail_job` (host), pa se serija sama vodi do kraja.

### I2.11 Waveform, audio_wrap, playback cache
- `waveform`: `audio_waveforms` (`status`, `a1_peaks`, `a2_peaks`, `peak_count`, `error`, `render_version`, `updated_at`) preko `waveform/store::mark`; `GET /api/ingest/waveform/status|peaks`. [nije pročitano: generiranje]
- `audio_wrap`: `ingest_assets` (`record_audio_wrap`, `mark_audio_wrap_error`) + `bump revision ingest`. [nije pročitano do kraja]
- `playback_cache_prepare`: samo ako postavke traže `on_import`; ne radi za medij unutar projekta; `playback_cache(status, priority, input_locator_json, ready_start/end_frame, duration_frames, total_chunks, ready_chunks, cache_path, error)`.

## 3. Pravila koja procedure koriste

### 3.1 Plan uvoza po postavkama (`resolve_import_plan`)
| `storage.ingest_media` | Način | Izvor | `status` |
|---|---|---|---|
| `link` | `LinkInPlace` | po `playback.input`: `proxy` (treba proxy), `original` (treba original), `proxy_if_available` | `linked` |
| `proxy` | proxy kartice → `CopyToProject`; nema → `GenerateProxy` (transcode originala u `proxy/`); audio → kopija | proxy/original | `ready` |
| `original` | `CopyOriginalToProject` u `original/` | original | `ready` |
- Greške (poruke): `playback_input_missing: link ingest zahtijeva playback.input`; `playback.input=proxy, ali kamera nema proxy`; `playback.input=original, ali original nije dostupan`; `nema proxy ni originala — otkrij materijal`; `nema originala — otkrij materijal`.
- Arhiviranje originala (`archive_original`, `ingest_meta`, zadano isključeno; `ingest_archive_original_available` = ne za breaking news / house media, i samo kad `ingest_media` nije zadan).

### 3.2 Značenje stanja `import_status`
`detected` → (`queued` | `generating_proxy`) → `processing` → `imported`; `error`; naslijeđeno `done` (= imported); `original_ready` (nakon arhiviranja). Uvezen klip: `imported`/`done`.

## 4. Što čitaju drugi
- **Media Assist/Story:** samo `import_status='imported'` (03).
- **Player/Export:** `original_path`, `proxy_path`/`project_proxy_path`, `probe_json`, `playback_cache`.
- **Shell/UI:** `GET /api/ingest/state` (`load_state`: izvori, klipovi, poslovi, `import_queued`…), `/api/ingest/options`.
