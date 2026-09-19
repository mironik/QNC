# 92 — QNC v5: procesi, workeri i direktorij projekta

Nadopuna dokumentima 89–91. Izvor: `QNC_v5/qnc-worker`, `qnc-host/src` (jobs, ingest_*, waveform, asr) i stvarni direktorij projekta na disku. Samo čitano. Sadržaj baze nisam mogao čitati alatom (na računalu nema `sqlite3` ni Pythona), pa su tvrdnje o retcima izvedene iz nizova u datoteci i označene „(zaključak)”.

Odluka korisnika: **što pišu grupe e i g određuje se naknadno, uz razvoj tih formi s aktivnim kodom.** Ovdje se za njih ne definira ništa.

## 0. Ispravak: pravi primjer je projekt našeg sustava, ne v5

Odjeljak 1 opisuje projekt otvoren u v5 (`novi13_…`, tablice `ingest_assets`, `virtual_shots`, `story_*`) pa je filmstrip u njemu prazan. Mjerodavan primjer za naš sustav je
`C:\Users\miron\Test projekt\novakkkkkll_e925387640d3471b96c3d0e48a4b59af\` (baza 12,6 MB + WAL 4,2 MB):

```
qnc_project.db (+ -shm, -wal)
filmstrip/<naziv klipa>/NNN_S_CC.jpg    98 klipova, 1269 JPEG-ova (12–13 po klipu), 5,8 MB
exports/projekti/                       prazno
audio/ ingest/ original/ proxy/ incoming/   prazno
```

- Naziv slike `NNN_S_CC.jpg`: redni broj kadra, cijele sekunde, stotinke (`001_0_54` = kadar 1 na 0,54 s). Isto je u `filmstrip_frames.seek_sec`.
- Tablice su **naša shema** (`crates/qnc-ingest-store/src/content/schema.sql`, ugovor `ingest-content.database.json` 0.2.0): `clips`, `clip_sources`, `clip_proxy`, `probe_records`, `filmstrip_artifacts`, `filmstrip_frames`, `wave_artifacts`, `ingest_content_schema` + projektne `project_*` i `project_origin`. Nema `ingest_assets`, `pool_clips`, `virtual_shots`, `story_*`.
- Stanje: 98 klipova `detected` (Select ih je objavio), 2 zapisa `imported`/`error` u tekstu (dio klipova je prošao pokušaj uvoza), **filmstrip generiran za sve**, a uvoz medija (`proxy/`, `original/`, `ingest/thumbnails/`) nije izvršen. Ugovor to i kaže: „Import execution is not implemented in this step”.
- Sve reference su URI-ji, ne putanje: `qnc://local/source/volume-<serijski>/file/PRIVATE/XDROOT/Clip/<klip>` (izvor na kartici), `qnc://local/project/<project_id>/…` (artefakti u direktoriju projekta), `qnc://local/artifact/probe-…`, `qnc://local/db/media_records`, `qnc://local/db/source_index`, `qnc://local/db/ingest_content/<project_id>`. Rješavanje URI-ja u datoteku ide kroz vezanja hosta (`data/ingest-transport.json`), pa ista baza radi lokalno, na LAN-u i intranetu.
- Filmstrip su **datoteke** u `filmstrip/`, a baza čuva samo URI i `seek_sec` (`public_filmstrip_frames`). To je obrazac koji Media Assist i Story već čitaju kroz `qnc-content-read`.
- Baza je u WAL načinu (postoje `-wal`/`-shm`): više čitatelja uz jednog pisca, što odgovara lancu „Ingest piše, ostali čitaju”. Ugovor traži da se postojeći WAL ne mijenja.
- Razlika prema v5: v5 drži kopiju članstva (`pool_clips`) i sličice/putanje u `ingest_assets`; kod nas je članstvo pogled `public_clips`, a putanje su URI-ji iz `clip_sources`/`clip_proxy`.

Ispravak zaključka iz odjeljka 1: „filmstrip je u bazi” **ne vrijedi**; u v5 projektu je samo prazan jer taj projekt nije prošao generiranje.

## 0a. Što se kopira, a što ne: odlučuju postavke projekta

Project zapisuje postavke (iz predloška, s nadjačavanjem po projektu) u bazu; Ingest ih **samo čita** i po njima radi plan. Ništa se ne kopira „po zadanom” niti po izboru aplikacije.

Postavke (`WorkSettings.storage`, `qnc-work-settings`; u bazi `public_project_settings.settings_json`):

