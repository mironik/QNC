# qnc_v4 application/module audit

Status: prvi korektni presjek po novoj klasifikaciji  
Datum: 2026-09-04  
Izvor: `C:\Users\miron\Projects\qnc_v4`

## Sto je provjereno

Provjereni izvori:

- `C:\Users\miron\Projects\qnc_v4\seed\tabs\*\plugin.json`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\main.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\app.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\project\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\ingest\mod.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\media_assist.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-app\src\story.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\main.rs`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\project\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\ingest\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\story\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\filmstrip\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\waveform\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\media_pool\*`
- `C:\Users\miron\Projects\qnc_v4\qnc-host\src\virtual_shots\*`

## Potvrdene aplikacije/forme u `qnc_v4`

Seed plugini potvrduju cetiri aplikacije/forme:

| Aplikacija/forma | Seed plugin | UI izvor | Host izvor | Napomena |
| --- | --- | --- | --- | --- |
| Project | `seed/tabs/project/plugin.json` | `qnc-app/src/project/*` | `qnc-host/src/project/*` | project list, templates, settings, workspace |
| Ingest | `seed/tabs/ingest/plugin.json` | `qnc-app/src/ingest/mod.rs` | `qnc-host/src/ingest/*` | source, browse, selection, discover/import |
| Media Assist | `seed/tabs/media_assist/plugin.json` | `qnc-app/src/media_assist.rs` | koristi `/api/story` u starom kodu | valjana forma/template; dijeljeni code path nije sam po sebi greska |
| Story | `seed/tabs/story/plugin.json` | `qnc-app/src/story.rs`, `qnc-app/src/story/*` | `qnc-host/src/story/*` | story parts, markers, covers, playlist |

Zakljucak: Filmstrip, Wave, Broadcast Player, Export, Media Probe, Media
Browser, Timeline, Monitor i Dir Browser nisu aplikacije/forme u ovom modelu.
To su moduli koje aplikacije koriste.

## Moduli nadeni u `qnc_v4`

| Modul | Lokacije | Trenutna uloga u `qnc_v4` | Novi status |
| --- | --- | --- | --- |
| Filmstrip | `qnc-host/src/filmstrip/*`, `qnc-app/src/qnc_filmstrip_background.rs` | store, frame rows, scheduler, UI paint support | shared modul; ne aplikacija; ne probe |
| Wave | `qnc-host/src/waveform/*`, `qnc-app/src/editorial/program_waveform.rs` | audio waveform store/scheduler/UI | shared modul; ne aplikacija; ne probe |
| Media Probe | `qnc-host/src/ingest_probe/*`, `qnc-host/src/ingest/store.rs`, `qnc-host/src/ingest/thumb.rs` | probe jobs i upis probe rezultata u ingest DB | javni modul; nema popis dozvoljenih korisnika; aplikacijski workflow odreduje smije li se probe pokrenuti |
| Dir Browser | `qnc-app/src/qnc_location_browser.rs`, `qnc-host/src/shell_fs.rs`, `qnc-host/src/shell_dialog.rs` | izbor foldera/rootova/file list | shared UI/transport modul, bez ingest logike |
| Media Browser | `qnc-app/src/editorial/media_pool.rs`, `qnc-host/src/media_pool/*` | prikaz/imported media pool i ingest-backed reads | shared modul; cita baze, ne radi probe |
| Broadcast Player | `qnc-app/src/qnc_broadcast_player.rs`, `playback_stack.rs`, `player_*`, `qnc-broadcast-player`, `qnc-player-*` | playback runtime, player bridge, frame transport/output | runtime modul koji aplikacija koristi; ne aplikacija |
| Export | `qnc-host/src/export_hires.rs`, `export_playlist.rs`, `services/export_process.rs`, Story export rute | export/render iz Story rezultata | modul; ne aplikacija; ne probe fallback |
| Timeline | `qnc-app/src/qnc_timeline.rs`, `qnc_segment_timeline.rs`, `qnc_timeline_progress.rs`, `qnc-host/src/timeline_model.rs`, `story/timeline_model.rs` | timeline model/painter/progress | shared modul; pasivan model/painter |
| Monitor | `qnc-player-monitor*`, `qnc-player-workstation-monitor`, `hardware_profile/probe.rs` | output/workstation/hardware status | shared runtime/status modul |
| Frame/timebase | `qnc-app/src/frame_time.rs`, `qnc-host/src/frame_time.rs`, service contracts | frame/sec/timecode/FPS math | prvi neutralni shared modul |
| Manifest/modules registry | `qnc-host/src/modules`, `seed/tabs/*/plugin.json`, `/api/modules` | tab/module registry | contract modul; ne workflow owner |

