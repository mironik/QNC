# Audit nakon popravaka: kod, funkcija, preporuke

Datum: 2026-09-12. Kod nije mijenjan u ovom koraku. Mjerilo: `AGENTS.md` i
korisnicko pravilo (forma = UI; uske univerzalne javne komponente).

Ovo je provjera **sto su popravci stvarno zatvorili** naspram docs/81, plus
funkcionalni rez i redoslijed ispravaka. Nije novi zakon.

## Zakljucak

Ispravak 2026-09-12 (ponovno citanje `AGENTS.md` prije ocjene): filmstrip
**13 sličica** je zakon (§8). Kod s `FILMSTRIP_FRAME_COUNT = 13` je
usklađen. v4 ima 14; to **nije** naredba za novi QNC. Prethodna stavka
"vrati na 14" je pogreška audita, ne koda.

Ukupna ocjena cijele aplikacije (Ingest + spojeni moduli + shell host):
**B- / 3.5 od 5** (docs/81 je bio **C / 2.8**).

Ocjene po sloju (isti zakon):

| Sloj | Ocjena | Mjerilo AGENTS |
| --- | --- | --- |
| Ingest forma | 4.2 | §10, preambula: samo UI |
| Ingest application | 1.8 | preambula + §2: nije umbrella |
| Javni generatori / player-input | 4.5 | §8, §12: bez ingest crate |
| ingest-select | 2.5 | W smije, M ne |
| Broadcast Player | 2.8 | §8.2 / §8.3 nije zatvoren |
| Import / LAN izvor | 1.0 | produkt Ingesta / §9 |
| Shell host | 3.8 | §3; factory jos 2 adaptera |
| Project | zamrznuto | §14 |

Korak A iz docs/81 je **zatvoren u runtime ovisnostima**.
`cargo tree -p qnc-filmstrip-worker -p qnc-wave-worker -p qnc-player-input`
(normal/runtime, offline) **nema** `qnc-ingest-*`. Worker crateovi vise ne
znaju Ingest ime u izvornom kodu.

Glavni preostali kvarovi nisu vise "worker vuče ingest-store", nego:

1. `qnc-ingest-application` i dalje je umbrella (~1680 linija `lib.rs` plus
   playback / artifacts / guard). Preambula + §2.
2. `PlaybackGuard` je Ingest-privatan i veze se na `INGEST_*` action listu.
3. `qnc-ingest-select` je i dalje debeo W (~2900 linija u `lib.rs`).
4. Import je stub. LAN/Internet browser je stub. Broadcast Player nije
   live prihvacen (§8.3).
5. Conformance jos dopusta `qnc-ingest-store` kao runtime ovisnost
   `qnc-player-input` (stvarni Cargo.toml je samo `dev-dependencies`).

Ciljani `cargo test` (offline) za `qnc-filmstrip-worker`, `qnc-wave-worker`,
`qnc-player-input`, `qnc-ingest-application` i `qnc-timeline`: **67 proslo,
0 palo**. Funkcionalna ocjena Playa ostaje iz docs/62–76 plus trenutni kod,
ne iz novog live Mira.

## 1. Sto je popravljeno (docs/81 -> sada)

| Stavka docs/81 | Stanje 2026-09-12 |
| --- | --- |
| Worker `Cargo.toml` vuče `qnc-ingest-select/store/work-plan` | **Zatvoreno.** Filmstrip/wave worker: decoder, media-*, transport, dir-browser. Nema ingest crate. |
| `qnc-player-input` runtime `qnc-ingest-store` | **Zatvoreno.** Store je samo `dev-dependencies`. Trait `PlayerContentRead`. |
| Worker javni API s Ingest imenom | **Zatvoreno.** Nema `Ingest` u `qnc-filmstrip-worker` izvoru. |
| `qnc-ingest-components` / `IngestComponent` | Ostaje zabranjeno u conformance. Crate nije u stablu. |
| LinearScan u filmstrip crateovima | Nije pronadjen. |
| HTTP za filmstrip frameove | Nije u workeru. |
| AGENTS §16 zastario (Close project; Filmstrip u manifestu) | **Azurirano 2026-09-12.** Close project + uvjetovani Filmstrip/Wave/Player u manifestu. |
| PlaybackGuard | Postoji; siri nego u docs/79. I dalje Ingest-privatan. |
| Forma pasivna | Drzi se: desktop nema store/ffprobe/Command. |

## 2. Kodni audit

### 2.1 Univerzalnost crateova

