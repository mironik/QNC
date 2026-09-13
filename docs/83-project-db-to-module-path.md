# Jedini put: projektna baza -> moduli

Datum: 2026-09-12. Ovo nije novi zakon. To je zakljucani slijed iz
`AGENTS.md` §4.1 i §8.2. Ako plan ili agent krene drugdje, plan je kriv.

Project kod se ne mijenja (§14). Project aplikacija ne mora biti pokrenuta.
Dovoljan je valjan DB zapis.

## Lanac (ne preskakati)

```text
project_registry
  public_app_settings.active_project_id
       |
       v
  public_projects.project_uri          (javni identitet)
  project_storage_locations.local_path (samo adapter binding, nije API)
       |
       v
qnc_project.db aktivnog projekta
  public_project_settings.settings_json
       |
       v
qnc-work-settings::SettingsReader     query_only
  WorkSettings
    workspace_db_uri
    playback.input
    audio.channels / audio.sample_rate
    storage raspored (za import/artefact, ne za sat)
       |
       +--> qnc-ingest-work-plan::from_settings
       |      standardni raspored unutar lokacije iz baze
       |
       +--> ingest content read port (clip_id)
       |      spremljeni snapshot: timebase, duration, streamovi,
       |      original_uri / proxy_uri
       |
       v
qnc-player-input::InputReader.load(workspace_db_uri, clip_id)
  PreparedInput
    playback.input          -> izbor slike original|proxy
    project_audio           -> izlaz iz settings_json
    snapshot timebase       -> sat; NIJE video.fps
    original audio identity -> ne proxy audio kao zamjena
       |
       v
qnc-player-launcher  (QNC URI -> binding)
       |
       v
qnc-broadcast-player / qnc-broadcast-engine
  primjenjuje PreparedInput
  ne cita Project, ne zove Ingest workflow, ne radi probe
```

Forma nije u lancu. Forma salje `action_id`. Shell ne nosi postavke.

Ingest `prepare_preview` samo prosljedjuje vec ucitani `SettingsReader`,
`workspace_db_uri` iz work plana i content read port. Ne sastavlja putanje.

## Sto koji zapis znaci

| Izvor | Polje | Tko smije koristiti | Tko ne smije |
| --- | --- | --- | --- |
| registry | `active_project_id` | work-settings | forma, engine, Codex “odaberi projekt” |
| settings_json | `playback.input` | player-input | engine kao hardkod |
| settings_json | `audio.channels`, `audio.sample_rate` | player-input -> engine izlaz | izmišljen 2.0 / 48000 |
| settings_json | `video.fps` | validate snapshota; nije playback sat | engine cadence |
| settings_json | `storage.*` | import / artifact layout | player izvor |
| clip snapshot | timebase, duration, streams | player, filmstrip, wave | novi probe |
| clip snapshot | original/proxy URI | player-input izbor slike | forma path-join |

## Gdje se Codex gubi (zabranjeno)

- Krenuti od `qnc-project` cratea, Project storea ili “otvori Project pa pošalji”
- Krenuti od forme, shell taba ili UI statea kao izvora projekta
- Otvoriti `qnc_project.db` path-joinom kao javni korak
- Uvesti `data/player-output.json` ili drugi lokalni JSON umjesto `audio.*`
- Uzeti `video.fps` / export FPS kao source sat
- Novi probe jer “nedostaje u modelu” — prvo provjeriti snapshot u bazi
- HTTP/URL u FFmpeg CLI
- Default kad `active_project_id` ili `settings_json` fali
- Nova Project polja prije potvrde da zapis vec ne postoji
- Citanje docs/65 kao pravila: tamo je povucen zakljucak o `player-output.json`

Ako korak fali, stati s kontroliranom greskom. Ne nadomjestati.

## Filmstrip / Wave

Isti koraci 1–5. Worker ne cita Project crate. Cita content port + artifact
direktorij izveden iz work plana / standardnog rasporeda. Write ide kroz
javni content writer, ne kroz formu.
