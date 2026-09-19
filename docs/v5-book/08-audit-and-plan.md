# 08 — Audit postojećeg stanja QNC-a i plan nastavka

Osnova: knjiga v5 (00–07) kao popis **potreba**, te stvarno stanje repozitorija. v5 kod nije mjerodavan. Pravila koja plan mora poštivati:
1. **UI je zaključan: samo dodavanje** (postojeće forme, layout ugovori i javne UI komponente se ne mijenjaju).
2. **Baza je jedina poveznica**; sve trajno u bazi; direktoriji samo preko URI-ja zapisanih u bazi.
3. **Aplikacije zasebne** (`qnc-project*`, `qnc-ingest*`, `qnc-media-assist*`, `qnc-story*` bez međusobne ovisnosti, ni tranzitivno); svaka radi samostalno i u shellu.
4. Sve komponente **javne i univerzalne**; forme pasivne, bez logike i testova.
5. Freeze (§14/§17/§18): svaka izmjena postojećeg traži izričito odobrenje s popisom putanja.
6. Laptop → LAN → intranet; OS-neutralno; bez plaćene vanjske usluge.

## 1. Što je izmjereno, a što pročitano

| Provjera | Rezultat |
|---|---|
| `cargo run -p qnc-conformance` (grana `wip/editorial-player-monitor`, danas) | **1 pao od svih provjera**: „business app DB-only isolation” (vidi 3.5). Sve ostale provjere prolaze (ugovori aplikacija i baza, layouti, granice forme, shell, player, timeline). |
| Broj crateova | 82 u `crates/`, 7 aplikacija u `apps/` (`qnc-app` shell, project, ingest, 4 editorial), 5 alata u `tools/` |
| Testovi | **nisu ponovno pokretani** (disk 98% pun, `target` očišćen); zadnje poznate brojke iz ranijih zapisa |
| Ostalo | pročitano iz koda i ugovora (navedeno uz stavku) |

## 2. Stanje repozitorija
- Grana `wip/editorial-player-monitor` (zadnji commit `62deb1b`: popis klipova kao virtualizirana mreža kartica). Stabilna osnova: `restore/2026-09-13` (+ zatvorena odobrenja do „trinaesto”).
- Necommitano: `AGENTS.md` (zapis odobrenja „petnaesto” za prvi korak kartice, treba ga ispraviti prema ovom planu), `docs/89–96`, `docs/v5-book/`, `data/`.
- Otvoreno odobrenje „četrnaesto” (spajanje preview/timeline na player u e, g, l, o) čeka zatvaranje nakon rješenja pada conformancea.

## 3. Audit po obiteljima

### 3.1 Project — riješeno (prema korisniku); potvrđeno u kodu
`qnc-project-{application,desktop,store,close}`, `qnc-project-desktop-adapter`, `qnc-settings-path`, `qnc-work-settings` (čitač postavki), `qnc-export-preset`, `qnc-application-selection`. Ugovori: `project-registry`, `project-workspace`. Postoje postavke `storage.ingest_media`, `storage.original_policy`, `storage.proxy_policy`, `storage.ingest_profile`, `playback.input`, odabir aplikacija (grupe a, b, e, g, l, o).
**Praznine u odnosu na knjigu (01):** nema napredovanja koraka procedure (oznaka gotovosti → sljedeći korak; v5 to također nema); nema zapisa „aplikacija dovršena”; sesije/korisnici: nisu provjereni.

