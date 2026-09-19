# 96 — Ingest u v5: baze, životni ciklus klipa, procesi i workeri

Izvor: `QNC_v5/qnc-host/src/{ingest/*,jobs.rs,media/resolve.rs}`, `qnc-worker/src/lib.rs`, `qnc-service-contracts`; usporedba s našim `qnc-ingest-store` (`content/`), `qnc-filmstrip-worker`, `qnc-wave-worker`. Samo čitano. „(zaključak)” = izvedeno.
Pravilo korisnika: **klip koji nije UVEZEN ne prikazuje se u media cardu** (Media Assist i Story). Ovaj dokument opisuje kako klip postaje uvezen.

## 1. Baza (v5, jedna projektna baza `qnc_project.db`)

| Tablica | Uloga | Ključni stupci |
|---|---|---|
| `ingest_assets` | jedan red po klipu (`source_id`, `clip_id`) | `import_status`, `status` (`linked`/`ready`), `selected`, probe (trajanje, fps, kanali, codec…), `source_path`, `original_path`, `proxy_path` (s kartice), `project_proxy_path` (u projektu), `thumb_path`, `card_thumb_path`, `thumb_status`, `read_from_card`, `card_locked`, `virtual_name` |
| `ingest_jobs` | red poslova | `job_id`, `job_type`, `source_id`, `clip_id`, `status` (`queued`→…→`done`/`error`), `attempts`, `payload_json`, lease |
| `ingest_import_batches` / `_items` | serija uvoza | `status` (`preparing`→`filmstrip`→`waveform`→`done`), stavke `(batch, source, clip, media_job_type)` |
| `ingest_meta` | postavke sesije | `active_source_id`, `card_root`, `archive_original`, `selection_revision` |
| `playback_cache` | predmemorija reprodukcije | `status`, `ready_start/end_frame`, `cache_path` |
| `audio_waveforms` | valni oblik | peaks |
| `project_data_revisions` | revizija `ingest` | podiže se pri svakoj završenoj promjeni |

Odabir (`selected`) je trajan podatak u bazi; **uvoz se pokreće samo iz baze** („DB-first: UI payload nije mjerodavan”).

## 2. Životni ciklus klipa (v5)

```
otkriven (import_status = '')  ──probe──►  odabran (selected=1)
        │                                          │  queue_import (samo iz baze)
        │                                          ▼
        │                       queued            (ingest_media_prepare)   ← link / copy / original
        │                       generating_proxy  (proxy_generate)         ← proxy bez proxyja na kartici
        │                                          │  worker preuzme posao (claim)
        │                       processing         (samo za queued)
        │                                          │  worker završi → host: complete_imported_clip
        ▼                                          ▼
      error  ◄────────── greška ──────────  imported   (status = linked | ready)
                                               │  (opcionalno) original_ready → arhiviranje originala u projekt
```

- Preskaču se klipovi koji su već `imported`/`done` (`skipped_imported`) ili aktivni (`queued`/`processing`/`generating_proxy`).
- Ponovni pokušaj: klip u `error` može opet u red.
- **`imported` postavlja samo `complete_imported_clip`** (dijele ga `ingest_media_prepare` i `proxy_generate`). U jednoj transakciji zapisuje: `import_status='imported'`, `status` (`linked`/`ready`), konačni probe (trajanje, fps, kanali, raster), `project_proxy_path` (samo ako je medij u `project/proxy`), `original_path`, `thumb_path` (ako poster postoji), `read_from_card`, `card_locked`; podiže reviziju `ingest`.

## 3. Što se radi pri uvozu odlučuju postavke projekta

`resolve_import_plan(meta, settings)` po `storage.ingest_media`:

| Postavka | Način | Izvor medija | `status` |
|---|---|---|---|
| `link` | `LinkInPlace` (bez kopije) | po `playback.input`: `proxy` (traži proxy), `original`, `proxy_if_available` (proxy pa original) | `linked` |
| `proxy` | proxy s kartice → `CopyToProject`; bez proxyja → `GenerateProxy` (transcode originala u `proxy/`); audio → kopija | proxy/original | `ready` |
| `original` | `CopyOriginalToProject` u `original/` | original | `ready` |

