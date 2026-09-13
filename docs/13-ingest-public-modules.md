# Ingest public modules

Status: trenutni runtime rez + ugovorni popis sljedecih modula  
Datum: 2026-09-05  
Root: `C:\Users\miron\Projects\QNC`  
Referenca: `C:\Users\miron\Projects\qnc_v4`

Ovaj dokument razdvaja Ingest UI i javne module. Ingest je aplikacija/forma.
Moduli su gradivni dijelovi. UI ne smije nositi aktivni kod.

## Osnovno pravilo

```text
UI forma
  -> crta 100% v4 layout
  -> prikazuje view model
  -> skuplja korisnicki input
  -> emitira action_id + payload
  -> ne cita DB, ne pise DB, ne otvara filesystem, ne scan, ne probe

Ingest aplikacijski sloj
  -> composition/root za javne module
  -> prima action_id i prosljedjuje ga uskom javnom modulu
  -> ne smije postati `qnc-ingest-components`
  -> ne smije u sebi skupljati browser, select, player, filmstrip, wave i DB

Javni moduli
  -> imaju public capability contract
  -> nisu vlasnici Ingest workflowa
  -> ne znaju tko ih koristi
  -> imaju samo dependency zabrane, ne listu dopustenih aplikacija
```

UI smije imati samo lokalno prezentacijsko stanje koje ne mijenja workflow:
hover, focus, scroll poziciju, velicinu panela i trenutni tekst inputa prije
slanja intenta. Sve ostalo pripada komponenti ili modulu.

Ingest ne zna za Project aplikaciju i ne smije imati dependency na Project app,
desktop adapter, component, store, action_id ili workflow. Ingest ne dobiva
workspace, runtime context za poslovno stanje, alat za suradnju ni nasljedeni
state. Ingest cita bazu, pronalazi koja projektna baza ima oznaku aktivnog
projekta i iz te baze cita samo postavke za rad read-only. Ingest nema rucno
postavljanje projektnih radnih postavki. Ne pise, ne migrira, ne popravlja i ne
kreira projekt.

QNC baza mora biti prenosiva. Ingest mora moci raditi na racunalu na kojem ne
postoje Project aplikacija, QNC.app shell ni drugi QNC aplikacijski procesi, ako
mu je dostupna valjana baza s oznakom aktivnog projekta i postavkama za rad.

## Trenutni runtime rez

Trenutno postoji standalone `qnc-ingest` aplikacija i shell-hosted Ingest
adapter. Ovaj rez je samo source browser + `source.select` +
registry/session DB zapis.

`Odaberi` jos nije puni Ingest. Puni Ingest pocinje tek kad postoje i budu
spojeni javni moduli za scan, camera detect, original/proxy grouping, jedini
probe prolaz i clip/probe DB upis.

Zbog toga trenutni `contracts/applications/ingest.application.json` ne smije
deklarirati scanner, camera detector, Media Probe, Media Browser, Filmstrip ili
Wave kao runtime dependency dok ti moduli ne postoje kao stvarni runtime crate
ili out-of-process adapter.

## Javni moduli iz Ingesta