### 3.2 Ingest — riješeno (prema korisniku); potvrđeno u kodu
`qnc-ingest-{application,desktop,store,catalog,select,work-plan}`, `qnc-ingest-desktop-adapter`, `qnc-scanner`, `qnc-camera-{detector,patterns}`, `qnc-sony-metadata`, `qnc-source-{contract,groups,index-contract,index-db,reader}`, `qnc-media-{probe,metadata,metadata-compose,records,record-db,thumbnail}`, `qnc-ffprobe-metadata`, `qnc-filmstrip(+worker)`, `qnc-wave(+worker,+view)`, `qnc-transport-resolver`, `qnc-json-transport`, `qnc-dir-browser`.
Baze: `ingest_content` (projektna baza: `clips`, `clip_sources`, `clip_proxy`, `probe_records`, `filmstrip_*`, `wave_artifacts`, 7 javnih pogleda), `ingest_registry`, `source_index`, `media_records`. Transport: lokalno/LAN/intranet po ugovoru.
DB automat uvoza postoji u `qnc-ingest-store` (`Select` → `QueueSelected` → `ClaimNext` → `FinishImport(media_uri | error)`; stanja `Detected/Queued/Processing/Imported/Failed`).
**Praznine (provjereno pretragom koda):**
1. **Nitko ne poziva `claim_next`/`finish_import`**: nema izvršitelja uvoza. Posljedica: nijedan klip nikad ne postaje `Imported`; Media Assist i Story bi imali prazan popis (98 klipova u vašem projektu su `detected`).
2. **Poster se ne kopira u projekt** (nema `thumb_copy` ekvivalenta); pravilo „poster se kopira kad se kopira proxy/original” nema izvršitelja.
3. Nema stanja `generating_proxy`/`original_ready` niti `linked`/`ready` (izvedivo iz postavki).
4. Zapis odabranog direktorija izvora u `ingest_registry` kroz javni write transport: nije potvrđen (ugovor postoji, sadržaj nisam čitao).
5. Revizija promjena: `CatalogSignature` (izvedena) umjesto `project_data_revisions`.

### 3.3 Player, timeline, UI kit — riješeno
`qnc-broadcast-{player,engine}`, `qnc-audio-output`, `qnc-video-output`, `qnc-monitor`, `qnc-gpu-raster`, `qnc-player-{client,contract,frame-transport,input,launcher,timeline}`, `qnc-timeline(+assets)`, `qnc-ui-kit`, `qnc-media-card`, `qnc-media-pool-head`, `qnc-source-dock`, `qnc-editorial-shell`, `qnc-keyboard-shortcut`. Katalog prečaca `contracts/qnc-keyboard-shortcuts.json` **već sadrži** ID-jeve iz v5: `mark_in`, `select_mark_in`, `add_marker`, `add_marker_continue`, `select_marker`, `delete_marker`, `quick_overwrite_cover`, `add_ton_segment`, `mark_in_fit_duration`, `clear_focus` (dio nije spojen na radnje).
**Praznine:** povlačenje na timelineu; fokus na marker (`Ctrl+M`); prikaz pin-ova markera i slotova pokrivalica po pravilima Storyja provjeriti (v5 ima `M` pin, slot, cover trake); indikator „prekratki izvor” (docs/93 5a) ne postoji.

### 3.4 Editorial (grupe e, g, l, o) — WIP, ne spojeno
`qnc-editorial-{application,desktop}`, `qnc-content-read`, `qnc-source-bindings`, `qnc-source-preview`, četiri aplikacije s adapterima; ugovori aplikacija; ugovori baza **deklarativni (tablice prazne)** za e, g, l; Story ugovor navodi tablice (`story_state`, `story_parts`, `story_markers`, `story_marker_slots`, `story_covers`, `virtual_shots`, `story_edl`, `story_playlist`) i 4 javna pogleda, bez sheme.
Radi: shell u istom layoutu kao Ingest, preview monitor i source timeline spojeni na player, popis klipova kao mreža kartica (svi klipovi, bez filtra uvezenih; `qnc-editorial-desktop` ima vlastitu kopiju kartice, `qnc-media-card` ne koristi).
**Praznine:** popis nije ograničen na `Imported`; nema točkica statusa, sličica, info box overlaya; nema Story procedura; tabovi prazni.

### 3.5 Conformance — pad
„business app DB-only isolation”: `qnc-media-assist-*`/`qnc-story*` (aplikacije i adapteri) tranzitivno ovise o `qnc-ingest-store`. Put: `qnc-editorial-application` → `qnc-source-preview` → `qnc-player-input` → `[dev-dependencies] qnc-ingest-store`. Provjera broji i dev-ovisnosti tranzitivnih crateova.
Rješenja (odabrati): (a) `qnc-player-input` testovi bez `qnc-ingest-store` (izmjena zamrznutog cratea), ili (b) provjera ne broji dev-ovisnosti tranzitivnih crateova (izmjena pravila: potrebno izričito dopuštenje korisnika, AGENTS).