## Baze i tablice nadene u `qnc_v4`

Project podrucje:

- `projects`
- `app_settings`
- `users`
- `sessions`
- `source_templates`
- `project_templates`
- `module_state`
- `project_template_kv`
- `project_template_sources`
- `source_template_kv`
- `project_settings`
- `project_members`
- `project_template_snapshot`
- `project_workflow_steps`
- `project_workflow_state`
- `project_data_revisions`
- `project_settings_kv`
- `project_snapshot_kv`
- `project_workflow_step_kv`

Ingest podrucje:

- `ingest_meta`
- `ingest_assets`
- `ingest_jobs`
- `ingest_import_batches`
- `ingest_import_batch_items`
- `playback_cache`

Story podrucje:

- `story_state`
- `story_parts`
- `story_markers`
- `story_marker_slots`
- `story_covers`
- `story_object_history`
- `virtual_shots`

Artefakt moduli:

- `filmstrips`
- `filmstrip_frames`
- `audio_waveforms`

## Vazni nalazi za novi dizajn

1. `qnc_v4` ima jedan centralni `qnc-host` koji spaja Project, Ingest, Story,
   media, render, jobs, filmstrip i waveform rute. To je korisno za audit, ali
   se ne smije kopirati kao nova arhitektura.
2. `qnc_v4` UI `QncApp` drzi Project, Ingest, Media Assist i Story u jednom
   procesu. Za novi QNC to je vizualni i funkcionalni izvor, ali ne dokaz da
   sve treba ostati monolit.
3. Media Assist u starom kodu koristi `StoryScreen`:
   `qnc-app/src/media_assist.rs` re-exporta `StoryScreen` kao
   `MediaAssistScreen`. To je prihvatljivo kao dokaz da zajednicki neutralni UI
   modul moze koristiti vise formi.
4. Stari Media Assist plugin koristi `/api/story`. U novom modelu to ne smije
   znaciti da Media Assist i Story dijele istu aktivnu aplikaciju ili istu
   privatnu bazu mimo javnog contracta.
5. Ingest u starom kodu vec pokazuje jasne module: location browser, media
   browser, media probe, poster/filmstrip/wave/proxy/audio wrap.
6. Story u starom kodu vec pokazuje jasne module: story state DB, parts edit,
   markers, marker slots, covers, timeline model, editorial playlist, playback
   adapter, export adapter.
7. Filmstrip i Wave imaju vlastite store/worker dijelove, ali u novom modelu su
   moduli, ne aplikacije. Njihov rezultat mora biti baza/artefakt koji cita
   aplikacija.
8. Media Probe je javni modul, ali nije javni fallback za kasnije aplikacije.
   Probe workflow je dio Ingest procesa. Story, Player, Export, Filmstrip i
   Wave imaju workflow/dependency zabrane koje im zabranjuju novi probe.

## Sto koristiti iz `qnc_v4`

Moze se koristiti kao polaziste:

- native egui vizualni stil
- svi postojeci UI i layout obrasci kao obvezna doslovna preslika
- Shell UI i tab/form layout
- Project UI kompozicija
- Ingest UI kompozicija i tok izbora sourcea
- Media Assist kao zasebna forma/template koja moze dijeliti neutralne module
- Story/editorial UI layout, fokus i panel ponasanje
- media browser, timeline, player kontrole, filmstrip/wave prikaz
- Story/editorial neutralni moduli
- frame/timebase math
- timeline model/painter ideje
- filmstrip/wave store ideje
- probe zapis podataka u Ingest DB
- seed plugin manifest ideja

Ne smije se kopirati kao nova arhitektura:

- jedan centralni host kao vlasnik svih workflowa
- direktni Story/Media Assist shared aktivni state mimo baze
- Story API kao vlasnik export/player runtimea
- Filmstrip/Wave/Player/Export kao aplikacije
- probe fallback izvan Ingest procesa
- raw OS path kao javni contract

## Sljedeci audit korak

Sljedeci korak je razbiti svaki modul na contract:

```text
modul -> public capabilities -> input -> output -> state/write policy -> forbidden calls
```

Pocetna matrica je zapisana u:

`C:\Users\miron\Projects\QNC\docs\04-module-contract-matrix.md`

Prioritet:

1. manifest/capability
2. transport/resolver
3. DB contract/validation
4. frame/timebase
5. Dir Browser
6. Media Probe
7. Media Browser
8. Filmstrip
9. Wave
10. Timeline
11. Broadcast Player
12. Export
