# Ingest public modules and components

Status: ugovorni popis prije implementacije  
Datum: 2026-09-05  
Root: `C:\Users\miron\Projects\QNC`  
Referenca: `C:\Users\miron\Projects\qnc_v4`

Ovaj dokument razdvaja Ingest UI, javne module i javne komponente. Ingest je
aplikacija/forma. Moduli i komponente su gradivni dijelovi. UI ne smije nositi
aktivni kod.

## Osnovno pravilo

```text
UI forma
  -> crta 100% v4 layout
  -> prikazuje view model
  -> skuplja korisnicki input
  -> emitira action_id + payload
  -> ne cita DB, ne pise DB, ne otvara filesystem, ne scan, ne probe

Javne komponente Ingest aplikacije
  -> primaju action_id
  -> izvode Ingest workflow
  -> zovu javne module
  -> pisu samo Ingest DB
  -> iz DB-a grade view model za UI

Javni moduli
  -> imaju public capability contract
  -> nisu vlasnici Ingest workflowa
  -> ne znaju tko ih koristi
  -> imaju samo dependency zabrane, ne listu dopustenih aplikacija
```

UI smije imati samo lokalno prezentacijsko stanje koje ne mijenja workflow:
hover, focus, scroll poziciju, velicinu panela i trenutni tekst inputa prije
slanja intenta. Sve ostalo pripada komponenti ili modulu.

## Javni moduli iz Ingesta

| Modul | Izvor iz v4 / QNC | Capability | Ulaz | Izlaz | Boundary |
| --- | --- | --- | --- | --- | --- |
| `qnc-ui-kit` | `qnc_theme`, `qnc_ui` | `ui.paint.primitives` | layout contract + view data | nacrtani chrome/widgeti | nema DB, nema filesystem, nema workflow |
| `qnc-editorial-shell` | `qnc_ui::editorial_shell`, `composition` | `ui.editorial_shell.paint` | split/layout model | lijevi/desni shell + dock prostor | ne zna Ingest/Story/MA workflow |
| `qnc-preview-panel` | `qnc_ui::preview`, monitor placeholder | `ui.preview.paint` | texture/status label | preview surface | ne decode, ne probe, ne player owner |
| `qnc-location-browser-ui` | `qnc_location_browser` | `ui.location_browser.paint` | listing snapshot | browser action intent | ne lista filesystem sam |
| `qnc-dir-browser` | v4 `FilesystemListComponent`, novi crate | `dir.list`, `dir.select` | QNC location URI | listing + selected QNC URI | nema media scan, nema probe |
| `qnc-media-browser-ui` | `editorial/media_pool`, `qnc_media_card` | `ui.media_grid.paint` | clip view rows + textures | focus/select intent | ne cita DB, ne probe |
| `qnc-media-card` | `qnc_media_card` | `ui.media_card.paint` | media card row | card paint + hit zones | status se samo prikazuje |
| `qnc-source-dock-ui` | `qnc_source_dock` | `ui.source_dock.paint` | timeline/view model | dock intents | nema import/probe/generator rada |
| `qnc-timeline-progress` | `qnc_timeline_progress`, `qnc_timeline` | `timeline.paint`, `timeline.hit_test` | frame model | scrub/cue intent | nema DB write, nema playback clock owner |
| `qnc-filmstrip-paint` | `qnc_filmstrip_background` | `ui.filmstrip.paint` | frame textures iz artefakta | nacrtani filmstrip | ne generira frameove |
| `qnc-image-assets` | `media_assets` | `asset.texture.load` | artifact/thumb URI | texture handle/result | ne bira clip, ne pise DB |
| `qnc-keyboard-shortcut` | `seed/keyboard-shortcuts.json`, QNC contract | `keyboard.shortcuts.resolve` | key event + scope | action_id | nema hardkodiranih akorda u UI |
| `qnc-transport-resolver` | QNC resolver contract | `qnc.uri.resolve` | QNC URI | private handle/endpoint | nije izvor istine |
| `qnc-db-contract` | DB contracts | `db.schema.validate` | DB URI + schema | validation result | ne pise poslovne podatke |
| `qnc-frame-timebase` | `frame_time` | `frame.convert`, `timecode.format` | fps/timebase/frame | frame/seconds/timecode | ne ffprobe |
| `qnc-source-scanner` | `qnc-host/src/ingest/scanner.rs`, store discover | `source.scan.roles` | source QNC URI | role map original/proxy/support | proxy nije clip, nema probe |
| `qnc-camera-detector` | camera/source rules | `source.camera.detect` | source facts/sidecars | card/source identity + role hints | nema import, nema probe |
| `qnc-media-probe` | `ingest_probe`, `record_media_probe_result` | `media.probe.full` | original URI + proxy URI ako postoji | puni probe record | ne zove filmstrip/wave/player/export |
| `qnc-poster` | `thumb`, `ingest_posters` | `poster.copy_or_generate` | clip + card thumb/proxy policy | poster artifact result | ne smije biti filmstrip zamjena |
| `qnc-filmstrip` | `filmstrip` | `filmstrip.generate14` | clip id + DB probe/source refs | 14 stvarnih frameova | ne ffprobe, ne scan, ne poster repeat |
| `qnc-wave` | `waveform` | `wave.generate` | clip id + DB probe/audio refs | waveform peaks/artifact | ne ffprobe, ne scan |
| `qnc-broadcast-player` | playback stack / player contract | `playback.open`, `playback.seek`, `playback.status` | media ref + DB probe/timebase | playback status/frame | ne ffprobe, ne Ingest workflow |
| `qnc-import-transfer` | `import_pipeline`, transport prepare/link/copy | `media.transfer.prepare` | selected clip refs + policy | copied/linked artifacts/status | ne scan, ne probe |
| `qnc-job-runner` | v4 ingest jobs/orchestrator | `job.run.batch` | owner job request | job events/status | ne zna poslovnu logiku svih aplikacija |