Plan akcija (`ImportActionPlan`): uvijek `CopyCardPosterIfAvailable`, zatim `PrepareMedia` (link/copy/original) ili `GenerateProxy`. Nedostatak izvora daje `error` s porukom (`nema proxy ni originala — otkrij materijal`, `playback.input=original, ali original nije dostupan`).
Dodatno: arhiviranje originala u projekt (`archive_original`, zadano isključeno; nedostupno za breaking news/house media) tek nakon uvoza, kao zaseban posao `original_archive_copy`.

## 4. Procesi i workeri

| Proces | Što radi | Piše |
|---|---|---|
| **Host** (`qnc-host`) | jedini pisac baze; queue_import, serijalizirano pisanje po projektu (`serialize_project_write`), lease/heartbeat, faze serije | `ingest_*`, revizija |
| **Worker** (`qnc-worker`, `local_workstation` / `intranet_shared_media`) | izvršava poslove (claim → izvrši → complete/fail); ne dira bazu | datoteke u direktorijima projekta |
| `media_probe` | brzi ffprobe izvora | probe u `ingest_assets` |
| `thumb_copy` / `thumb_proxy` | poster klipa (kopija s kartice ili iz proxyja) | `ingest/thumbnails/<clip>/poster.jpg`, `thumb_*` |
| `ingest_media_prepare` | kopiranje u komadima (`.partial`) ili link | `original/` ili veza |
| `proxy_generate` | transcode u proxy | `proxy/…` |
| `original_archive_copy` | arhiva originala | `original/` |
| `waveform` | valni oblik | `audio_waveforms` |
| `audio_wrap` | audio-omot | podaci |
| `playback_cache_prepare` | predmemorija reprodukcije | `playback_cache` |

Pipeline serije (`advance_selected_import_pipeline`): `preparing` (medij + poster) → tek kad su **svi** klipovi serije `imported`/`error` i poster je `ready`/`no_card_thumb`/`error` → `waveform` (samo klipovi `imported` s videom) → `done`. Idempotentno (`queue_ingest_artifact_job_once`). Za vrijeme reprodukcije radnici dobivaju samo dopuštene poslove.

## 5. Naš Ingest (stanje danas) i razlike

| Područje | v5 | Naš sustav |
|---|---|---|
| Stanja | `''`, `queued`, `generating_proxy`, `processing`, `original_ready`, `imported`/`done`, `error` | `Detected`, `Queued`, `Processing`, `Imported`, `Failed` (5 stanja) |
| Red poslova | tablica `ingest_jobs` + batch tablice | **sam `import_status` je red**: `QueueSelected` (detected/failed → queued, samo odabrani s gotovim metapodacima), `ClaimNext` (queued → processing u transakciji) |
| Izvršitelj uvoza | worker `ingest_media_prepare` / `proxy_generate` | **nema izvršitelja** (ugovor: „Import execution is not implemented in this step”); nitko ne postavlja `Imported` |
| Filmstrip | nije dio uvoza (na zahtjev) | `qnc-filmstrip-worker` (u procesu aplikacije, do 2 radnika, pauzira se za playera): JPEG **112×64**, do 13 po klipu, u `filmstrip/<klip>/` + `public_filmstrip_frames` |
| Wave | `waveform` posao | `qnc-wave-worker`, `public_wave_artifacts` |
| Poster | `thumb_copy`/`thumb_proxy` → `ingest/thumbnails/<clip>/poster.jpg` | referenca kamerine sličice (`thumbnail_uri`) iz Select-a čita se preko `SourceReader`; **kopije u projekt nema** (dio nerealiziranog uvoza) |
| Probe | u `ingest_assets` | `probe_records` + `media_records` baza (zaseban zapis, `record_db_uri`, `record_revision`) |
| `status` linked/ready | stupac | nema (izvedivo iz postavki i `imported_media_uri`) |
| Revizija | `project_data_revisions('ingest')` | `revision` po klipu + `CatalogSignature` |

