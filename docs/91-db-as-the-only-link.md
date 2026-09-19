# 91 — Baza kao jedina poveznica procedure (lanac aplikacija)

Ispravak pristupa iz dokumenta 90: prvo je **lanac** (što koja aplikacija čita iz baze, što u nju i u direktorije piše, što time omogućuje sljedećoj), tek onda nalazi o kvaliteti. Izvor: QNC v5 (`qnc-host/src`), samo čitano. „(zaključak)” = nije doslovno u kodu.

## 1. Načelo

- Jedna projektna baza (`projects/<project_id>/qnc_project.db`) je jedina istina i jedina veza među aplikacijama. Aplikacije se ne zovu međusobno.
- Svaka aplikacija: **čita** ulaz koji je prethodnik zapisao, **piše** samo vlastiti rezultat (redovi u bazi + datoteke u direktorijima projekta).
- Direktoriji su dio istine, ali se do njih dolazi samo preko putanja zapisanih u bazi.
- Identiteti koji povezuju cijeli lanac: `project_id`, `clip_id`, `shot_id`, `part_id`.

## 2. Lanac po koracima

### Korak 1 — Project (kreira)
- **Piše u globalnu bazu:** `projects` (id, naziv, `project_dir`).
- **Kreira projektnu bazu** i direktorije: `proxy/`, `original/`, `audio/`, `incoming/card/`, `incoming/ftp/`, `ingest/thumbnails/`, `filmstrip/`.
- **Piše u projektnu bazu:**
  - `project_settings` + `project_settings_kv` (postavke projekta koje nadjačavaju predložak),
  - `project_template_snapshot` + `project_snapshot_kv` (snimka predloška u trenutku kreiranja),
  - `project_members`,
  - `project_workflow_steps` (redoslijed aplikacija iz predloška, `status` locked / active / complete, `next_step_id`) i `project_workflow_state` (aktivni i ulazni korak).
- Efektivne postavke = postavke predloška, nadjačane postavkama projekta (`project_effective_settings`). Iz njih dolaze i direktorij exporta (`export.directory`) i korijen projekata.

### Korak 2 — Ingest
- **Čita:** efektivne postavke projekta (predložak, direktoriji, proxy recept), `project_dir`.
- **Piše u bazu:** `ingest_assets` (po klipu: probe, trajanje, fps, kanali, `import_status`, putanje `source_path`, `original_path`, `proxy_path`, `project_proxy_path`, sličice), `ingest_jobs`, `ingest_import_batches/_items`, `playback_cache`, `audio_waveforms`, filmstrip; `bump_project_data_revision('ingest')` nakon promjena.
- **Piše u direktorije:** `original/` (kopija originala), `proxy/`, `audio/`, `filmstrip/`, `ingest/thumbnails/`.
- **Signal sljedećem:** `import_status = 'imported'` (ili `done`) na redu klipa. To je definicija „klip je dio projekta”.

### Korak 3 — Media Assist (e → g → čekanje urednika/lektora → l)
- **Čita:** klipove s `import_status = imported` iz `ingest_assets` (v5 `read_imported_clips`), putanje proxyja i sličica, probe podatke, postavke projekta.
- **Piše:**
  - `pool_clips` — v5 to **kopira** iz `ingest_assets` (`sync_pool_from_ingest_db`: briše klipove koji više nisu uvezeni, dodaje nove). Kod nas to mora biti **pogled** (`public_clips`), ne druga tablica (istina o članstvu je ingest).
  - e (Audio AI): `clip_transcripts`, `clip_transcript_segments` (v5 ASR); dodatne AI oznake nisu u v5.
  - g: v5 nema zasebnih podataka (nije određeno).
  - l (Video): `virtual_shots` (korijenski kadar po klipu + kratki kadrovi + cover kadrovi), radne oznake `media_pool_workflow*`.
- **Signal sljedećem:** postojanje/stanje transkripta i virtualnih kadrova po klipu (zaključak; v5 nema izričitu oznaku „klip pripremljen”).

