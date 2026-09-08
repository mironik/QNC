# QNC - ponovni dubinski audit 2026-09-07

Datum: 2026-09-07 (drugi prolaz, isti dan).
Predmet: Git HEAD, cisto radno stablo.
Root: `C:\Users\miron\Projects\QNC`.
HEAD: `9a09e93` - `Implement modular Ingest Select, persisted metadata and card thumbnails`.
Prethodni prolaz: [docs/36](36-deep-audit-2026-09-07.md) (pisan na necommitano stablo uz `0a46c1a`, zatim commitan u `9a09e93`).

Kod, AGENTS, Project i korisnicke baze nisu mijenjani ovim auditom.
Ovo je nalaz, ne novo pravilo i ne odobrenje sljedeceg koraka.

## Zakljucak

Select je u HEAD-u, ne samo u radnom stablu. `Odaberi` i dalje radi scan,
original/proxy grouping, `source_index`, jedan probe i `media_records`.
`Uvezi` je odbijen. `ingest_content` (`clips`, `probe_records`) ostaje prazna
shema. Story i Media Assist i dalje smiju citati samo `ingest_content`.

Od docs/36 zatvorena su dva sitna nalaza: `image-assets` je u Ingest
manifestu, `source-reader` crate i contract su `0.3.0`. AGENTS §16 sada
opisuje Select, ali ostavlja stari zabranu da scanner/probe ne smiju biti
u manifestu. Manifest ih ima. docs/01 i docs/33 jos tvrde da Odaberi nije
spojen.

Najvazniji blokator je isti: nema javnog clip/probe kataloga koji kasnije
aplikacije smiju citati. `media_records` je trajni owner zapis, nije ugovor
koji Story/Media Assist deklariraju.

## Sto se promijenilo od docs/36

| docs/36 (necommitano uz 0a46c1a) | Sada (HEAD 9a09e93, cisto) |
| --- | --- |
| Select crateovi necommitani | Isti kod u commitu |
| F5: `image-assets` nije u manifestu | Zatvoreno; deklarirano i koristi se |
| F7: source-reader 0.3.0 / contract 0.2.0 | Zatvoreno; oboje 0.3.0 |
| AGENTS §16 kaze da je Ingest samo browser | Select je opisan; stara zabrana manifesta ostaje |
| "36 Rust paketa" | 41 workspace clan (35 `crates/*` + 3 app + 3 tool) |
| F1, F2, F3, F6, F8 | I dalje tocan |

## Sto stvarno postoji

| Dio | Stanje |
| --- | --- |
| Workspace | 41 clan; 35 crateova; 36 module contracta; 5 application manifesta |
| Project | Zamrznut; freeze scope prazan u `git status` |
| Ingest | Standalone + adapter + komponenta + store; Select u `selection.rs` |
| Shell | Factory mapa `desktop_entry` -> Project i Ingest adapter |
| Work settings | `query_only` / `SQLITE_OPEN_READ_ONLY`; nema INSERT/UPDATE u `src/` |
| Dir Browser | Dva sessiona: stari lokalni + `TransportBrowserSession` ako postoji config. Cancel i `path_for_uri` za registry i dalje idu starim lokalnim sessionom. |
| Source / probe / records | Spojeni na Odaberi; Camera pa Final; bez probe retrya |
| Ingest registry | `record_source_selection` pise |
| Ingest content | Shema i javni pogledi; nula INSERT-a u `clips` / `probe_records` |
| Media records | Odvojena datoteka; Select pise; ima `public_media_*` |
| UI kartice | Memorija (`Event::Clip`); `INGEST_RELOAD` ne ucitava clipove iz baze |
| Import | Odbijen |
| Filmstrip / Wave / Player / Timeline / Export / Monitor / Media Browser | Samo ugovori |
| Media Assist / Story | Samo contracti; `read_database_contracts` = `ingest_content` |
| Camera catalog | Select koristi `camera-patterns-2026.09.07.1.sqlite`. Stariji `camera-patterns-v1.sqlite` ostaje u testovima. |
| Media probe | Local: in-process `Executor`. Remote: `Client::connect`. Contract kaze OOP helper. Nema retrya. |

## Select call chain

Nije se promijenio:

```text
Odaberi (INGEST_DIR_CONFIRM)
  -> transport_browser.selected(uri)   (bez toga odbijeno)
  -> load_work_settings (read-only)
  -> confirm_source_selection
       -> IngestStore::record_source_selection
       -> selection::run
            -> SourceReader
            -> camera-patterns.read_uri
            -> scanner.scan_roles
            -> source-index-db.write
            -> sony-metadata + thumbnail (source-reader + image-assets)
            -> media-record-db Camera ili citanje postojeceg snapshota
            -> media-probe (acquisition claim, jedan pokusaj)
            -> ffprobe-metadata.read
            -> media-metadata-compose + media-record-db Final
            -> Event::Clip u IngestViewModel
```

Postojeci Final snapshot se cita bez novog probea. `INGEST_RELOAD` samo
osvjezava radne postavke. Ponovni Odaberi moze ponovno objaviti kartice
iz `media_records`; to nije hidracija pri pokretanju.

## Prioritetni nalazi

### F1 - P1: Javni Ingest clip/probe katalog nije napunjen (potvrdeno, ostrenje)

Pravila: AGENTS 4, 5, 6.

Select pise `source_index` i `media_records`. To nisu iste datoteke kao
`ingest_content`. `qnc-ingest-store` i dalje samo kreira `clips` /
`probe_records`. Nema INSERT-a u cijelom repou.