### 3.6 Ugovori i baze (deklarirano)
`contracts/databases/`: `ingest-content`, `ingest-registry`, `source-index`, `media-records`, `project-registry`, `project-workspace`, `story`, `media-assist`, `media-assist-audio`, `media-assist-audio-ai`, `media-assist-video`. Modula: 60+ (`contracts/modules`), uključujući `content-read`, `source-bindings`, `source-preview` (WIP).

## 4. Potrebe iz knjige → postojeće → nedostaje

| Potreba (v5-knjiga) | Postoji kod nas | Nedostaje |
|---|---|---|
| Postavke projekta određuju kopiranje, lokacije, indikaciju (01, 02 §3) | `WorkSettings.storage`, `IngestWorkPlan` | pravila indikacije (status, overlay) kao modul |
| Otkrivanje, probe, odabir (02 I2.3–I2.6) | Select runtime, probe, `select/queue` u content DB | – |
| Uvoz (medij + poster) (02 I2.7–I2.8) | automat u bazi (`QueueSelected/ClaimNext/FinishImport`) | **izvršitelj uvoza** (Ingest obitelj), kopija postera |
| Korijenski (source) virtualni kadar pri probeu (02 I2.5, 03) | – | shema + zapis + pogled `public_source_clips` |
| Short i b-roll kadrovi, IN/OUT slike (03 M3.5) | – | shema `virtual_shots` s klasom, write transport, `public_short_clips`, `public_b_roll_clips` |
| Popis uvezenih klipova u pool-u (03 M3.1–M3.3) | `qnc-content-read` (svi klipovi) | filtar `Imported` (novi pogled/upit) |
| Točkice statusa, sličica, overlay (docs 96) | `qnc-media-card` (točkice po v5 bojama) | `qnc-clip-status`, čitač postera po URI-ju, `SourceAvailability` |
| Transkripti (03 M3.6) | – | grupa e: shema, ASR modul (lokalni) |
| Story dijelovi, markeri, slotovi, cover, commit (04) | – | sheme, pravila (docs 93, 94), write transport, javni pogledi |
| Montažna lista + flat program (04 §3) | `qnc-frame-timebase`, `qnc-player-input` | **program builder** (v5 `qnc-program-playlist`: nema kod nas) |
| Jedinstveni obrazac naredbi I/O/M + Ctrl (docs 95) | katalog prečaca (ID-jevi postoje) | `qnc-edit-focus` modul, povlačenje na timelineu |
| Red poslova, lease, worker (05) | filmstrip/wave worker u procesu aplikacije | opći job service (samo ako zatreba; DB status kao red već postoji za uvoz) |
| Export HI-res (05 §5) | `qnc-export-preset` | job, render worker, `public_exports` |
| Workflow: oznaka gotovosti → sljedeći korak (01 P1.5) | koraci iz predloška | zapis dovršenosti, promjena statusa koraka kroz Project transport |

## 5. Plan nastavka (redom, s ovisnostima)

**Faza 0 — stabilizacija (preduvjet)**
- Riješiti pad conformancea (3.5) uz izričito odobrenje; zatvoriti odobrenje „četrnaesto”; commitati dokumente; spojiti grane.

**Faza 1 — ugovori podataka za Media Assist i Story (samo ugovori, bez UI-ja)**
- Popuniti deklarativne DB ugovore: `virtual_shots` kao **javna zajednička baza bez vlasnika-aplikacije** (klase source/short/b_roll; vidi odjeljak 8), `clip_transcripts*` (e), `story_*` (o) s javnim pogledima i write transportima.
- Odluke potrebne: sadržaj g; verzije priče.

**Faza 2 — Ingest izvršitelj uvoza (Ingest obitelj; traži otključavanje)**
- Novi crate `qnc-ingest-import-worker` (ili sličan): `claim_next` → plan iz `IngestWorkPlan` → link / kopija proxyja / originala / generiranje proxyja → kopija postera **samo kad se kopira medij** → `finish_import`. Rezultat u bazi (`imported_media_uri`, poster URI); revizija.
- Bez ovoga Media Assist ne može imati stvarne klipove.

**Faza 3 — Media Assist temelj (aditivno)**
- Novi javni moduli: `qnc-clip-status` (točkice iz `import_status` + postavki), čitač postera po URI-ju (lokalno/LAN/intranet), `SourceAvailability` + info box overlay (poruka „Ubacite karticu ili odaberite direktorij source zapisa”, izbor direktorija preko `qnc-dir-browser` i Ingest write transporta), `qnc-edit-focus` (obrazac I/O/M + Ctrl).
- Aditivne izmjene forme: nova polja u pogledu (`EditorialClip` dobiva status i poster), nova traka overlaya. **Postojeći widgeti se ne mijenjaju.**
- Funkcije: popis samo uvezenih; source dock IN/OUT; Add virtual clip (short); tabovi Virtual, B-roll; izbor kartice (kvačica).