| Ključ | Vrijednosti / značenje |
|---|---|
| `storage.ingest_media` | `link` (medij ostaje na izvoru, upisuje se samo veza), `proxy` (kopira se proxy), `original` (kopira se original u projekt) |
| `storage.proxy_policy` | npr. `use_house_media`, `link_when_available` (koristi proxy s kartice/kuće ako postoji) |
| `storage.original_policy` | npr. `link_when_available`, `ignore_for_fast_news` |
| `storage.ingest_profile` | npr. `house`, `field` (profil rada) |
| `playback.input` | `original`, `proxy`, `proxy_if_available` (što player čita) |
| `output_root_uri` | korijen izlaza projekta; iz njega se izvode `original/`, `proxy/`, `audio/`, `incoming/`, `ingest/thumbnails/`, `filmstrip/` (`qnc-ingest-work-plan`) |

Stvarne vrijednosti u `novakkkkkll_…` (predložak `tpl_ingest_house`): `ingest_media = link`, `ingest_profile = house`, `proxy_policy = use_house_media`, `original_policy = link_when_available`, `playback.input = original`. Zato u tom projektu nema kopija u `original/` i `proxy/`, dok filmstrip i sličice (uvijek izlaz Ingesta) postoje. To nije nedostatak nego posljedica postavki.

Posljedice za lanac:
- **Ingest:** po `ingest_media` odlučuje hoće li stvarati `ingest_media_prepare` / `original_archive_copy` / `proxy_generate` poslove. `link` znači samo probe, filmstrip, wave i zapis URI-ja.
- **Media Assist / Story / Export:** ne pretpostavljaju kopiju. Klip se uvijek razrješava iz baze (`clip_sources.original_uri`, `clip_proxy.proxy_uri`, `imported_media_uri`) i **`playback.input`**: `original` čita izvor s kartice/mreže, `proxy` traži proxy, `proxy_if_available` pada na original. Ako izvor nije dostupan (kartica izvađena), to je kontrolirana greška, ne zamjena.
- **Točkice statusa** moraju poštivati postavke: kad je `ingest_media = link`, original „nije u projektu” je normalno stanje, a ne pogreška (v5 tu razliku ne pravi i crta crveno; zato pitanje o `idle`).
- **Export HI-res** uvijek koristi original (`OriginalMaster`), pa u `link` projektu treba dostupan izvor; to se provjerava prije pokretanja posla.
- v5 je imao dodatni prekidač u `ingest_meta` (`archive_original`, zadano isključeno, dostupan ovisno o profilu). Kod nas taj izbor pripada postavkama projekta (Project), ne Ingestu.

## 1. Direktorij projekta otvorenog u v5 (za usporedbu)