| Modul | Izvor iz v4 / QNC | Capability | Ulaz | Izlaz | Boundary |
| --- | --- | --- | --- | --- | --- |
| `qnc-ui-kit` | `qnc_theme`, `qnc_ui` | `ui.paint.primitives`, `ui.form_action_bar`, `ui.exclusive_panel` | layout contract + view data + akcijski labeli + pasivni UI open/close state | nacrtani chrome/widgeti, standardna potvrdna akcijska traka, genericki UI-state rezultat | nema DB, nema filesystem, nema workflow, nema browser session state |
| `qnc-editorial-shell` | `qnc_ui::editorial_shell`, `composition` | `ui.editorial_shell.paint` | split/layout model | lijevi/desni shell + dock prostor | ne zna Ingest/Story/MA workflow |
| `qnc-preview-panel` | `qnc_ui::preview`, monitor placeholder | `ui.preview.paint` | texture/status label | preview surface | ne decode, ne probe, ne player owner |
| `qnc-location-browser-ui` | `qnc_location_browser` | `ui.location_browser.paint` | browser state snapshot | source/gore/diskovi/breadcrumb/row intent | ne lista filesystem, ne zna OS path, ne posjeduje potvrdu/odustajanje |
| `qnc-dir-browser` | v4 `FilesystemListComponent`, novi crate | `dir.list`, `dir.select` | QNC location URI / session request | OS-neutral listing + selected QNC URI | samostalni javni modul; nema egui dugmad, nema media scan, nema probe |
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
| `qnc-filmstrip` | `filmstrip` | `filmstrip.generate` | clip id + DB probe/source refs | stvarni filmstrip frameovi | ne ffprobe, ne scan, ne poster repeat |
| `qnc-wave` | `waveform` | `wave.generate` | clip id + DB probe/audio refs | waveform peaks/artifact | ne ffprobe, ne scan |
| `qnc-broadcast-player` | playback stack / player contract | `playback.open`, `playback.seek`, `playback.status` | media ref + DB probe/timebase | playback status/frame | ne ffprobe, ne Ingest workflow |
| `qnc-import-transfer` | `import_pipeline`, transport prepare/link/copy | `media.transfer.prepare` | selected clip refs + policy | copied/linked artifacts/status | ne scan, ne probe |
| `qnc-job-runner` | v4 ingest jobs/orchestrator | `job.run.batch` | owner job request | job events/status | ne zna poslovnu logiku svih aplikacija |

Napomena: modul je javan po capabilityju. Ne smije imati hardkodiranu listu
aplikacija koje ga smiju koristiti. Zabrane se upisuju kao dependency boundary
modula i kao workflow zabrane aplikacija koje ga ne smiju koristiti.

## Zabranjeni umbrella sloj

`qnc-ingest-components` ne smije postojati kao javni runtime sloj. Ingest forma
smije prikazivati layout i slati `action_id`, a Ingest aplikacijski sloj smije
biti samo composition/root koji povezuje uske javne module. Svaka aktivna
odgovornost mora imati vlastiti uski modul ili adapter.

| Javni modul/adapter | Odgovornost | Smije zvati | Pise |
| --- | --- | --- | --- |
| `qnc-ingest-desktop-adapter` | shell-hosted ulaz: `create` + `show_desktop` prema jedinom `qnc-app.exe` desktopu | Ingest aplikacijski root | nista direktno |
| `qnc-ingest-application` | privremeni composition/root dok se ne izdvoje svi uski moduli | javne module po action_id-u | nista direktno osim kroz javne write adaptere |
| `qnc-work-settings` | cita oznaku aktivnog projekta i postavke za rad iz baze read-only | DB contract/resolver | nista |
| `qnc-ingest-catalog` | gradi pasivni view model iz Ingest DB public viewova | `qnc-ingest-store` read API | nista |
| `qnc-ingest-store` | Ingest DB read/write transport i public views | DB contract/resolver | samo Ingest DB |
| `qnc-dir-browser` | source kind, list/open/confirm/cancel state | resolver/source reader | `ingest_registry.source_locations`, `source_sessions` samo kroz owner ugovor |
| `qnc-source-reader` | OS-neutral source listing | resolver | nista |
| `qnc-camera-detector` | serial number, volume/source name, source identity | source facts/sidecars | nista |
| `qnc-scanner` | source roles + original/proxy grouping | camera detector/source reader | nista |
| `qnc-media-probe` | jedini full probe prolaz | decoder/probe adapter | nista direktno |
| `qnc-ingest-select` | Odaberi -> katalog/probe/source zapis | scanner, camera detector, media probe, store transport | `ingest_content` kroz javni DB writer |
| `qnc-media-thumbnail` | poster/thumbnail ucitavanje ili priprema | image assets/source refs | nista direktno |
| `qnc-filmstrip-worker` | automatsko kreiranje filmstrip JPEG artefakata | filmstrip + decoder catalog + DB read | nista direktno, publish ide kroz transport |
| `qnc-timeline-assets` | pasivno cita filmstrip/wave artefakte za timeline | Ingest DB read + artifact reader | nista |
| `qnc-wave-worker` | kreiranje wave peak zapisa | wave + decoder catalog + DB read | nista direktno, publish ide kroz transport |
| `qnc-wave-view` | pasivni wave paint podaci | wave peaks | nista |
| `qnc-monitor` | pasivni player preview surface | potvrdjeni frame ili poruka | nista |
| `qnc-player-launcher` / `qnc-player-client` | preview focus, cue, play/pause intent prema Broadcast Playeru | player contract/resolver | samo player session state |