### Korak 4 — Story
- **Čita:** `public_clips` (klipovi), `virtual_shots`, transkripte, postavke projekta.
- **Piše:** `story_parts`, `story_markers`, `story_marker_slots`, `story_covers`, `story_state`, `story_object_history`.
- **Signal sljedećem:** `story_state.committed_at` (u v5 samo vremenska oznaka).

### Korak 5 — Montažna lista, program, export
- **Čita:** samo iz baze — partove i covere (Story), izvore (ingest), putanje originala (`original_path`), postavke (`export.directory`).
- **Izvodi (ne trajni izvor istine):** montažna lista u izvornim koordinatama, program (flat playlist), preview ulaz playera.
- **Piše:** posao exporta (`ingest_jobs`, tip `EXPORT_HIRES`) i izlaznu datoteku u export direktorij iz postavki.

## 3. Tko koga „aktivira”

- **Kojeg sljedećeg pokrećemo** određuje shell iz baze: `project_workflow_state.active_step_id` i `project_workflow_steps.next_step_id`. Aplikacija samo javlja da je gotova, a shell čita bazu i pokreće sljedeću.
- V5 postavlja početno stanje (`project` = complete, prvi korak = active, ostali locked) i popravlja ga pri promjeni predloška, ali **kod koji pomiče korak naprijed (complete → sljedeći active) u v5 nisam našao**. To vlasnik lanca (Project/shell) mora definirati; predlažem da aplikacija zapiše svoj rezultat + oznaku dovršenosti u svojoj tablici, a Project/shell iz te oznake mijenja `status` koraka kroz Projectov write transport.

## 4. Pregled: ulaz → izlaz po aplikaciji

| Aplikacija | Čita | Piše (baza) | Piše (direktoriji) | Oznaka gotovosti |
|---|---|---|---|---|
| Project | – | projekt, postavke, predložak, koraci | struktura direktorija | koraci lanca |
| Ingest | postavke, `project_dir` | `ingest_*`, filmstrip, wave | `original/ proxy/ audio/ filmstrip/ ingest/thumbnails/` | `import_status = imported` |
| MA e | `public_clips*` | transkripti (+ AI oznake, nije definirano) | – | nije definirano |
| MA g | isto + izlaz od e | nije definirano | – | nije definirano |
| MA l | isto + izlaz od e/g | `virtual_shots`, radne oznake | sličice kadrova | nije definirano |
| Story | klipovi, kadrovi, transkripti | `story_*` | cover slike | `committed_at` |
| Export | montažna lista + izvori | posao, rezultat | export direktorij | status posla |

## 5. Što ovo znači za naše module

- **Čitanje** iz baze već imamo kao javne poglede (`qnc-content-read`): `public_clips`, `public_clip_sources`, `public_clip_proxy`, filmstrip, wave. Media Assist ne treba `pool_clips` tablicu; „koji klipovi su u projektu” je upit nad `public_clips` (uvezeni).
- **Pisanje** svaka grupa u vlastite tablice kroz vlastiti write transport; direktoriji se otkrivaju samo preko zapisanih putanja.
- **Oznaka gotovosti** mora postojati kao podatak u bazi (ne u memoriji aplikacije), da shell može odlučiti što je sljedeće.
- **Montažna lista i export** grade se iz baze i mogu se uvijek ponovno izgraditi; ne pohranjuju se kao dodatni izvor istine.

## 6. Odluke potrebne prije ugovora (kratko)

1. **Oznaka dovršenosti koraka:** zapisuje li je svaka aplikacija u svojoj tablici (a Project/shell mijenja `status` koraka), ili aplikacija izravno postavlja svoj korak u `project_workflow_steps`?
2. **Što točno piše e i g** (rezultat za l i za lektora): samo transkript i tekst, ili i druge oznake (govornici, jezik, prijevod, teme)? Što znači „čeka urednika/lektora” u bazi: status u tablici g ili korak lanca?
3. **Direktoriji medija za Media Assist i Story:** koriste li samo direktorije koje je stvorio Ingest, ili imaju vlastite (npr. cover slike, audio komentar, transkript-datoteke)?