**Faza 4 — Story (aditivno)**
- Moduli s čistim pravilima: `qnc-story-parts`, `qnc-story-markers` (validacije, slotovi, upiti, podjela slota), `qnc-cover-fit` (usporedba trajanja; crveni indikator), `qnc-program-builder` (montažna lista → flat program), stabilan `slot_id`.
- UI dodaci: segment panel (samo grupa o), marker/cover traka, povlačenje markera, indikator prekratkog izvora (novi slojevi na timelineu kao dodatni sloj, ne izmjena).
- Procedure: docs 93 (P1–P16), docs 94.

**Faza 5 — Export i AI**
- Export: montažna lista → flat program (`OriginalMaster`) → render posao (potvrđena verzija priče); status/otkazivanje.
- Grupa e: lokalni ASR (bez plaćene usluge), zapis transkripta; g nakon definicije.

**Faza 6 — LAN i intranet**
- Lokalno već postoji kroz resolver; dodati čitanje sadržaja preko mreže (`qnc-content-read` danas odbija mrežni endpoint), sesije po korisniku, revizije po vlasniku.

## 6. Otvorene odluke (samo one koje blokiraju)
1. Rješenje pada conformancea (3.5): (a) izmjena testova `qnc-player-input` ili (b) izmjena pravila provjere.
2. Sadržaj grupe g. (Vlasnik `virtual_shots` riješen: nema ga, vidi odjeljak 8.)
3. Otključavanje Ingesta za izvršitelja uvoza (Faza 2).
4. Verzije priče (samo zadnja potvrđena ili povijest).

## 7. Ispravci ranijih dokumenata (89–96) prema ovoj reviziji
- Root (source) virtualni kadar nastaje **pri probeu** (Ingest), ne u Media Assistu (docs/89 B2, docs/90).
- Media Assist popis = samo `Imported` (docs/91, 92, 96 to već navode; docs/89 pogrešno implicira sve klipove).
- Točkice: crvena kod `link` nije greška izvora; kad izvora nema, statusa nema i prikazuje se overlay (docs/96 §9).
- Zapis odabranog direktorija izvora ide u bazu Ingesta kroz njegov write transport, ne u konfiguracijsku datoteku (docs/96 §9).
- Prijedlog „zamijeniti kopiju kartice u `qnc-editorial-desktop` s `qnc-media-card`” povučen (UI zaključan: samo dodavanje).
- Odobrenje „petnaesto” u `AGENTS.md` (nova crate `qnc-clip-status` + izmjene `qnc-content-read`, `qnc-editorial-*`, `editorial.layout.json`) treba prepisati u skladu s Fazom 3 i pravilom „samo dodavanje”; do tada ostaje neiskorišteno.

## 8. Odluka: `virtual_shots` je javan i nema vlasnika-aplikaciju

Odluka korisnika: virtualni kadrovi (source, short, b-roll) nisu ničija tablica; svaka postojeća ili buduća komponenta smije ih čitati i pisati.

Kako to uklopiti u zakon (AGENTS §4: svaki zapis ide kroz javni DB write adapter, aplikacije ne ovise jedna o drugoj):
- **Javni modul `qnc-virtual-shots`** je vlasnik pravila i jedini put zapisa: čista pravila (klasa strogo `source|short|b_roll`, IN/OUT u frameovima, `OUT ≥ IN+1`, root read-only, id `<clip>_shot_NNN`) + javni write transport + javni pogledi. Svaka aplikacija ovisi samo o tom modulu, ne o drugoj aplikaciji, pa izolacija obitelji ostaje.
- **Zaseban DB ugovor** `qnc.db.virtual_shots` (ne dio `story`, `media-assist*` ni `ingest*` ugovora); `owner`/`write_owner` je modul, ne aplikacija. Javni pogledi: `public_source_clips`, `public_short_clips`, `public_b_roll_clips`, `public_virtual_shots`.
- Posljedice za postojeće ugovore: `story.database.json` trenutno navodi `virtual_shots` među svojim tablicama; ta stavka se uklanja (aditivno: novi ugovor + izmjena tog popisa uz odobrenje). Validator ugovora baza (`qnc-db-contract`) treba prihvatiti modul kao vlasnika (aditivna izmjena).
- Zapis iz više procesa u istu SQLite datoteku već je poznat rizik (audit Ingesta: više write transporta, `PERSIST` dnevnik). Modul mora imati jedan serijalizirani put zapisa (`BEGIN IMMEDIATE`, kratke transakcije, `busy_timeout`) i zapise potvrde (`write_receipts`), po uzoru na content transport.
- Root (source) kadar zapisuje **Ingest pri probeu** preko tog istog modula (vlasništvo modula, ne Ingesta), kao i svi ostali.