Klasa: **U** univerzalan, **W** aplikacijski workflow, **O** owner, **A**
sastav/forma, **M** monolit.

| Crate | Klasa | Ocjena | Napomena |
| --- | --- | --- | --- |
| qnc-timeline, player-timeline, ui-kit, dir-browser, keyboard, work-settings | U | 5 | Isti API za svaku formu |
| qnc-filmstrip, qnc-wave, wave-view, timeline-assets | U | 4.5 | Trait reader |
| **qnc-filmstrip-worker** | **U** | **4.5** | Trait `FilmstripContentRead/Write`. `dir-browser` samo za volume serial. |
| **qnc-wave-worker** | **U** | **4.5** | Isto |
| **qnc-player-input** | **U** | **4.5** | Trait `PlayerContentRead`. Store samo u testovima. |
| player-client/launcher/engine/decode | U | 4 | Play nije live zatvoren |
| qnc-ingest-work-plan | W lose ime | 3 | Projektni raspored pod Ingest imenom |
| qnc-ingest-catalog | W | 4 | Samo Ingest katalog |
| qnc-ingest-select | W + M | 2.5 | Scan+probe+vise DB u jednom `lib.rs` |
| qnc-ingest-store | O | 4 | Ispravno owner; implementira traitove u application adapteru |
| **qnc-ingest-application** | **A + M** | **1.8** | Dispatch + Select + thumbs + player + FS + wave + guard |
| qnc-ingest-desktop | A forma | 4.2 | Paint + action_id |
| qnc-project-* | zamrznuto | — | Nije dirano |

Cetiri slicna porta (`PlayerContentRead`, `FilmstripContentRead`,
`WaveContentRead`, `TimelineArtifactRead`) su varijacija. Nisu blokada
univerzalnosti. Kasnije ih spojiti u jedan uski content port samo ako to
ne napravi novi monolit.

### 2.2 Application umbrella

`IngestApplication::dispatch` i `poll` orkestriraju:

- radne postavke i work plan
- dir browser i Select
- katalog / selekcija
- thumbnail load (gasi ga Guard)
- player prepare / play_when_ready
- filmstrip/wave sync i `playback_priority`
- Import stub

To je ista greska kao stari `components` crate, drugo ime. Story ne smije
ovisiti o ovom crateu. Nova forma ne smije copy-pasteati ovaj `lib.rs`.

Adapteri u `timeline_artifacts.rs` i `playback.rs` su **tocan smjer**
(owner implementira trait). Ostaju zakljucani unutar application cratea.

### 2.3 PlaybackGuard

Aktivacija: `play_when_ready || preparing || playing`.

Blokira Ingest akcije: reload, source kind, dir, select-all/clear/toggle,
approve proxy posters. Gasi thumbnail poll. Odgada artifact DB sync.
Postavlja `playback_priority` na workere.

Ne blokira po capabilityju. Story mora prepisati listu. To nije javni modul.

### 2.4 Filmstrip broj sličica

`AGENTS.md` §8: "Filmstrip ima 13 sličica." Kod: `FILMSTRIP_FRAME_COUNT = 13`.
**Uskladjeno.** v4 `FILMSTRIP_FRAME_COUNT = 14` je stari ugovor; zakon
novog QNC-a ga nadglasava. Ne dirati broj bez izricite izmjene `AGENTS.md`.

### 2.5 Conformance rupa

`tools/qnc-conformance/src/player_boundary.rs` i dalje tretira
`qnc-ingest-store` kao dozvoljenu runtime ovisnost `qnc-player-input`.
Kod je ispred testa. Test treba zabraniti store u `[dependencies]`,
dopustiti samo `[dev-dependencies]`. Dodati istu zabranu za
filmstrip-worker i wave-worker (docs/81 korak F).

`docs/04-module-contract-matrix.md` je i dalje skica 2026-09-04.

### 2.6 Manifest vs §16

`ingest.application.json` deklarira Filmstrip, Wave, Timeline, player,
scanner, probe. To sada pokriva azurirani §16 **ako** moduli postoje u
runtimeu. Stavka §16 i dalje kaze da Select rez **ne znaci** da su
kopiranje, filmstrip, wave, playback implementirani — to je djelomicno
zastarjelo: generatori i player **jesu** spojeni; import/kopiranje nisu.

## 3. Funkcionalni audit

Provjereno iz koda i ugovora, ne iz novog live Mira.

