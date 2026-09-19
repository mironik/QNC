# 09 — Ingest u QNC v5: potpuni spisak akcija i procesa (i status kod nas)

Izvor: kod QNC v5 (`qnc-app/src/ingest/mod.rs`, `qnc-app/src/app.rs`, `qnc-host/src/ingest/*`,
`qnc-worker/src/lib.rs`, `qnc-host/src/jobs.rs`). Podrobni tokovi i rječnik podataka: `02-ingest.md`,
`05-jobs-workers-export.md`, `06-data-dictionary.md`. Ovdje je jedan popis; uz svaku stavku piše
što naš kod (grana `dev/2026-09-19`) radi. Status: **da** = radi kao v5, **drukčije** = radi, ali
po odluci korisnika ili drukčijim putem, **nema** = ne postoji.

## A. Akcije korisnika (v5 `IngestAction` i akcije komponenti)

| # | v5 akcija | Što radi u v5 | Kod nas |
|---|---|---|---|
| A1 | `RequestState` / `Reload` | učita stanje ingesta iz baze (`GET /api/ingest/state`), ponavlja polling dok ima posla | da (`INGEST_RELOAD`, `CatalogLoader`) |
| A2 | `RequestDirList(path)` / `ConfirmDir` / `CancelDir` | pregled i potvrda direktorija izvora (lokalno; LAN i Internet u v5 nisu bili spojeni) | da (`qnc-source-browse`, LAN/Intranet iza transporta) |
| A3 | `SelectKind(Local/Lan/Internet)` | promjena vrste izvora | da |
| A4 | `Toggle(clip)` (checkbox) | **optimistično**: okrene lokalno odmah, spremi **cijeli odabir** s revizijom (`save_import_selection`), pri grešci vrati staro | **drukčije**: okreće se tek nakon upisa u bazu; nema revizije odabira; nema vraćanja |
| A5 | `SelectAll` / `ClearSelection` | isto kao A4 za sve klipove (`select_all_clip_selection_local`, `clear_clip_selection_local`) | drukčije (isto što i A4) |
| A6 | `ImportSelected` (Uvezi) | ako nema odabranih: greška "Nema odabranih klipova."; sprema odabir, stavlja uvoz u red i **odmah prebacuje shell na sljedeću aplikaciju** (`go_workflow(Next{from:"ingest"})`, poruka "Uvoz pokrenut → {next}"); kopiranje teče u pozadini | **nema prijenosa u sljedeću aplikaciju**; uvoz se izvršava u pozadini, poruka "Uvoz je pokrenut." |
| A7 | `ApproveProxyPosters(ids)` | odobrenje postera iz proxyja za klipove bez postera na kartici | akcija postoji, ali **nema izvršitelja** (ništa ne radi); po odluci korisnika odobrenje nije potrebno: poster se kreira sam kad ga nema |
| A8 | `SetArchive(bool)` | opcija "arhiviraj original", sprema se u `ingest_meta.archive_original` | djelomično: akcija samo postavlja lokalni prikaz (`view.archive_original`), ne zapisuje se u bazu i nema izvršitelja |
| A9 | `SetAiMining(bool)` | lokalni prekidač AI (bez baze) | da (iz postavki projekta) |
| A10 | `FocusPreview(clip)` | pripremi preview klipa (player, timeline, filmstrip, wave) | da (`qnc-source-preview`) |
| A11 | `TogglePlay`, `CueFrame(n)`, korak ±1 frame (I/O u pool headu), scrub | upravljanje playerom | da (play/pauza, cue, korak); I/O oznake nema |
| A12 | zaključavanje radnji dok svira player | `playback_active` pauzira teške poslove | da (`qnc-playback-priority`, kopiranje čeka) |

## B. HTTP rute hosta (v5 `qnc-host/src/ingest/api.rs`)

| # | Ruta | Funkcija | Kod nas |
|---|---|---|---|
| B1 | `GET /api/ingest/state` | `load_state` | da (`qnc-ingest-catalog::load`) |
| B2 | `POST /api/ingest/source` | `set_active_source`, pa `discover` ako je `browse_path` postavljen | da (`confirm_source_selection` u registry store, pa Select) |
| B3 | `POST /api/ingest/browse` | `set_browse_path` + `discover` | da |
| B4 | `POST /api/ingest/discover` | otkrivanje (I2.3) | da (Select: scan) |
| B5 | `POST /api/ingest/register-files` | ručna registracija datoteka | nema |
| B6 | `POST /api/ingest/selection` | `save_selection` (cijeli odabir + revizija) | drukčije (odabir po klipovima kroz `ContentWriteTransport::select`) |
| B7 | `POST /api/ingest/selection/toggle` | `toggle_clip_selection` | drukčije (isto) |
| B8 | `POST /api/ingest/selection/select-all` | `select_all_clips` (sve ili ništa) | drukčije |
| B9 | `POST /api/ingest/options` | `set_ingest_archive_original` | nema |
| B10 | `POST /api/ingest/import` | `queue_import` (I2.7) | djelomično (`queue_selected` + `claim_next`; nema serija) |
| B11 | `POST /api/ingest/thumbs/from-proxy` | odobrenje postera iz proxyja | nema (i više se ne traži) |
| B12 | `GET /api/ingest/thumbnail`, `POST /api/ingest/thumbnails/batch` | čitanje postera | da (`qnc-media-thumbnail`) |
| B13 | `GET /api/ingest/waveform/status`, `/peaks` | čitanje waveforma | da (`qnc-wave`, `qnc-timeline-artifacts`) |
| B14 | (kroz `editor_assets`) `filmstrip`, `thumbnail` | filmstrip klipa | da (`qnc-filmstrip`) |