`C:\Users\miron\Test projekt\novi13_1789816756\` (projekt sa snimki, „Projekt novi13”):

```
qnc_project.db                      843 KB   (jedina baza projekta)
audio/                              prazno
filmstrip/                          prazno
incoming/card/  incoming/ftp/       prazno
original/                           prazno
proxy/                              prazno
ingest/thumbnails/<clip>/poster.jpg 5 klipova (mironik_1503 … mironik_1507)
virtual_shots/<clip>_shot_NNN/cover.jpg, out_cover.jpg   2 kadra
```

Što se iz toga vidi:
- Ovaj projekt **ne kopira originale** (`original/` prazan): izvori se čitaju izravno s kartice/diska (u bazi se vide putanje na `G:`, zaključak). Zato je arhiviranje originala (`original_archive_copy`) isključeno.
- `proxy/` je prazan: proxy datoteke leže drugdje ili se ne stvaraju za te klipove; putanja je u `ingest_assets.project_proxy_path` / `proxy_path` (zaključak, bez čitanja retka).
- `filmstrip/` je prazan jer je ovo v5 projekt bez generiranog filmstripa (v5 ga prikazuje iz vlastitog stanja); netočan raniji zaključak „filmstrip je u bazi” povučen, vidi odjeljak 0.
- Sličice i naslovnice: `ingest/thumbnails/<clip_id>/poster.jpg` (Ingest) i `virtual_shots/<shot_id>/cover.jpg|out_cover.jpg` (Media Assist/Story, IN i OUT kadar).
- Tablice u bazi ovog projekta: `project_*` (postavke, članovi, predložak, koraci, revizije), `ingest_*` (assets, jobs, batches, meta, `ingest_metr`), `playback_cache`, `audio_waveforms`, `virtual_shots`, `story_state/parts/markers/marker_slots/covers`. **Nema** `pool_clips`, `clip_transcripts`, `media_pool_*` u ovom projektu, jer se te tablice stvaraju tek kad Media Assist otvori bazu; Story tablice postoje.
- Nazivi klipova: `Mironik 1500…1509.MXF`; kadrovi se zovu `<clip>_shot_001`, korijenski `import_root`.

Pravilo: **datoteke su izlaz aplikacije, putanja do njih je uvijek u bazi.** Direktorij bez baze ne znači ništa.

## 2. Procesi

| Proces | Uloga | Piše u bazu? |
|---|---|---|
| `qnc-host` | HTTP API, **jedini pisac baze** za poslove; redoslijed faza uvoza, red poslova, lease/heartbeat | da |
| `qnc-worker` | vanjski radnik: zatraži posao (claim), izvršava, javlja rezultat (complete/fail) HTTP-om | **ne**, samo kroz host |
| `qnc-player-runner` | Broadcast Player (izlaz slike/zvuka) | ne |
| aplikacije (`qnc-app`) | UI | ne izravno (v5: kroz host API) |

Worker se konfigurira: `--capability` (koje tipove poslova smije), `--placement` (`local_workstation` ili `intranet_shared_media`), `--poll-ms`, `--lease-ms`, `--media-probe-parallelism`. Preslikava se na naše faze: laptop = `local_workstation`, TV intranet = `intranet_shared_media`.

## 3. Poslovi (worker handleri) i njihovi rezultati

| Tip posla | Što radi | Rezultat (baza / direktorij) |
|---|---|---|
| `media_probe` | brzi ffprobe izvora | probe podaci → `ingest_assets` |
| `ingest_media_prepare` | kopira medij u komadima (`.partial` pa preimenovanje) | datoteka u projektu |
| `original_archive_copy` | arhivira original u `original/` | datoteka + status |
| `thumb_proxy`, `thumb_copy` | sličica/poster klipa | `ingest/thumbnails/<clip>/poster.jpg`, `thumb_*` stupci |
| `proxy_generate` | proxy iz izvora | proxy datoteka, `project_proxy_path` |
| `waveform` | valni oblik | `audio_waveforms` |
| `audio_wrap` | audio-omot (voice) | podaci u bazi |
| `playback_cache_prepare` | priprema predmemorije za reprodukciju | `playback_cache` (ready_start/end_frame, chunks, cache_path) |
| `export_hires` | render iz montažne liste na originalima | izlazna datoteka u export direktoriju; status u `ingest_jobs` |
| `qnc_worker_smoke` | test | – |

Redak posla (`ingest_jobs`): `job_id`, `job_type`, `source_id`, `clip_id`, `status` (queued → claimed → done/failed), `attempts`, `payload_json`, vremena. Faze uvoza (`ingest_import_batches.status`): `preparing` → `filmstrip` → `waveform` → `done`; host prelazi u sljedeću fazu tek kad su svi klipovi serije gotovi (`advance_selected_import_pipeline`).

Pravila procesa koja treba zadržati:
1. **Pisanje samo kroz hosta** (kapija po projektu: `serialize_project_write`); worker ne otvara bazu.
2. **Atomski izlaz:** datoteke se pišu kao `.partial`, pa preimenuju; rezultat se javlja tek nakon toga.
3. **Lease + heartbeat:** posao koji radnik ne obnovi vraća se u red; nema dvostrukog izvršenja.
4. **Reprodukcija ima prednost:** dok player svira (`BackgroundWorkGate`, lease 5 s), host daje radnicima samo poslove koji smiju raditi za vrijeme reprodukcije.
5. **Idempotentnost:** `queue_ingest_artifact_job_once` ne stvara duplikate; ponovno pokretanje uvoza ne ponavlja gotov posao.
6. Poslovi su vezani za aktivni projekt (`project_id` iz zahtjeva, inače aktivni projekt hosta); bez projekta nema poslova.

## 4. Što to znači za naš lanac

- Poslovi su **dio baze** (red poslova + rezultati), pa je i „što se trenutno obrađuje” vidljivo iz baze bilo kojoj aplikaciji, uključujući Media Assist i Story (stanje točkica statusa dolazi iz `import_status` i stanja poslova).
- Media Assist i Story **ne trebaju vlastite workere** za članstvo klipova; trebaju ih tek za nove obrade (ASR u e, render u Exportu). Isti obrazac: red poslova u bazi, worker s `--capability`, pisanje rezultata kroz vlasnika.
- Za e i g: **ništa se ne definira sada.** Kad se razvijaju s aktivnim kodom, dodaju se novi tipovi poslova (npr. `asr_transcribe`) i njihove tablice po ovom obrascu.

## 5. Preostala otvorena pitanja (samo ova dva)

1. **Oznaka dovršenosti koraka:** predlažem da je aplikacija zapiše u svoju tablicu, a Project/shell iz nje mijenja stanje koraka u `project_workflow_steps`. Odgovara li?
2. **Direktoriji Media Assista i Storyja:** v5 ih već koristi (`virtual_shots/<shot>/cover.jpg`); predlažem da to ostane pod direktorijem projekta, s putanjom u bazi. Odgovara li?