## 9. Pojednostavljenje: virtualni kadar je sirovina; pisanje samo preko proxy transporta

Odluke korisnika:
1. **Nijedna forma nema pravo pisanja u bazu.** Svaki zapis ide kroz javni proxy/write transport (forma emitira namjeru, aplikacijski sloj zove transport). Vrijedi i za `virtual_shots`.
2. **Virtualni kadar je sirovina** koju koriste Broadcast Player i druge javne komponente i moduli. Ne komplicirati zapis ni vlasništvo.

Posljedica za oblik zapisa (potreba, ne v5 izvedba). Trajno se čuva samo ono što nije izvedivo:
`shot_id`, `clip_id`, klasa (`source` | `short` | `b_roll`), `in_frame`, `out_frame`, opcionalno `source_shot_id`, naziv i vrijeme. **Ne kopira se** ništa što već postoji u zapisu klipa: fps/timebase, probe, trajanje u sekundama, oznaka i boja trajanja, TC (računaju se iz `in/out` i media recorda klipa). Slike IN/OUT su artefakti s URI-jem. Time nestaju v5 duplikati (dvostruki fps, `source_probe_json`, `data_json`, migracije backfill).
Root (source) kadar je izveden iz klipa (`in=0`, `out=trajanje u frameovima`); treba mu zapis samo ako ga drugi zapisi referenciraju (segment, cover).

## 10. Provjera: Sony XML na kartici i kada se radi probe (izmjereno u kodu)

Što Ingest čita s kartice (`qnc-sony-metadata`, docs/26): indeks `MediaProfile` i sidecar `NonRealTimeMeta` (npr. FX6: `XDROOT/Clip/*M01.XML`). Iz njih se promoviraju **činjenice o klipu**: dimenzije (VideoLayout), progresivni format FPS, ukupno trajanje u frameovima (samo za potvrđen normalni progresivni zapis), opis audio kanala, UMID, datum, veza proxy/sličica. Ništa se ne izvodi iz ekstenzije, decimalni NTSC/interlaced se ne pretpostavlja.

Što XML **ne daje** (docs/26, ostaje prazno; `streams_complete` se ne postavlja na true): indeks streama u kontejneru, `time_base`, `start_pts`, `duration_ts`, sample rate/format zvuka, pixel format, popis streamova kontejnera.

Kada se radi probe (`qnc_media_metadata_compose::required_probes`, `qnc_media_metadata::inspect`): samo za **original i/ili proxy čiji zapis ima nedostajuće obavezno polje** (`IssueCode::Missing`). Obavezna polja ugovora `qnc.media.metadata` 0.2.0 uključuju kontejner, trajanje, `streams_complete`, po streamu `index`, `codec`, `time_base`, `start_pts`, `duration_ts` (i audio format). Zaključak: **za Sony karticu XML pokriva podatke o klipu, ali ugovor traži i podatke streamova kontejnera, pa se probe za original (i proxy) i dalje radi jednom.** To odgovara ranijem mjerenju (196 pokušaja probea za 98 klipova = original + proxy po klipu, docs/35). Probe je at-most-once (`BeginAcquisition`), nikad ponovljen za isti medij, i ne pokreće se za klip čiji je snapshot već `Final`.

Izvor za odluku: ako je zahtjev „nema probea kad kartica ima XML”, treba **razdvojiti razine potpunosti**: `Camera-complete` (dovoljno za popis, karticu, Media Assist, source virtual) i `Stream-complete` (potrebno za Broadcast Player i dekoder). Trenutni ugovor ima jednu razinu.