| Tok | Stanje | Dokaz |
| --- | --- | --- |
| Aktivni projekt + radne postavke read-only | Radi u sastavu | `SettingsReader`, work-plan; nema Project crate |
| Local Dir Browser | Radi | javni `qnc-dir-browser` |
| LAN / Internet izvor | Stub poruka | `LAN izvor nije povezan u ovom rezu.` |
| Select / jedan probe | Implementiran u select | manifest capability `media.probe.full` samo na Ingestu |
| Katalog + New/All filter | Implementiran | view filter, bez novog scana |
| Thumbnails s kartice | Implementirani; pauza za Play | Guard gasi `poll_thumbnails` |
| Preview / nova sesija na klik | Implementirano | `stop_player` prije prepare |
| Play / Pause / step / cue | Spojeno na player-client | nije live prihvaceno |
| Timeline pasivan | Drzi se | forma zove paint + artifacts; nema sata |
| Filmstrip generate | Worker + JPEG + DB veza | 13 sličica po §8; uskladjeno |
| Wave generate | Worker + peaks u content DB | v4 obrazac, ne wave.db |
| Import / kopiranje medija | **Nije** | `"Media import jos nije implementiran."` |
| Close project u shellu | Javni modul | `qnc-project-close`; Project freeze |
| Shell factory | Project + Ingest | sljedeca app ne smije ici if/switch uz puni crate |

Broadcast Player (docs/62–76, kod engine): clock iz WASAPI kad ima audio;
mmap + TCP kontrola; underrun / convert budget i skip-on-latest i dalje
cine Play **neprihvacenim**. Guard smanjuje konkurenciju thumb/Select, ali
ne liječi decode/convert.

## 4. AGENTS uskladjenost (kratko)

| Pravilo | Rez |
| --- | --- |
| Forma = UI | Uskladjeno |
| DB-first, nema app-to-app | Uskladjeno |
| Jedan probe | Uskladjeno |
| Worker bez ingest crate | **Sada uskladjeno** |
| Nema umbrella | **Prekrsaj**: application |
| Filmstrip 13 sličica (§8) | **Uskladjeno** |
| §8.3 Play prije FS/Wave | Integracija postoji; §16 to sada ogranicava |
| Project freeze | Nije diran |
| Import kao produkt Ingest | Nije implementiran |

## 5. Preporuke — cvrsti redoslijed

Ne raditi sve odjednom. Ne preimenovati application crate.

### P1 — zatvoriti korak A u conformance

Zabraniti `qnc-ingest-store`, `qnc-ingest-select`, `qnc-ingest-work-plan`
u runtime `Cargo.toml` za `qnc-filmstrip-worker`, `qnc-wave-worker`,
`qnc-player-input`. Store u player-input samo kao dev-dep.

### P2 — javni PlaybackPriority (docs/81 korak B)

Uski javni modul: `active` iz player view + queued play; `set_priority`
na filmstrip/wave/thumbnail; blokada po capabilityju, ne po `INGEST_*`.
Ingest samo prosljedjuje. Forma ne zna za Guard. Engine ne prima Guard.

### P3 — smanjiti application (korak C)

Nakon P2: `IngestApplication` = handle servisa + `dispatch`/`poll` +
slozeni view. Thumbnail, Select, browser, player, FS, wave ostaju
klijenti javnih servisa, ne privatna polja s logikom u istom `lib.rs`.

### P4 — cijepati ingest-select (korak D)

Orkestrator zove postojece U module. Worker i dalje ne uvozi select.

### P5 — Play live §8.3, zatim Import

Mironik 2002/2679, Pause/Play, seek, promjena klipa, A/V mjerenje.
Tek onda `ingest_import_selected`. LAN/Internet nije uvjet za Import.

### P6 — zakon / docs

Poravnati §16 recenicu o "filmstrip/wave/playback nisu implementirani".
Osvjeziti `docs/04`. Korisnik odlucuje ostaje li §8.3 strogi redoslijed
ili ostaje §16 iznimka.

### Ne raditi

- Novi `qnc-ingest-runtime` / `core` / `host`
- Story ovisnost na `qnc-ingest-application`
- Guard u formi ili u broadcast-engine
- Otvaranje Projecta
- LinearScan fallback
- Drugi filmstrip loader
- Local default umjesto projektnih postavki
- HTTP za filmstrip frameove

## 6. Sto nije provjereno

- Live Play uz Guard (Mironik).
- Vizualni UI 1:1 s v4 (nije bio predmet ovog reza).
- Project desktop linija-po-linija (freeze).
- Sve camera sheme u Selectu.
- LAN/Intranet player transport (lokalni mmap nije dokaz).