Napomena: modul je javan po capabilityju. Ne smije imati hardkodiranu listu
aplikacija koje ga smiju koristiti. Zabrane se upisuju kao dependency boundary
modula i kao workflow zabrane aplikacija koje ga ne smiju koristiti.

## Javne komponente Ingest aplikacije

Ovo su komponente koje nova Ingest aplikacija treba javno izloziti svojoj
formi i shell adapteru. One nisu UI widgeti i nisu privatni pozivi druge
aplikacije.

| Komponenta | Odgovornost | Smije zvati | Pise |
| --- | --- | --- | --- |
| `IngestDesktopAdapter` | shell-hosted ulaz: `create` + `show_desktop` | `IngestApplicationComponent` | nista direktno |
| `IngestApplicationComponent` | lifecycle standalone/shell, batch exit | session/status komponente | nista direktno osim preko store ownera |
| `IngestActionDispatcher` | mapira `action_id` + payload u komponentnu naredbu | sve Ingest komponente | nista direktno |
| `IngestViewModelComponent` | gradi pasivni view model iz Ingest DB public viewova | `qnc-ingest-store` read API | nista |
| `IngestStatusComponent` | status, progress, pending/imported count, errors | store read API | privatni status ako treba |
| `IngestSourceBrowserComponent` | source kind, list/open/confirm/cancel | `qnc-dir-browser`, resolver | `ingest_registry.source_locations`, `source_sessions` |
| `IngestSourceIdentityComponent` | serial number, volume/source name, first/last seen | `qnc-camera-detector` | `ingest_registry.source_cards` |
| `IngestSourceDiscoveryComponent` | Odaberi -> scan roles + original/proxy grouping | scanner, camera detector | `ingest_content.clips`, `clip_sources`, `clip_proxy` |
| `IngestProbeComponent` | jedini full probe prolaz | `qnc-media-probe` | `ingest_content.probe_records` |
| `IngestSelectionComponent` | select/toggle/select all/clear + revision guard | store | Ingest selection fields |
| `IngestOptionsComponent` | `Kopiraj original`, `AI mining`, dodatne checkbox opcije | store | Ingest options/status |
| `IngestImportBatchComponent` | `Uvezi` selektirane, batch faze, batch finish | transfer/job runner | privatni job/batch store + javni status |
| `IngestArtifactComponent` | opcionalni poster/filmstrip/wave artefakti | poster, filmstrip, wave | artifact tablice/status |
| `IngestPreviewComponent` | preview focus, cue, play/pause intent | broadcast player, resolver | samo status/cache koji je Ingest-owned |
| `IngestDbOwnerComponent` | schema, migrations, short transactions, public views | DB contract/resolver | samo Ingest DB |
| `IngestReadApiComponent` | stabilni read model za druge aplikacije | Ingest DB public views | nista |

## Ingest action_id katalog

Svaka korisnicka akcija iz forme mora izaci kao `action_id`. Klik i shortcut
koriste isti action_id; shortcut akordi dolaze iz
`contracts/qnc-keyboard-shortcuts.json`.