### Odluka (korisnik): probe se radi samo kad nema XML zapisa

- Klip **ima** Sony XML (indeks/sidecar): **probe se nikad ne radi**, ni sada ni kasnije. Podaci iz XML-a su konačni zapis klipa.
- Klip **nema** XML: probe se radi **jednom, u Ingestu** (at-most-once, kao danas).
- Posljedica: metapodaci klipa iz kartice smiju biti konačni (`Final`) bez podataka streamova kontejnera (`time_base`, `start_pts`, indeks streama…). Tko treba te detalje (Broadcast Player, dekoder) čita ih iz same datoteke pri otvaranju, ne iz zapisa.
- Već probani klipovi (196 pokušaja, 98 konačnih zapisa) ostaju kakvi jesu; nova pravila vrijede za klipove koji tek dolaze.

Točke koda koje ovo dotiče (sve zamrznute; potrebno otključavanje s popisom putanja):
| Putanja | Što se mijenja |
|---|---|
| `crates/qnc-media-records` (`inspect` za `Phase::Final`, `completeness`) | Final smije biti snimka iz kamere bez zapisa streamova ako postoji XML dokaz |
| `crates/qnc-media-metadata` (`validation.rs`, obavezna polja) | polja streamova nisu obavezna za zapis s XML dokazom |
| `crates/qnc-media-metadata-compose` (`required_probes`) | s XML dokazom vraća prazan popis |
| `crates/qnc-ingest-select` (`lib.rs` ~700–790) | kamera zapis bez probea upisuje odmah kao Final; probe samo bez XML-a |
| `docs/25`, `docs/33`, `contracts/modules/media-metadata*.json`, `contracts/databases/ingest-content.database.json` | ažurirani opis pravila |

## 11. Kamere kao zasebne komponente (odluka korisnika) i XML za proxy i original

**Sony XML pokriva i proxy i original.** Potvrđeno u `docs/26` i `qnc-sony-metadata`: original iz `Contents/Material`, proxy iz njegova `Proxy` elementa, uz sidecar `NonRealTimeMeta`; proxy ima vlastiti fps i trajanje i ne nasljeđuje prazna polja od originala. Pravilo „XML = nema probea” vrijedi za oba prikaza.

**Što je danas zamjenjivo, a što nije** (izmjereno u kodu):
- Zamjenjivo je grupiranje: `qnc_source_groups::IndexReader` (`reader_id`, `namespace`, `read`) je javno sučelje bez I/O-a, a `qnc_scanner::scan_roles` prima popis čitača i provjerava jedinstvenost `reader_id`.
- **Nije zamjenjivo** izdvajanje dokumenata, sličice i metapodataka: `qnc-ingest-select::adapters()` je ugrađen vektor s jednim Sony unosom (`documents`, `thumbnail`, `metadata`). Nova kamera traži izmjenu Selecta.

**Ciljno rješenje (potreba):**
1. Javno sučelje `CameraAdapter` (novi javni modul): `IndexReader` + `documents(prijedlog)` + `thumbnail(prijedlog)` + `metadata(clip_id, prijedlog, dokumenti)` + **`metadata_sufficiency`** (`Declared` = kartica daje dovoljno, probe se nikad ne radi; `NeedsProbe` = probe jednom u Ingestu).
2. **Po jedan samostalan crate za svaku vrstu kamere** (npr. `qnc-camera-sony`, kasnije `qnc-camera-<proizvođač>`), koji implementira sučelje. Postojeći `qnc-sony-metadata` ostaje nepromijenjen, a adapter ga samo omata (aditivno).
3. Popis adaptera ulazi u Select izvana (composition root aplikacije), a ne iz ugrađenog `adapters()`. Nova kamera = novi crate + jedan unos u composition rootu, bez izmjene Selecta.
4. Kamera bez XML-a (ili bez adaptera) dobiva generički adapter s `NeedsProbe`: probe jednom u Ingestu, kao što je odlučeno.
5. Katalog kamera (`qnc-camera-patterns`) i dalje bira `reader_id`; adapter ga imenuje.

### Ispravak (korisnik): adapter po kameri i vrsti zapisa, ne po proizvođaču

