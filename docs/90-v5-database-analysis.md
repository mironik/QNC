# 90 — QNC v5: analiza veza s bazom (Media Assist i Story)

Nadopuna dokumenta 89. Izvor: `C:\Users\miron\Projects\QNC_v5\qnc-host\src` (samo čitano).
Status: **analiza, nije implementacija.** Tvrdnje označene „(zaključak)” nisu doslovno u kodu.

## 1. Fizički raspored baza

| Baza | Datoteka | Sadržaj (istina za…) |
|---|---|---|
| Globalna (katalog) | `global_db` | `projects`, `app_settings`, `users`, `sessions`, `source_templates`, `project_templates`, `module_state` |
| Projektna | `projects/<id>/qnc_project.db` | **sve** po projektu: postavke, tijek, ingest, media pool, virtualni kadrovi, Story |

`open_project` otvara jednu SQLite datoteku; `open_ingest`, `story::db`, `media_pool::db` i `virtual_shots::db` samo dodaju svoje tablice u **istu** datoteku (`CREATE TABLE IF NOT EXISTS`). Broker (`ProjectDbBroker`) deklarira: globalna baza = katalog projekata, projektna = „project_truth”, runtime cache = „non_durable_fast_state”, `ui_db_access = host_api_only`.

## 2. Tablice po području (vlasnik u v5 → predloženi vlasnik kod nas)

| Područje | Tablice (v5) | Piše | Čita |
|---|---|---|---|
| Projekt | `project_settings`, `project_settings_kv`, `project_snapshot_kv`, `project_members`, `project_template_snapshot`, `project_workflow_steps/state/step_kv`, `project_data_revisions` | Project | svi |
| Ingest | `ingest_meta`, `ingest_assets` (probe, putanje, `import_status`, sličice), `ingest_jobs`, `ingest_import_batches/_items`, `playback_cache`, `audio_waveforms`, filmstrip | Ingest (host + worker) | svi (kod nas javni pogledi) |
| Media pool | `pool_clips`, `media_pool_displayed_clips`, `media_pool_workflow` (1 red: trenutni klip, playhead, mark in/out, aktivni virtualni kadar), `media_pool_workflow_selection` | Media Assist | Media Assist, Story |
| ASR | `clip_transcripts`, `clip_transcript_segments` | ASR (e) | e, g, l, o |
| Virtualni kadrovi | `virtual_shots` (≈45 stupaca: in/out u sekundama **i** frameovima, dvostruki fps, `kind`, `category_key`, `locked`, `data_json`, `source_probe_json`) | Media Assist **i** Story (dock: Add virtual clip) | Story, export |
| Story | `story_state` (1 red: odabrani part/shot/slot/cover, `draft_updated_at`, `committed_at`), `story_parts`, `story_markers`, `story_marker_slots`, `story_covers`, `story_object_history` (undo) | Story | Story, playlist, export |

Sve poveznice između tablica su **tekstualni ID-jevi bez stranih ključeva** (`clip_id`, `shot_id`, `virtual_shot_id`, `part_id`, `origin_part_id`). Jedini strani ključ u cijelom skupu je `ingest_import_batch_items → ingest_import_batches`.

## 3. Lanac podataka: od ingesta do exporta

1. **Ingest** upisuje `ingest_assets` (probe: trajanje, fps, kanali, `import_status`, putanje originala i proxyja). Poslovi idu kroz `ingest_jobs` (claim, heartbeat, complete, fail).
2. **Media Assist** čita klipove iz `ingest_assets` (`read_imported_clips`) i kreira **korijenski virtualni kadar** po klipu (`import_root`, id `root_<clip_id>`). Radne oznake (IN/OUT, playhead, odabir) piše u `media_pool_workflow*`. ASR zapisuje transkript (`clip_transcripts*`). Stare datoteke transkripta se pri otvaranju baze **migriraju u bazu**.
3. **Story** stvara `story_parts` (raspon izvora u frameovima + `clip_id` + `virtual_shot_id`), markere i slotove (`story_markers`, `story_marker_slots`) te pokrivalice (`story_covers`, slot po `slot_signature`).
4. **Montažna lista** = `EditorialPlaylist`: `build_editorial_playlist_from_conn` čita aktivne partove i covere, gradi segmente i validira timebaseove. Kaže se izričito: „Raw editorial playlist — the montage result stored in source coordinates”, bez ffprobea u vrućem putu.
5. **Program (flat playlist)**: `qnc-program-playlist` iz montažne liste + `ProjectMediaGateway` (razrješava medij po `MediaAccessKind`) gradi `FlatProgramPlaylist`. Isti program ide u **preview** (`PlaybackInput` proxy) i u **export** (`OriginalMaster`).
6. **Export HI-res**: `submit_hires_export_job` gradi payload iz iste liste, upisuje posao u `ingest_jobs` (tip `EXPORT_HIRES`), worker izvršava, status se čita (`export/status`).
7. Aktivni projekt: sve operacije nose `project_id`; UI ne otvara bazu, nego zove host API (`/api/story/...`); UI drži samo predmemoriju (`StoryScreen.parts`, `all_clips`, `markers`…) koja se osvježava iz `/api/story/state`.

Zaključak: **istina za montažu su redovi Storyja u bazi; playlist, preview i export su izvedeni i mogu se uvijek ponovno izgraditi.** To odgovara vašem pravilu.

## 4. Što u v5 NIJE ispravno usklađeno s tim pravilom (nalazi)