| UI akcija | action_id | Komponenta |
| --- | --- | --- |
| Računalo / LAN / Internet | `ingest_source_kind_*` | `IngestSourceBrowserComponent` |
| Gore | `ingest_dir_up` | `IngestSourceBrowserComponent` |
| Diskovi | `ingest_dir_roots` | `IngestSourceBrowserComponent` |
| Otvori mapu/stavku | `ingest_dir_open` | `IngestSourceBrowserComponent` |
| Odaberi source | `ingest_dir_confirm` | `IngestSourceDiscoveryComponent` |
| Odustani | `ingest_dir_cancel` | `IngestSourceBrowserComponent` |
| Klik kartice | `ingest_preview_focus` | `IngestPreviewComponent` |
| Check na kartici | `ingest_clip_toggle` | `IngestSelectionComponent` |
| Odaberi sve | `ingest_select_all` | `IngestSelectionComponent` |
| Očisti | `ingest_clear_selection` | `IngestSelectionComponent` |
| Uvezi | `ingest_import_selected` | `IngestImportBatchComponent` |
| Osvježi | `ingest_reload` | `IngestViewModelComponent` |
| Kopiraj original | `ingest_set_archive` | `IngestOptionsComponent` |
| AI mining | `ingest_set_ai_mining` | `IngestOptionsComponent` |
| Generiraj postere | `ingest_approve_proxy_posters` | `IngestArtifactComponent` |
| Play/Pause | `play_pause` | `IngestPreviewComponent` |
| `[` / `]` | `step_back_frame` / `step_forward_frame` | `IngestPreviewComponent` |
| Scrub timeline | `ingest_cue_frame` | `IngestPreviewComponent` |
| Expand A1/A2 | `ingest_toggle_audio_lane` | `IngestPreviewComponent` |

Ako action_id ne postoji u shortcut/action contractu, ne smije se kodirati UI
akcija koja ga koristi.

## Sto UI crate ne smije imati

Ingest UI/forma crate ne smije importati niti koristiti:

```text
std::fs
rusqlite
std::process::Command
ffmpeg / ffprobe wrapper
scanner module direct API
media probe direct API
filmstrip generator direct API
wave generator direct API
broadcast player direct runtime
project store direct API
story/media-assist private API
raw OS path join kao javni identitet
```

Ako se u UI crateu pojavi neka od ovih ovisnosti, to je conformance greska.

## Minimalni tok

```text
1. UI: korisnik klikne Odaberi
2. UI: emitira action_id = ingest_dir_confirm
3. IngestActionDispatcher: prima intent
4. IngestSourceDiscoveryComponent:
   - trazi QNC URI od IngestSourceBrowserComponent
   - poziva scanner + camera detector
   - pise source/card/clip/proxy u Ingest DB
5. IngestProbeComponent:
   - pokrece jedini media.probe.full prolaz
   - pise probe_records
6. IngestViewModelComponent:
   - cita public views
   - vraca view model formi
7. UI: samo prikazuje nove rows/status
```

Filmstrip, wave, poster i copy original su dodatne radnje. Ako su ukljucene,
Ingest ih pokrece preko komponenti i javnih modula. Ako nisu ukljucene, ne rade
se. Nijedna od tih radnji ne smije ponovno pokrenuti probe.

## Sto je provjereno

- `AGENTS.md` pravila za UI bez aktivnog koda, DB-first i module kao javno
  dobro.
- `docs/11-ingest-ui-reference-audit.md` i `docs/12-ingest-recoding-proposal.md`.
- v4 Ingest UI izvori: `ingest/mod.rs`, `qnc_location_browser.rs`,
  `qnc_source_dock.rs`, `editorial/media_pool.rs`, `qnc_media_card.rs`.
- v4 Ingest component izvori: `source_import_command`, `source_import_state`,
  `source_import_selection`, `source_import_status`, `filesystem_list`.
- v4 Ingest host izvori: `ingest/db.rs`, `ingest/store.rs`, `ingest/scanner.rs`,
  `ingest/import_pipeline.rs`, `ingest/orchestrator.rs`.
- Novi QNC trenutno ima samo Ingest ugovore, ne jos Ingest aplikacijske crateove.

## Sto nije provjereno

- Nije radjen live test jer se Ingest jos ne implementira.
- Nije provjeren stvarni LAN/Intranet source registry.
- Nije zakljucan puni `probe_records` field set za player/filmstrip/wave/export.
- Nije dodan conformance test koji automatski brani aktivne dependencyje u UI
  crateu.

## Sljedeci rizik

Najveci rizik je da se v4 `IngestScreen` prekopira kao nova forma i tako u UI
ponovno unese polling, playback, command queue, scan/probe trigger i host API.
Ispravan prvi kodni korak je zato izdvajanje pasivne forme + action dispatcher,
uz conformance test da UI crate nema aktivne ovisnosti.