Jedan proizvođač ima više kamera i više vrsta zapisa. Katalog kamera to već razlikuje: **54 uzorka** (`pattern_id`), od toga samo za Sony 12 (`sony-fx6-v6`, `sony-fx6-proxy-chunks`, `sony-a7iv`, `sony-xdroot-sd`, `sony-m4root-sd`, `sony-m4root-cfexpress`, `sony-pxroot`, `sony-xdcam`, `sony-xdcam-ex-bpav`, `sony-xdcam-mxf`, `sony-xocn-cineroot`, `sony-sd-observation`), te npr. `canon-c70` i `canon-c70-mp4`, `panasonic-p2`, `red-r3d`, `gopro-standard-lrv`.

Pravila:
1. **Jedinica adaptera = jedan uzorak kataloga** (kamera × vrsta zapisa), identificiran `pattern_id`. Crate se zove po njemu (npr. `qnc-camera-sony-fx6-v6`, `qnc-camera-canon-c70-mp4`), ne po proizvođaču (`qnc-camera-sony` se ne uvodi).
2. Zajednički dijelovi proizvođača ostaju **zajedničke javne biblioteke** (npr. postojeći `qnc-sony-metadata`: MediaProfile, NonRealTimeMeta), koje adapteri kamera koriste; ne kopiraju se.
3. `CameraAdapter` javlja `pattern_ids()` koje poslužuje i `metadata_sufficiency` **po uzorku**: FX6 s XML-om = `Declared`; uzorak bez metapodataka na kartici = `NeedsProbe`.
4. Registar preslikava `pattern_id` → adapter; uzorak bez adaptera ide na generički adapter (`NeedsProbe`).
5. Nova kamera ili novi format zapisa = novi uzorak u katalogu + novi crate adaptera + unos u composition rootu.

### Izvedeno (sedamnaesto odobrenje) i granice

Izvedeno i provjereno (testovi + conformance): `qnc-camera-adapter` (sučelje, registar, `MetadataSufficiency`), `qnc-camera-sony-fx6-v6` (uzorak `sony-fx6-v6`, `Declared`), `qnc-ingest-select` s registrom izvana (Declared → Final bez probea; NeedsProbe → jedan probe; prazan registar → kontrolirana greška), `qnc-ingest-application` sastavlja registar.

Granice, izmjereno:
1. **(Riješeno naknadnim odobrenjem: `ready()` u `qnc-ingest-store` sada dopušta Final bez ffprobe dokaza.) Uvoz Declared klipa je bio blokiran:** `qnc-ingest-store::ready()` traži `Final` **i** `Complete`; Declared klip je `Final` + `Partial` (ugovor `qnc-media-record-db`: Final s nedostajućim poljima je Partial, test `final_partial_is_immutable_…`). Treba odobrenje za izmjenu `ready()` (npr. Final i (Complete ili bez ffprobe dokaza)). Pokušaj da Final bez probea bude Complete u `qnc-media-records` slomio je taj ugovorni test i vraćen je.
2. **Scanner bira čitač po imenskom prostoru** kataloga i traži točno jedan čitač po uzorku (`ReaderAmbiguous`). Drugi adapter s istim imenskim prostorom (npr. drugi Sony uzorak s MediaProfile) zahtijeva izmjenu odabira po `pattern_id` u `qnc-scanner`.
3. **Kamera bez adaptera / bez XML-a** još nema puta: nema generičkog adaptera koji bi grupirao datoteke bez indeksa. `NeedsProbe` je izveden i testiran samo za adapter koji ima indeks.
4. Uživo na stvarnoj kartici (G:) nije provjereno; provjera je na Sony fixtureima.

### Izvedeno (osamnaesto odobrenje): odluka o probeu po zapisu

Odluka više nije svojstvo cijele kamere nego zapisa (`CameraAdapter::sufficiency(&ClipMetadata)`): ako zapis klipa nosi probe podatke (video: dimenzije, frame rate, točan broj frameova; audio: sample rate, kanali, trajanje; `has_probe_facts`), klip se nikad ne probe-a; ako ih nema (npr. Sony bez sidecara), probe se radi jednom (original i proxy) i drugi Select ga ne ponavlja. Testirano na Sony fixtureima (select 16). **Kamere bez indeksa** (grupiranje bez indeksa, probe prije snimke kamere) i dalje nisu izvedene: traže izmjene `qnc-scanner`, `qnc-source-groups` i ugovora `qnc-media-record-db`.
