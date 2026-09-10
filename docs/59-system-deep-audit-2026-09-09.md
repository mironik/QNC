# QNC - dubinski audit cijelog sustava 2026-09-09

Datum: 2026-09-09.
Predmet: cijeli sustav na radnom stablu, ne samo Git HEAD.
Root: `C:\Users\miron\Projects\QNC`.
HEAD: `875d981` - `Persist incremental Ingest catalog and refine selection UI`.
Prethodni sustavni audit: [docs/43](43-system-deep-audit-2026-09-08.md).
Player koraci: docs/44-57. Transport/DB incident: [docs/58](58-ingest-transport-db-audit-2026-09-08.md).

Kod, AGENTS, Project i korisnicke baze nisu mijenjani ovim auditom.
Ovo je nalaz, ne novo pravilo i ne odobrenje sljedeceg koraka.

## Zakljucak

Od docs/43 radno stablo je dobilo javni Broadcast Player i spojilo ga na
Ingest monitor. To vise nije ugovor na papiru. Play/Pause/korak idu kroz
`player-input` (spremljeni `ingest_content` snapshot) i OOP helper
`qnc-broadcast-player`. Decode koristi ffmpeg bez novog ffprobea.

Import, Filmstrip i Wave i dalje nisu runtime. Story i Media Assist nemaju
crateove. Project freeze nije narusen. Conformance, ukljucujuci novi
`public player boundary`, prolazi.

Najvazniji preostali proizvodni jaz je isti kao juce za medij na disku:
`Uvezi` je stub. Player cita izvor preko URI bindinga, pa playback moze
raditi s kartice bez importa. To ne zatvara import ugovor.

## Sto se promijenilo od docs/43

| docs/43 (HEAD, 8. rujna) | Sada (radno stablo, 9. rujna) |
| --- | --- |
| Player samo ugovor | 11 novih workspace clanova; Ingest play spojen |
| S2 ukljucivao Player kao P1 | Player zatvoren kao runtime; ostaju Filmstrip/Wave |
| Workspace 41 | 52 (41 + 10 crateova + `qnc-player-runner`) |
| Module contracti 36 | 46 |
| WAL/SHM readonly (docs/58 F1) | `enable_owner_write` sada skida readonly i s `-wal`/`-shm`/`-journal` |
| §16: playback nije implementiran | Recenica zastarjela; manifest ima `player-input` i `player-client` |

## Karta sustava

```text
QNC.app
  factory: qnc_project | qnc_ingest

Project (zamrznut)
  pise registry + workspace

Ingest
  Select -> source_index + media_records + ingest_content u qnc_project.db
  Preview/Play -> qnc-player-input -> qnc-player-client
               -> proces qnc-broadcast-player (monitor RGBA)
  Uvezi: odbijen

Broadcast Player (javni modul, OOP helper)
  contract -> input -> stream -> decode (ffmpeg) -> pixel/audio/video
  runtime + runner bin
  nema probe, nema Ingest/Project app crate

Media Assist / Story
  samo contract; read ingest_content
```

| Sloj | Broj | Stanje |
| --- | --- | --- |
| Workspace | 52 | 3 app + 4 tool + ostalo crateovi |
| Module contracti | 46 | Filmstrip/Wave/Timeline/Monitor/Export/Media Browser/test-adapter bez cratea |
| App registry | 2 | Project, Ingest |
| DB contracti | 8 | MA/Story sheme i dalje prazne |

## Project

Freeze scope prazan naspram HEAD. Zadnji Project kod ostaje `0a46c1a`.
`tools/qnc-conformance` ima player granicu; to nije Project crate.

docs/58: `lock_project_dir` i dalje rekurzivno stavlja ReadOnly.
Ingest sada pokusava skinuti ReadOnly s DB + WAL/SHM/journal prije upisa.
Linux/macOS write-bit rizik nije verificiran. Project kod nije diran.

## Shell

Ista dva adaptera. Nema MA/Story taba u runtimeu. `shell_next_group` samo
iz Project adaptera.

## Ingest

Select, inkrementalni katalog, Novi/Sve, checkbox vs fokus: kao u docs/43.

Novo: `playback.rs`. Klik kartice priprema sesiju; Play ne ucitava bazu
ponovo. Nespremljen klip se odbija. Izvor mora imati transport binding.
Video u monitoru tek nakon potvrdenog Playing; do tada poster.

`INGEST_IMPORT_SELECTED` i dalje: "Media import jos nije implementiran."

Helper se trazi kao `qnc-broadcast-player` pored `current_exe`.
`qnc-ingest` Cargo.toml ne ovisi o `qnc-player-runner`; obicni
`cargo run -p qnc-ingest` ne gradi helper.

## Broadcast Player

Implementirano i spojeno:

- ugovor, input iz spremljenog snapshota, media-stream
- decode (ffmpeg, `-nofind_stream_info`), pixel-convert, audio/video out
- runtime, runner, klijent
- seek/ready u engineu; Ingest UI ima samo korak ±1, ne proizvoljni cue

Nema `allowed_applications`. Player crateovi ne ovise o `qnc-ingest` ili
`qnc-project` aplikaciji. `qnc-player-input` cita `qnc-ingest-store` content
ugovor, ne Ingest workflow.

Zivi LAN AV i udaljeni audio uredaj nisu tvrdnja ovog reza (docs/57).

## Probe zakon

ffprobe spawn ostaje u `qnc-media-probe` za Select. Player ne zove probe.
`Command::new` za ffmpeg i player proces nisu drugi probe.

## Story / Media Assist

Samo contracti. Mogu citati napunjen `ingest_content` kad postoji binding.
Nema crateova.

## Dokumentacijski drift

- AGENTS §16 i dalje kaze da playback/filmstrip/wave nisu implementirani
  i da scanner/probe ne smiju biti u manifestu dok ne postoje. Scanner,
  probe i player-input/client jesu u manifestu i u runtimeu.
- docs/01 i dalje tvrdi da source-index i media-records nisu spojeni.
- docs/00 crta Ingest s Filmstrip/Wave kao da su u workflowu.

## Prioritetni nalazi

### S1 - P1: Uvezi nije izvrsen (potvrdeno)

Isto kao docs/43. Store ledger postoji. Komponenta odbija.
Player moze ici na izvorni URI; to nije import.

### S2 - P1: Filmstrip i Wave nemaju runtime (suzeno)

Player je skinut s ovog P1. Generator i artefact tablice i dalje prazni.
Po 8.2 to je sljedeci red nakon playera, ne uvjet za playback.

### S3 - P2: Helper bin nije Cargo ovisnost Ingesta

Play put je stvaran. Bez sibling `qnc-broadcast-player.exe` spawn puca.
Nema primjera u seedu kako se bin isporucuje uz standalone/shell.

### S4 - P2: Transport config i dalje izvan gita

Lokalna `data/ingest-transport.json` postoji (docs/58). U repou je
gitignore. Bez nje Odaberi i player binding padaju.

### S5 - P2: §16 i docs/01 zastarjeli

Kod je ispred odstupanja. Pravila 1-15 i 8.1/8.2 vrijede.

### S6 - P2: docs/58 lock vs WAL

Kod sada skida readonly s pratilaca. Uzrok `attempt to write a readonly
database` na starom procesu nije ponovno live dokazan. Unix lock ostaje
otvoren. To nije odobrenje za mijenjanje Project lock koda.

### S7 - P3

- Conformance mapa crateova i dalje ne nabraja sve player module
  (boundary check ipak prolazi zasebno).
- Proizvoljni cue nije u Ingest UI.
- Poster approve / audio lane i dalje nisu spojeni.

## Granice koje drze

- Nema Ingest/Player -> Project cratea.
- Work settings `query_only`.
- Content authorizer i dalje cuva Project tablice.
- Nema app allowliste ni app bridgea.
- Jedan probe prolaz; player cita spremljeni snapshot.
- Forma ne dekodira; slika dolazi iz player procesa.
- Project freeze scope prazan.

## Conformance

`cargo run -p qnc-conformance` na ovom stablu: **all checks passed**,
ukljucujuci `public player boundary`. Keyboard: 87 akcija.

## Provjereno

- Diff naspram docs/43 i HEAD.
- Freeze scope, shell factory, Story/MA odsutnost.
- Player crateovi, Ingest `playback.rs`, import stub.
- Authorizer i `enable_owner_write` WAL/SHM.
- Ingest manifest vs §16.
- docs/01 zastarjelost.
- Conformance.

## Nije provjereno

- Live Play na stvarnoj kartici u ovom prolazu.
- Fizicki LAN/Intranet AV.
- Linux/macOS lock + upis.
- Pun `cargo test --workspace`.
- Da li korisnicki `target/release` trenutno ima sibling helper.

## Sljedeci rizik

Tretirati player kao gotov Ingest proizvod bez isporuke helper bina ostavlja
Play mrtvim u cistom `cargo run -p qnc-ingest`. Pokrenuti Filmstrip prije
importa je dopusteno samo iz baze, bez probea. Import i dalje odvaja
"moze se gledati s kartice" od "medij je u projektu".

Predlozeni redoslijed, bez implementacije:

1. Uskladiti §16 i docs/01 s player/Select stanjem.
2. Dogovoriti isporuku `qnc-broadcast-player` uz Ingest/shell.
3. Izvrsni import po zapisanim politikama.
4. Filmstrip, zatim Wave, samo iz baze.
5. Story / Media Assist crateovi.
)
