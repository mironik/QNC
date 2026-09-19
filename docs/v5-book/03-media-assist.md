# 03 — Media Assist: media pool, virtualni kadrovi, transkripti

Kod: `qnc-host/src/media_pool/{db,ingest_db,store,transcripts}.rs`, `virtual_shots/db.rs`, `editor_assets.rs`, `asr/mod.rs`, `story/db.rs` (čitanje popisa). v5 kod je samo izvor informacija o potrebama (00).
V5: Media Assist i Story su isti ekran s dvije uloge; podaci su u `/api/story/*`. Ovdje je opisano ono što Media Assist treba u bazi.

## 1. Što je „klip u projektu”

- **Samo `ingest_assets.import_status = 'imported'`** (`read_imported_clips`). Ostali klipovi (`detected`, `queued`, `processing`, `generating_proxy`, `error`) ne ulaze u pool ni u „All”.
- Prazan popis: poruka „Nema klipova — prvo Ingest import.”
- Popis po `clip_id`. Za svaki klip čita se: naziv, trajanje, fps, putanje proxyja/originala/sličice, audio, probe meta (`ingest_asset_meta`), `virtual_name`.

## 2. Zapisi Media Assista

| Tablica | Sadržaj | Procedura |
|---|---|---|
| `pool_clips(clip_id, status, added_at, updated_at)` | zrcalo uvezenih klipova | M3.2 |
| `media_pool_displayed_clips(clip_id, snapshot_signature, displayed_at, updated_at)` | što je UI već prikazao (za inkrementalno osvježavanje) | M3.4 |
| `media_pool_workflow` (1 red), `media_pool_workflow_selection` | radno stanje (trenutni klip, playhead, mark in/out, aktivni virtualni kadar) | u kodu pisano samo pri migraciji (`migrate_workflow_json`) (zaključak: naslijeđe) |
| `virtual_shots` | source/short/cover kadrovi | M3.5 |
| `clip_transcripts(clip_id, status, text_body, transcript_json, updated_at)` | transkript klipa | M3.6 |
| `clip_transcript_segments(clip_id, segment_index, start_sec, end_sec, text)` | segmenti transkripta | M3.6 |
| FS `virtual_shots/<shot_id>/cover.jpg`, `out_cover.jpg` | IN i OUT slika kadra | M3.5 |

## 3. Procedure

### M3.1 Otvaranje baze pool-a (`media_pool::open_db`)
- Stvara tablice pool-a i pokreće migracije; **poziva `virtual_shots::ensure`** (shema i migracije `virtual_shots`).
- Migracije pri svakom otvaranju: `migrate_media_pool_schema` (dodaje stupce), `migrate_transcript_files` (datoteke `transcripts/<clip>` → baza).

### M3.2 Usklađivanje pool-a s ingestom (`sync_pool_from_ingest_db`)
- Čita uvezene klipove; **briše** iz `pool_clips` one koji više nisu uvezeni; **upsert** ostale (`status='active'`).
- Potreba: „koji su klipovi dio projekta” mora uvijek proizlaziti iz `import_status`; kopija tablice je samo v5 pomoć i nije potreba (08).

### M3.3 Popis klipova za UI (`list_clips_enriched`)
- Poziva M3.2, čita uvezene klipove, dodaje `has_transcript`, `transcript_status` (`none`/`processing`/`complete`), putanje, trajanje, fps, kanale; `transferred = true` za sve.
- Vraća `clips` + sažetak (`pool_summary`: ukupno, otkriveni, validirani, preneseni).

### M3.4 Prikazani klipovi (`mark_displayed_clips`, `list_incremental_updates`)
- `media_pool_displayed_clips` čuva potpis prikazanog klipa; inkrementalni popis vraća samo promijenjene i uklonjene (uklonjeni se brišu iz tablice). Označeno kao neupotrijebljeno (`dead_code`) u v5.

### M3.5 Virtualni kadrovi
Zapis: `virtual_shots` (jedna tablica; polja: `shot_id`, `clip_id`, `kind`, `source_shot_id`, `locked`, `category_key`, `display_name`, `virtual_name`, `in/out` u sekundama i frameovima, `duration_*`, dvostruki fps, `source_fps_num/den`, `field_order`, `interlaced`, `source_class`, `proxy_recipe`, `source_probe_json`, `cover_path`, `out_cover_path`, `in_tc`, `out_tc`, `description`, `data_json`, vremena).