## C. Procesi

| # | v5 proces | Sažetak | Kod nas |
|---|---|---|---|
| C1 | otkrivanje (`discover`) | sken do dubine 8, grupiranje original+proxy+sličica, `upsert_media_group`, `reconcile_source_assets` (briše nestale), `purge_non_video_clips` | da (Select: `scan` + `records`; nestali samo uz potpun sken) |
| C2 | zakazivanje probea (`after_discover`, `MediaProbeScheduler`) | probe za sve klipove čiji `probe_json` nije spreman | **drukčije po odluci**: probe samo kad zapis kartice nema XML podatke, jednom |
| C3 | probe posao (`media_probe`, `complete_media_probe_job`) | upis probea u `ingest_assets` + **korijenski virtualni kadar** `root_<clip_id>` u `virtual_shots` (`kind=import_root`) | probe da; **`virtual_shots` nema** (javni modul nije napravljen) |
| C4 | odabir klipova | `ingest_assets.selected`, `ingest_meta.selection_revision` | drukčije (`clips.selected`, bez revizije) |
| C5 | plan uvoza (`resolve_import_plan`) | `link` / kopija proxyja (ili generiranje) / kopija originala; audio kopija | da za link, proxy, original; **nema generiranja proxyja**, **nema audio kopije** |
| C6 | serija uvoza (`ingest_import_batches`, `ingest_import_batch_items`) | jedna serija po Uvezi, faze `preparing → filmstrip → waveform → done` | nema (nema tablica serije) |
| C7 | posao `ingest_media_prepare` | host računa payload i preuzima klip (`import_status='processing'`), worker kopira u komadima | da (`ClaimNext`, `Importer`); kopija je obična, bez provjera (odluka korisnika) |
| C8 | posao `proxy_generate` (+ `proxy_encode`, `proxy_source`, HW enkoder) | transcode originala u `proxy/` | nema |
| C9 | završetak uvoza (`complete_imported_clip`, `complete_imported_audio_clip`) | `import_status='imported'`, `status` `linked`/`ready`, `project_proxy_path`, `original_path`, `thumb_path`, konačni probe, `bump_project_data_revision('ingest')` | djelomično: `imported`, `imported_media_uri`, `thumbnail_uri`; **nema** `status`, putanja proxy/original zasebno, ni revizije podataka |
| C10 | posao `thumb_copy` | kopija kartične sličice u `ingest/thumbnails/<clip>/poster.jpg`; `thumb_status` `ready` / `no_card_thumb` / `error` | da (pri uvozu, isti put; `thumb_status` nema) |
| C11 | posao `thumb_proxy` | poster iz proxyja uz odobrenje | nema; po odluci korisnika poster se **kreira kad ga nema** (bez odobrenja) — nije napravljeno |
| C12 | `waveform` (faza serije) | peaks A1/A2 po klipu, `audio_waveforms` | da (radnici `qnc-wave-worker`, ne kao faza serije) |
| C13 | filmstrip | artefakti filmstripa | da (`qnc-filmstrip-worker`) |
| C14 | `audio_wrap` | omatanje audio-only klipova | nema |
| C15 | `playback_cache_prepare` | cache za reprodukciju uz postavku `on_import` | nema |
| C16 | `original_archive_copy` | arhiva originala uz opciju `archive_original` | nema |
| C17 | `export_hires` | izvoz visoke kvalitete (nije ingest) | izvan opsega |
| C18 | lease i ponovni pokušaj poslova (`fail_job`: `retryable` → `queued`, inače `error`) | `ingest_jobs`, worker `heartbeat` | djelomično: najam s otkucajima za `Processing` (izmišljen, nije iz v5) |
| C19 | `advance_selected_import_pipeline` | vodi seriju do kraja nakon svakog završenog/neuspjelog posla | nema |
| C20 | prijenos shellu nakon Uvezi | `go_workflow(Next{from:"ingest"})` | **nema** |

## D. Baza (v5 je polazište)

v5 drži sve u projektnoj `qnc_project.db` (`ingest_assets`, `ingest_meta`, `ingest_jobs`,
`ingest_import_batches`, `ingest_import_batch_items`, `audio_waveforms`, `playback_cache`,
`virtual_shots`). Statusi:
- `import_status`: `detected`, `queued`, `processing`, `generating_proxy`, `imported`, `done`, `error`;
- `thumb_status`: `pending`, `processing`, `ready`, `no_card_thumb`, `error`;
- serija: `preparing`, `filmstrip`, `waveform`, `done`;
- `status`: `on_source`, `linked`, `ready`.

Naša content baza (`clips` u `qnc_project.db`, vlasnik `qnc-ingest-store`) ne prati tu shemu:
nema `ingest_jobs`, serija, `thumb_status`, `status`, zasebnih putanja proxy/original ni
`virtual_shots`. Izmjene koje sam dodao (stupac `import_claimed_at`, operacija `Heartbeat`,
`thumbnail_uri` u `FinishImport`) **nisu izvedene iz v5** i treba ih preispitati.

## E. Poznata razlika u ponašanju odabira (uzrok prijave "checkbox ne radi")

v5: klik → odmah `selected` lokalno → spremi cijeli odabir (revizija) → greška vraća staro.
Naš kod: klik → zapis u bazu u pozadini → tek onda `selected` u prikazu; ako zapis ne uspije,
vidljivo je samo u poruci u podnožju. Uzrok kvara u živoj aplikaciji nije potvrđen: upis u bazu
(kopija projektne baze), `SelectionWriter` i aplikacijski testovi rade.