## 6. Posljedice za media card (Media Assist i Story)

1. **Popis = samo `import_status = Imported`** (v5 `read_imported_clips … WHERE import_status = 'imported'`). Klipovi u `detected`/`queued`/`processing`/`failed` se ne prikazuju; prazan popis kaže „Nema klipova — prvo Ingest import.”. To vrijedi i za naš projekt: **98 klipova je `detected`, pa bi Media Assist trenutno bio prazan**, jer uvoz nema izvršitelja.
2. **Točkice** (v5, tek za uvezene klipove): proxy — zelena kad je proxy spreman, žuta u tijeku, crvena inače; original — plava kad je u projektu, žuta dok se kopira, **crvena kad nije u projektu** (u v5 to je normalno stanje za `link`). Korisnik čita crvenu/zelenu/žutu kao status klipa.
3. **Poster kartice je proizvod uvoza**, ne filmstripa. Filmstrip okviri su 112×64 (za timeline) i preslabi za karticu (~213×120). Poster treba nastati pri uvozu (kopija s kartice ili iz proxyja) u direktorij projekta i biti javno objavljen (`thumbnail_uri`).
4. Sve što Media Assist treba za svoj popis nalazi se u javnim pogledima: `public_clips` (status, trajanje, `imported_media_uri`), `public_clip_proxy`, `public_probe_records`; poster kad Ingest objavi.

## 7. Što treba odlučiti / nedostaje

1. **Izvršitelj uvoza** (bez njega nema uvezenih klipova, dakle ni sadržaja u Media Assistu): tko ga gradi i kada (Ingest je zamrznut). Predložen obrazac iz v5: `ClaimNext` → izvrši prema postavkama (link/proxy/original) → jedan završni zapis (`Imported` + `imported_media_uri` + poster) + revizija.
2. **Poster nakon uvoza**: gdje leži (`ingest/thumbnails/<clip>/poster.jpg`) i koji stupac/pogled ga objavljuje.
3. **Značenje crvene točkice originala** za `link` projekte: zadržati v5 (crveno = original nije u projektu) ili prikazati drukčije? (Ranije predložena siva boja nije u v5.)
4. **Dok uvoza nema:** razvoj i testiranje kartice na fixtureu s klipovima označenim kao uvezeni (npr. primjer aplikacije), a ne na stvarnom projektu.

## 8. Dogovoreno: gdje će što biti i što se kopira određuju postavke projekta

Nijedna aplikacija ne odlučuje o lokaciji ni o kopiranju sama. Ingest (i svaka druga aplikacija) čita postavke iz baze i po njima gradi plan (`IngestWorkPlan` iz `WorkSettings`).

| Odluka | Postavka (Project je zapisuje) |
|---|---|
| Gdje žive projekti | `storage.projects_root` (standardni token ili putanja) → `output_root_uri` projekta |
| Struktura izlaza | standardni raspored unutar lokacije projekta (`original/`, `proxy/`, `audio/`, `incoming/`, `ingest/thumbnails/`, `filmstrip/`; docs/38): izvodi se iz `output_root_uri`, ne iz aplikacije |
| Profil rada | `storage.ingest_profile` = `field` (Teren) / `house` (TV kuća) |
| Što se uvozi | `storage.ingest_media` = `link` / `proxy` / `original` |
| Proxy | `storage.proxy_policy` (npr. `link_when_available`, `use_house_media`, `generate_if_missing`) |
| Original | `storage.original_policy` = `link_when_available` (samo link), `copy_background` (kopiraj u pozadini), `ignore_for_fast_news` |
| Što player čita | `playback.input` = `original` / `proxy` / `proxy_if_available` |
| Gdje ide export | `export.directory` |