## Ingest action_id katalog

Dir Browser nije Ingest forma i ne nosi potvrdna/odustajna dugmad. Ingest
aplikacijski root koristi javni `qnc-dir-browser` session/state, browser UI modul
prikazuje snapshot i navigacijske hit-zone, a aplikacijska akcijska traka
prikazuje `Odaberi`/`Odustani` kroz `qnc-ui-kit` i salje odgovarajuci
`action_id`. Ingest u ovom rezu ima jedan source browser, ali mora koristiti
isti standardni obrazac kao Project: browser view nema confirm/cancel dugmad,
a standardna potvrdna traka je javni UI-kit obrazac. Ako se isti browser koristi
u drugoj aplikaciji, ne smije se kopirati Ingest workflow niti hardkodirati
korisnik modula.

Svaka korisnicka akcija iz forme mora izaci kao `action_id`. Klik i shortcut
koriste isti action_id; shortcut akordi dolaze iz
`contracts/qnc-keyboard-shortcuts.json`.

| UI akcija | action_id | Javni modul |
| --- | --- | --- |
| Računalo / LAN / Internet | `ingest_source_kind_*` | `qnc-dir-browser` |
| Gore | `ingest_dir_up` | `qnc-dir-browser` |
| Diskovi | `ingest_dir_roots` | `qnc-dir-browser` |
| Otvori mapu/stavku | `ingest_dir_open` | `qnc-dir-browser` |
| Odaberi source | `ingest_dir_confirm` | `qnc-ingest-select` |
| Odustani | `ingest_dir_cancel` | `qnc-dir-browser` |
| Klik kartice | `ingest_preview_focus` | `qnc-player-client` / `qnc-timeline-assets` |
| Check na kartici | `ingest_clip_toggle` | `qnc-ingest-store` |
| Odaberi sve | `ingest_select_all` | `qnc-ingest-store` |
| Očisti | `ingest_clear_selection` | `qnc-ingest-store` |
| Uvezi | `ingest_import_selected` | `qnc-import-transfer` |
| Osvježi | `ingest_reload` | `qnc-ingest-catalog` |
| Kopiraj original | `ingest_set_archive` | `qnc-ingest-store` |
| AI mining | `ingest_set_ai_mining` | `qnc-ingest-store` |
| Generiraj postere | `ingest_approve_proxy_posters` | `qnc-media-thumbnail` |
| Play/Pause | `play_pause` | `qnc-player-client` |
| `[` / `]` | `step_back_frame` / `step_forward_frame` | `qnc-player-client` |
| Scrub timeline | `ingest_cue_frame` | `qnc-player-client` preko `qnc-timeline` intenta |
| Expand A1/A2 | `ingest_toggle_audio_lane` | `qnc-timeline` |

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
3. Ingest aplikacijski root: prima intent i prosljedjuje ga uskom modulu
4. qnc-ingest-select:
   - trazi QNC URI od qnc-dir-browser sessiona
   - poziva scanner + camera detector
   - pise source/card/clip/proxy u Ingest DB
5. qnc-media-probe:
   - pokrece jedini media.probe.full prolaz
   - pise probe_records
6. qnc-ingest-catalog:
   - cita public views
   - vraca view model formi
7. UI: samo prikazuje nove rows/status
```

Filmstrip, wave, poster i copy original su dodatne radnje. Ako su ukljucene,
Ingest ih pokrece preko javnih modula. Ako nisu ukljucene, ne rade se. Nijedna
od tih radnji ne smije ponovno pokrenuti probe.

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