1. **Export čita radnu skicu, ne potvrđeni zapis.** `story_state.committed_at` je samo vremenska oznaka; `commit_story` ne pohranjuje što je potvrđeno, a nijedan modul osim `story/db.rs` ne čita `committed_at`. Export i playlist grade se iz živih redova (`list_active_parts`, `list_covers`). Tko uređuje za vrijeme exporta, mijenja izlaz.
2. **Nema revizije za Story.** `project_data_revisions` se povećava samo za opseg `ingest`. Story/virtualni kadrovi/transkripti ne dižu reviziju, pa drugi klijent (LAN) ne može znati da se nešto promijenilo. `FlatProgramPlaylist.revision` je tvrdo `0`.
3. **Pisanje mimo kapije.** Broker deklarira „single_writer_gate_per_project”, ali `story/db.rs` otvara vezu izravno `open_project(...)` na 40 mjesta; kroz broker idu samo playlist i timeline-model. Nekoliko pisača na jednu datoteku (host, worker, Story) bez zajedničke kapije.
4. **Stanje odabira je u bazi kao jedan red za cijeli projekt** (`story_state.id=1`, `media_pool_workflow.id=1`). Dva korisnika ili dva prozora dijele isti „trenutni klip” i „odabrani part”. Prihvatljivo za laptop, neprihvatljivo za LAN.
5. **Dvostruko trajno stanje.** Probe se kopira u `virtual_shots.source_probe_json`, dvostruki fps (`fps`, `source_fps`, `timeline_fps`, `*_num/_den`), in/out i u sekundama i u frameovima uz migracije „backfill”. Izvor istine za trajanje je `ingest_assets`, a kopije mogu odstupati.
6. **Nema stranih ključeva.** Brisanje klipa ili dijela ostavlja siročad (`story_parts.clip_id`, `story_covers.virtual_shot_id`, `origin_part_id`).
7. **Dva pisača `virtual_shots`.** Media Assist i Story oba zapisuju kadrove; DDL postoji u `virtual_shots/db.rs`, a `story/db.rs` ima svoju kopiju u testu (rizik razilaženja shema).
8. **Undo samo za cover.** `story_object_history` podržava jedino `object_type = "cover"`; dijelovi i markeri nemaju povijest.
9. **Migracije u vrućem putu.** Svako otvaranje `open_db`/`ensure` pokreće `migrate_*` i `backfill_*` (I/O čitanje, ponekad probe putanja). Utječe na latenciju i na to tko smije pisati.
10. **Izlaz exporta nije u bazi kao rezultat.** Status exporta živi kao posao u `ingest_jobs` (tip exporta u ingest tablici), a putanja izlaza u `payload_json`. Nema tablice „export” s poviješću i vezom na potvrđenu verziju.

## 5. Preslikavanje na pravila QNC-a (`AGENTS.md`)

Pravilo: veza među aplikacijama je samo baza; čitanje kroz javne poglede vlasnika; pisanje samo kroz transport vlasnika; obitelji `qnc-project*`, `qnc-ingest*`, `qnc-media-assist*`, `qnc-story*` ne ovise jedna o drugoj.

Predloženi vlasnici i javni pogledi (prijedlog, čeka odobrenje):

| Vlasnik | Piše | Javni pogledi za druge |
|---|---|---|
| Project | postavke, tijek | već postoji |
| Ingest (zamrznut) | `ingest_assets`…, filmstrip, wave | `public_clips`, `public_clip_sources`, `public_clip_proxy`, `public_filmstrip_*`, `public_wave_artifacts` (postoje) |
| Media Assist e | transkripti (`clip_transcripts*`) | `public_transcripts` |
| Media Assist g | (nije određeno, v5 nema zasebnu tablicu) | – |
| Media Assist l | virtualni kadrovi, radne oznake pool-a | `public_virtual_shots` |
| Story o | `story_*`, potvrđene verzije | `public_story_program` (montažna lista), `public_story_versions` |
| Export | poslovi i rezultati exporta | `public_exports` |

Što bi se **popravilo** naspram v5 (svako je zasebna odluka):
- Export čita **potvrđenu verziju** (`public_story_versions`: nepromjenjiva snimka partova, covera, markera u trenutku commita), ne živu skicu. Nakon commita export je ponovljiv.
- Svaki vlasnik ima **reviziju** svojih podataka (kod nas već `CatalogSignature` za Ingest); čitatelji je koriste umjesto pollinga cijelih lista.
- Pisanje samo kroz **jedan write transport po vlasniku** (kapija po vlasniku, ne izravni `open_project`).
- Stanje odabira ne ide u dijeljeni red projekta nego po **sesiji/korisniku** (za LAN); na laptopu je to jedna sesija.
- Trajanje ostaje **samo u ingestu**; Story čuva frameove i timebase, ne kopiju probea.
- Strani ključevi ili barem provjere u transportu (brisanje klipa uklanja ili označava ovisne redove).

## 6. Otvorena pitanja (potrebna odluka prije ugovora)

1. **Tko je vlasnik `virtual_shots`?** V5 ga piše i Media Assist i Story ("Add virtual clip" u docku svih formi). Predlažem vlasnika l, a ostali forme zovu njegov write transport. Slaže li se?
2. **Što je točno grupa g** u bazi (audio nakon e)? V5 nema zasebne tablice. Je li g vlasnik lektorirane verzije teksta (transkripta), ili samo status „čeka urednika/lektora”?
3. **Verzije priče:** treba li Story imati više potvrđenih verzija (povijest) ili samo „zadnja potvrđena”?
4. **Odabir/playhead po sesiji** već sada (priprema za LAN) ili tek u LAN fazi?
5. **Export kao vlastiti vlasnik** (tablica `exports`), ili ostaje u ingest poslovima kao u v5?