Napomene:
- v5 prekidač „Kopiraj original u projekt” (`ingest_meta.archive_original`, zadano isključeno, ovisio o profilu) kod nas **ne postoji kao izbor Ingesta**: njegovu ulogu preuzima `storage.original_policy = copy_background`.
- Poster (`ingest/thumbnails/<klip>/poster.jpg`), filmstrip i wave leže na standardnim mjestima projekta; njihova lokacija dolazi iz postavki (`output_root_uri`), a u bazi ostaje samo URI.
- **Poster nema vlastitu postavku: vezan je uz kopiranje medija (odluka korisnika).** Ako se kopira proxy ili original (`ingest_media` = `proxy`/`original`, a pretpostavka: i `original_policy = copy_background`), kopira se i poster u projekt (`ingest/thumbnails/<klip>/poster.jpg`). Ako se ne kopira ništa (`link`), poster se ne kopira, a **korisnik za rad treba izvor**: originalnu karticu, ili kopiju na računalu, LAN-u ili intranetu.
- Posljedice: (1) `public_clips.thumbnail_uri` je jedino mjesto istine za poster; pri kopiranju Ingest upisuje URI kopije u projektu, pri `link` URI reference na izvoru; čitač razrješava URI kroz vezanja (lokalno / LAN / intranet). (2) Kad izvor nije dostupan (kartica izvađena, mreža nedostupna), kartica ostaje u popisu (klip je uvezen), a sličica je rezervni prikaz („…”); to nije greška klipa. (3) Dostupnost izvora je stanje koje forma prikazuje pasivno; provjeru radi aplikacijski sloj. (4) Isti obrazac vrijedi za reprodukciju: `playback.input` čita izvor s tog mjesta.

## 9. Dogovoreno: postavke projekta određuju i indikaciju (status, info box)

- Statusi (točkice) prikazuju se samo kad postavke projekta i dostupnost izvora to dopuštaju.
- **Postavka `link` na karticu, a kartice nema** (izvor nedostupan): **nema statusa**, a preko popisa se prikazuje **info box overlay**: „Ubacite karticu ili odaberite direktorij source zapisa.”
- Kad se medij kopira (`proxy`/`original`), podaci su u projektu, pa overlay nema; status se prikazuje po pravilima iz odjeljka 6.
- Slijed u sustavu: aplikacijski sloj iz postavki i vezanja razrješava izvor (lokalno / LAN / intranet) i daje pasivnom prikazu stanje `SourceAvailability` (dostupan / nedostupan + poruka). Prikaz samo crta točkice ili overlay i emitira namjeru „odaberi direktorij”. Ne provjerava datoteke.
- **Sva istina je u bazi (odluka korisnika).** Lokacija izvora zapisuje se u bazu, ne u konfiguracijsku datoteku. Vlasnik je Ingest: `qnc.db.ingest_registry` (tablice `source_cards`, `source_locations`, `source_sessions`; javni pogledi `public_source_cards/_locations/_sessions`) i `qnc.db.source_index`. Datoteka `data/ingest-transport.json` je samo privatno vezanje „URI → fizička datoteka/korijen” koje postavlja owner aplikacija; nije istina o lokaciji i nije javni identitet (AGENTS §9).
- Tok radnje „odaberi direktorij source zapisa”: (1) overlay u Media Assistu/Storyju emitira namjeru; (2) javni `qnc-dir-browser` vraća **QNC URI** odabrane lokacije, ne OS putanju; (3) aplikacijski sloj šalje URI **javnom write transportu vlasnika (Ingest)**, koji upisuje `source_locations`; (4) čitatelji (Media Assist, Story, player) čitaju novu lokaciju kroz `public_source_locations` i razrješavaju je resolverom. Media Assist i Story ne pišu u tuđu bazu i ne otvaraju direktorij sami.
- Potvrdne tipke („Odaberi”, „Odustani”) pripadaju aplikacijskoj akcijskoj traci, a ne Dir Browseru (AGENTS §9).
- Provjera: sadržaj `ingest_registry` u ovom projektu nisam čitao (nema alata za čitanje baze); vlasnik i tablice su iz ugovora `contracts/databases/ingest-registry.database.json`.