Story i Media Assist deklariraju citanje `qnc://local/db/ingest_content`,
ne `media_records`. Portable `media_records` baza stoga nije ugovoreni
katalog za te aplikacije.

UI lista zivi u `view.clips`. Restart ostavlja prazan prikaz.

### F2 - P1: Uvezi nije implementiran (potvrdeno)

`INGEST_IMPORT_SELECTED` vraca "Media import jos nije implementiran."
`ClipView.imported` ostaje `false`.

### F3 - P1: Select ovisi o transport bindingu izvan repoa (potvrdeno)

Nema `ingest-transport.json` u repou. Bez nje ili
`QNC_INGEST_TRANSPORT_CONFIG` nema `transport_browser`; Odaberi pada.
LAN/Internet tada ostaju "nije povezan". Zivi LAN nije verificiran.

### F4 - P2: AGENTS §16 je unutarnje proturjecan; docs/01 i docs/33 zastarjeli

§16 sada točno opisuje Select (docs/34, docs/35), a sljedeci bullet i
dalje kaze da scanner, camera detector i Media Probe ne smiju biti u
Ingest manifestu dok ne postoje. Manifest ih ima i kod ih zove.

docs/01: "Modul jos nije povezan s Ingest Odaberi runtimeom" za
source-index i media-records. To je netocno.

docs/33: "Select jos nije povezan s ovim modulima." To je netocno.

### F5 - zatvoreno: `image-assets` je u manifestu

`ingest.application.json` nabraja `qnc.module.image-assets`.
`selection.rs` zove `decode_thumbnail`.

`qnc.module.media-metadata` ostaje u manifestu bez izravnog importa u
komponenti (dolazi preko compose / record-db).

### F6 - P2: Thumbnails nisu trajni; poster approve UI je spojen, dispatch nije

`thumb_image` je `#[serde(skip)]`. Decode je u Select workeru iz kartice,
ne iz baze. Ponovni Odaberi moze ponovno procitati sliku s izvora.

`INGEST_APPROVE_PROXY_POSTERS` postoji u katalogu i u desktop gumbu
(`widgets.rs`). `dispatch` nema ruku; pada na "komponenta jos nije spojena".
Isto za `INGEST_TOGGLE_AUDIO_LANE`.

### F7 - P2: Sitni ugovorni driftovi (dio zatvoren)

Zatvoreno: source-reader verzije.

Ostaje:

- `qnc-media-records` / `qnc-media-record-db` Cargo `0.1.0`, contract `0.2.0`.
- `qnc-media-records` nema vlastite unit testove.
- Conformance `runtime_crate_for_module` i dalje nema
  `source-contract`, `source-index-contract`, `json-transport`,
  `media-records`, niti `application-catalog` (mapiran je samo reader).

### F8 - P3: Media Assist / Story i media UI moduli nisu krenuli (potvrdeno)

Tocno po AGENTS 13. Nema crateova. Shell footer samo Project i Ingest.

## Granice koje drze

- Nema `qnc-project*` Cargo ovisnosti u Ingest crateovima.
- Work settings: samo SELECT, `query_only`.
- Moduli nemaju `allowed_applications`; `qnc-contracts` to odbija.
- Produkcijski ffprobe spawn samo u `qnc-media-probe/src/process.rs`.
- Acquisition ledger + Select ne retryaju failed/uncertain pokusaj.
- Proxy nije zaseban clip.
- Forma salje `action_id`; ne zove scanner/probe.
- Project freeze scope prazan.
- Javni Dir Browser identitet ostaje QNC URI.

## Conformance

`cargo run -p qnc-conformance` na ovom HEAD-u: **all checks passed**.
Alat ne hvata prazan `ingest_content`, proturjecje §16, ni Story/MA
read ugovor naspram stvarnog Select proizvoda.

## Provjereno

- HEAD `9a09e93`, cist `git status`, crate/contract brojevi.
- Re-score svih nalaza iz docs/36.
- Select call chain, reload, import, transport gate.
- Ingest store vs `ingest_content` INSERT.
- Story/Media Assist `read_database_contracts`.
- AGENTS §16, docs/01, docs/33.
- Work settings write surface, Ingest->Project crate, ffprobe spawn.
- Freeze scope, shell factory, camera catalog `2026.09.07.1`.
- Poster-approve UI vs dispatch.
- Conformance.

## Nije provjereno

- Live Sony kartica u ovom prolazu.
- Zivi LAN/Intranet endpoint.
- Prisutnost `qnc-media-probe` exe uz desktop deployment.
- Pun `cargo test --workspace`.
- UI/layout doslovnost naspram `qnc_v4` nakon thumbnail painta.

## Sljedeci rizik

Isti kao u docs/36, sada jasniji: `media_records` je napunjen, a ugovoreni
citacki katalog kasnijih aplikacija nije. Pokretanje Filmstrip/Player ili
importa prije odluke sto je javni clip zapis daje drugi katalog ili drugi
probe.

Predlozeni redoslijed, bez implementacije:

1. Ukloniti proturjecje u AGENTS §16 i netocne recenice u docs/01 i docs/33.
2. Dogovoriti javni clip/probe zapis (`ingest_content` ili formalni read
   ugovor nad `media_records`) i uskladiti Story/Media Assist manifeste.
3. Tek onda import, zatim filmstrip/wave, zatim player.
4. Primjer `ingest-transport.json` bez credentiala u seed/docs.
)