| Procedura | Okidač | Učinak |
|---|---|---|
| **Root (source virtual)** | Ingest probe (`apply_root_media_probe_prepared`); i `ensure_reserved_root_shots_for_project` (popravak/bootstrap za sve klipove) | `reserve_root_virtual_shot`: id `root_<clip_id>`, `kind=import_root`, `locked=1`, `category_key=import_root`, `source=import`, `quality=ok`; IN=0, OUT=trajanje (frameovi i sekunde), fps/timebase iz probea, `source_probe_json`; trajanje/oznaka/boja iz frameova; `in_tc=00:00:00:00` |
| **Add virtual clip (short)** | `POST …/virtual-shot` (`clip_id` + `in_frame/out_frame` ili sekunde; ili `source_shot_id` → izvedeni) | `add_virtual_shot_from_frames`: klip mora biti uvezen (`nije uvezen u ingest`); `OUT` ≥ IN+1; `shot_id` = `<clip>_shot_NNN`; `virtual_name` iz imena korijena; `category_key='manual_cut'`, `kind='virtual'`; `write_shot_covers` (FS: IN i OUT slika, `ffmpeg` s ponavljanjem); podaci o probeu preuzeti iz klipa |
| **Izvedeni kadar** | `POST …/virtual-shot` sa `source_shot_id` | `derive_virtual_shot_from_frames`: lokalni IN/OUT relativno na IN izvora; novi kadar pamti `source_shot_id`, `kind='virtual'` |
| **Ažuriranje IN/OUT** | `POST …/virtual-shot/update` | `update_virtual_shot_from_frames`: root je read-only (`Originalni (root) kadar je read-only.`); preračun trajanja/oznake/boje/TC; nove IN/OUT slike |
| **Brisanje** | u v5 nema rute (funkcija `delete_virtual_shot` postoji, briše red i mapu kadra) | — |
| **Sinkronizacije** | pri otvaranju baze i pri probeu | `sync_virtual_shot_source_fps`, `sync_virtual_shot_probe_meta`, `propagate_source_probe_snapshot`, `backfill_frame_fields`, `backfill_dual_fps`, `migrate_shot_identity_standard` (preimenovanje starih id-eva u `rename_shot_id`, mijenja i `story_parts`, `story_covers`, `story_state`, `media_pool_workflow`) |
| **Slika kadra** | `GET …/virtual-shot/{id}/thumb?kind=in|out` | čita `cover.jpg` / `out_cover.jpg` (stvara ako nedostaje: `cover_path_for_shot`) |

Klasa kadra u v5 (potreba, ne rješenje): dogovoreno strogo razdvajanje source / short / b-roll (docs/89, B2).

### M3.6 Transkripti / ASR
- `POST /api/ai-search/transcribe-stream`: lokalni ASR (whisper.cpp; log datoteke `stdout/stderr`) nad proxyjem klipa; stanja: `processing` → `complete` (ili vraća prethodni). `save_transcript_conn`: briše i ponovno upisuje `clip_transcript_segments`, upsert `clip_transcripts` (`status`, `text_body`, `transcript_json`).
- `POST /api/ai-search/translate-transcript`, `GET /api/asr/health`, `GET /api/translation/health`.
- Zahtjev projekta: lokalno bez plaćene usluge. [nije pročitano do kraja: unutrašnjost ASR/prijevoda]
- Ovo je područje grupe e (Audio AI).

### M3.7 Editor asseti (`/api/story/*` prefiks preko `editor_assets::router`)
| Ruta | Čita |
|---|---|
| `clips` | `list_clips_enriched` (M3.3) |
| `media` | putanja proxyja/originala klipa |
| `virtual-stream` | tok kadra (IN/OUT) |
| `thumbnail`, `filmstrip`, `filmstrip/package`, `filmstrip/placeholder`, `waveform/status` | sličica klipa, filmstrip okviri, valni oblik |
| `virtual-shot` (POST), `virtual-shot/update` (POST), `virtual-shot/{id}/thumb` (GET) | M3.5 |

## 4. Potrebe koje Media Assist iz toga traži (bez preuzimanja v5 izvedbe)
1. Popis klipova = samo uvezeni, iz baze.
2. Status klipa (točkice) iz stanja uvoza i postavki projekta.
3. Za svaki klip postoji source virtual zapis (IN=0, OUT=trajanje).
4. Stvaranje short/b-roll kadra iz source klipa (IN/OUT u frameovima), uz IN i OUT sliku.
5. Transkript klipa (grupa e).
6. Izvor medija razriješen po postavkama (proxy/original) i dostupnost izvora (overlay kad kartice nema).
