# Freeze cijele QNC obitelji

Status: zamrznuto

Zatvoreno ograniceno odobrenje 2026-09-13 uz docs/85: Ingest startup cita
aktivni projekt iz baze, ne Dir Browser korijene.

Datum: 2026-09-13

Razlog: korisnik je izricito zatrazio da se zamrzne cijeli projekt sa svim
modulima i aplikacijama i da se to stanje posalje na GitHub.

## Odnos prema starijim freezeovima

- Project: `AGENTS.md` odjeljak 14 i `docs/11-project-freeze.md`.
- Ingest i moduli koje Ingest koristi: odjeljak 17 i `docs/85-ingest-freeze.md`.
- Ovaj zapis zatvara sve ostalo: shell, preostale crateove, alate, ugovore
  i razvojne dokumente. Ne slabi 14 ni 17.

## Opseg: kod i ugovori, ne radni podaci

Freeze se odnosi na izvorni kod, manifeste, module/application/DB/UI
ugovore, seed, conformance i razvojne dokumente. Ne odnosi se na radne
projektne direktorije, baze ni artefakte. Ti se i dalje smiju zapisivati
kroz vec zamrznute javne write putove.

## Ne smatra se dozvolom

- "nastavi", "idemo dalje", "sredi QNC", "dodaj Story"
- otvoreni `AGENTS.md` odjeljak 8.3
- preview/player/trzaj kao razlog za izmjenu bez imenovanog otkljucavanja

Dozvola mora imenovati tocnu aplikaciju, modul ili putanju i vrstu promjene.

## Zamrznuti scope

Sve ispod root-a `C:\Users\miron\Projects\QNC` sto je razvojni kod:

- `apps/**` ukljucujuci `qnc-app`, `qnc-project`, `qnc-ingest`
- `crates/**`
- `tools/**`
- `contracts/**`
- `seed/**`
- `docs/**` (osim novog freeze/unlock zapisa na izriciti zahtjev)
- root `Cargo.toml`, `Cargo.lock`, `AGENTS.md` (osim freeze/unlock zapisa
  na izriciti zahtjev)

`qnc-ingest-components` ne smije postojati.

## Dopusteno bez otkljucavanja

- citanje i audit
- pokretanje testova i live programa
- zapis poslovnih rezultata kroz postojeci javni write put

## Postupak otkljucavanja

1. Zaustaviti implementaciju.
2. Navesti tocne datoteke.
3. Tražiti izricitu korisnicku dozvolu.
4. Nakon odobrene promjene ponovno vratiti status na zamrznuto ovdje i u
   `AGENTS.md` odjeljku 18.

## Zatvoreno ograniceno odobrenje 2026-09-18

Otkljucana je samo `crates/qnc-broadcast-engine/src/av_sync.rs`, jedna
promjena: `#![cfg(test)]` na pocetku datoteke, da `qnc-conformance`
(player boundary) prepozna test-only kod. Bez promjene ponasanja.

Verificirano: `qnc-conformance` prolazi sve provjere. Test
`recorded_player_log_picture_must_not_lag_sound` cita lokalni
`data/diagnostics/player.log` i pada na tim podacima jednako i bez ove
promjene; to je zaseban nalaz o A/V pomaku, nije dio ovog odobrenja.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (drugo)

Otkljucana je samo `crates/qnc-broadcast-engine/src/av_sync.rs`, jedna
promjena: `#[ignore]` s razlogom na testu
`recorded_player_log_picture_must_not_lag_sound`. Test cita lokalni,
netrackirani `data/diagnostics/player.log` i hvata stare sesije, pa nije
deterministicki. Analiza loga: pomak slike od zvuka (do -21 kadra) potjece iz
starih sesija; zadnje sesije imaju -1..0 kadra. Test se i dalje pokrece s
`--ignored`.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (trece)

Otkljucana je samo `crates/qnc-dev-diagnostics/src/lib.rs`, funkcija
`log_line`: cijela linija se pise jednim `write_all`. Prije su se linije vise
procesa isprepletale u `player.log`. Format se nije promijenio.

Verificirano stresom (4 procesa x 4 niti, 48000 linija): stara verzija 37259
neispravnih linija, nova 0. Testovi crate-a i `qnc-conformance` prolaze.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (cetvrto, korak 1)

Forma Project vise ne ovisi o `qnc-project-store`. `ProjectComponent` je
premjestena u novi tanki crate `qnc-project-application` (composition root, bez
UI-ja), a odabir aplikacija po prioritetnim grupama u zaseban javni modul
`qnc-application-selection` s ugovorom `application-selection.module.json`.
Pravila Project granice u `qnc-conformance` sada traze: desktop ovisi o
`qnc-project-application`, a ne o storeu ni rusqlite.

Put zapisa lanca aplikacija je nepromijenjen: komponenta predaje snapshot
storeu, store ga sprema u `workspace/application_selection`.

Verificirano: conformance, testovi (selection 4, store 36, desktop 8),
gradnja `qnc-project`/`qnc-app`/adapter, zivi smoke `qnc-project`.

Ostaje otvoreno (korak 2): poslovna logika export presetova u
`crates/qnc-project-desktop/src/project_advanced.rs` i 8 testova u desktop
crateu (§10). Zahtijeva novo otkljucavanje.

Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (cetvrto, korak 2)

Forma Project vise ne sadrzi poslovnu logiku ni testove (§3, §10).

- `qnc-settings-path` (novo, javni modul): JSON putanje postavki.
- `qnc-export-preset` (novo, javni modul): export presetovi (ugradjeni katalog
  `contracts/export_profiles.json` + custom presetovi u postavkama).
- `qnc-project-application::selected_project_label` s testom.
- Ugovori `settings-path.module.json`, `export-preset.module.json`.
- `qnc-conformance`: nove provjere `Project form has no tests` i `Project layout
  and shortcut reference` (asercije koje su bile u testovima forme, sada citaju
  ugovore izravno).

Uklonjeno bez zamjene, jer testovi nisu ispitivali kod, nego ponavljali
implementaciju ili vlastiti literal: `opened_folder_rows_share_breadcrumb_column`,
`short_path_keeps_short_values`, `root_disk_entries_are_public_browser_entries`
(sve u `location_browser.rs`; dostupni u git povijesti).

Verificirano: conformance, `cargo check --workspace`, testovi (selection 4,
export-preset 2, settings-path 1, project-application 1, store 36), zivi smoke
`qnc-project`.

Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (peto)

Dodan je samo dokument `docs/87-editorial-ui-reference-audit.md`: snimka v4
rasporeda Story / Media Assist i mapa pasivnih komponenti za planirane
aplikacije po grupama e, g, l, o. Nema izmjena koda ni ugovora.

Odluke korisnika: desni panel ostaje prazan (prostor za funkcije pojedine
grupe), a Ingest se ne dira; nove komponente gradit ce se izravno iz v4
reference.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (sesto)

Dodan je ugovor `contracts/ui/editorial.layout.json`: zajednicka geometrija
editorial shella (ista kao `ingest.layout.json`, v4 `qnc_ui::space`) i
kompozicija po grupama e, g, l, o. Desni panel za e, g, l je prazan, za o je
`segment_panel`. U `tools/qnc-conformance` dodana je provjera `Editorial layout
composition` (kompozicija po grupama i podudarnost geometrije s Ingest
ugovorom). `ingest.layout.json` i Ingest nisu diranji.

Verificirano: conformance prolazi; mutacijski test (izmjena `left_ratio` i
desnog panela grupe e) daje FAIL, nakon vracanja prolazi.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (sedmo)

Tri nova javna pasivna modula iz v4 reference, bez diranja Ingesta:

- `qnc-editorial-shell`: shell (lijevi stupac, razdjelnik, desni panel), preview
  monitora, content panel.
- `qnc-media-pool-head`: tabovi All / Virtual / B-roll / Segment i transport
  (`>`, `[`, `]`, `B`, Export HI-res); vraca jedan neutralni intent.
- `qnc-media-card`: kartica i virtualizirana mreza, bez imenovanih presetova po
  aplikaciji; zastavice (`selection_check`, `status_dots`) dolaze iz ugovora.

Ugovori: `editorial-shell`, `media-pool-head`, `media-card` u
`contracts/modules/`. Moduli ne drze stanje i ne znaju aplikaciju; boje i mjere
dobivaju od forme (iz `editorial.layout.json`).

Verificirano: testovi (4 + 4 + 7), `cargo check --workspace`, conformance. Moduli
jos nisu spojeni ni u jednu formu.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (osmo)

Dodan je samo probni prozor `crates/qnc-editorial-shell/examples/editorial_demo.rs`
(s dev-ovisnostima) koji crta `qnc-editorial-shell`, `qnc-media-pool-head` i
`qnc-media-card` s lazniim podacima. Kompozicija dolazi iz
`contracts/ui/editorial.layout.json` za odabranu grupu (e, g, l, o), boje i
mjere iz `contracts/ui/shell.layout.json`. Pokretanje:

    cargo run -p qnc-editorial-shell --example editorial_demo -- e

Nema aplikacije, baze ni manifesta. Vrijednosti `preview_black` i `select_red`
u demou su privremene (ugovor ih imenuje, ali ne daje vrijednost).

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (deveto)

Dodan je javni pasivni modul `qnc-source-dock` (ugovor `source-dock.module.json`),
blok `source_dock` i `actions_rtl` po grupama u `editorial.layout.json`, provjera
podudarnosti mjera docka s Ingest ugovorom u `qnc-conformance`, te dock s
timelineom u probnom prozoru `editorial_demo`. Referenca za raspored docka je
Ingest layout (odluka korisnika); Ingest nije diran.

Verificirano: test modula, conformance, smoke probnog prozora (e, o).

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (deseto)

Dodan je samo dokument `docs/88-qnc-v4-layout-extract.md`: tocne mjere, boje i
izracuni iz qnc_v4 (teme, chrome, shell, preview, glava pool-a, kartica, dock,
timeline, filmstrip, location browser, form kit, Story paneli) i popis razlika
prema novom QNC-u. Nema izmjena koda ni ugovora. Ispravak boje playheada u
`qnc-timeline` je zabiljezen kao nalaz i trazi zasebno otkljucavanje.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (jedanaesto)

Novi crate forme `qnc-editorial-desktop`: Ingest layout i UI kopirani 1:1
(`theme.rs`, shell, preview monitor, glava pool-a, dock s timelineom), bez
desne mreze klipova i izbora direktorija (to mjesto je prazno, kasnije izbor
klipova). Forma je pasivna: host popunjava `EditorialView` i prima
`EditorialIntent` (action_id iz keyboard ugovora, ili timeline intent). Nema
ovisnosti o Ingestu, Projectu, storeu, scanneru, probeu ni playeru.

Ugovor `editorial.layout.json` dobio je `pool_head` (tabs_left, transport_right)
i `source_dock.clip_label_fallback`, iste vrijednosti kao `ingest.layout.json`.
Dodana provjera `Editorial form boundary` (bez testova u formi, bez zabranjenih
ovisnosti). Probni prozor: `cargo run -p qnc-editorial-desktop --example
editorial_form -- e`.

Nije ukljuceno: tipkovnicki precaci (scope u keyboard ugovoru), gumbi docka su
prikazani ali onemoguceni, desni panel je prazan.

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-18 (dvanaesto)

Aplikacije grupa e, g, l, o dodane su u shell desktop istim putem kao Ingest:

| Grupa | Aplikacija | App crate / exe | Tab | desktop_entry |
|---|---|---|---|---|
| e | MA Audio AI | `qnc-media-assist-audio-ai` | `media_assist_audio_ai` | `qnc_media_assist_audio_ai` |
| g | MA Audio | `qnc-media-assist-audio` | `media_assist_audio` | `qnc_media_assist_audio` |
| l | MA Video | `qnc-media-assist-video` | `media_assist_video` | `qnc_media_assist_video` |
| o | Story | `qnc-story` | `storyboard` | `qnc_story` |

Svaka ima `qnc-app.json`, samostalni exe i javni desktop adapter
(`*-desktop-adapter`); shell ovisi samo o adapterima i ima 4 unosa u tablici
tvornica. Sve koriste zajednicku pasivnu formu `qnc-editorial-desktop`
(`EditorialApp`). Novi ugovori aplikacija: `media-assist-audio-ai`,
`media-assist-audio`, `media-assist-video` (Story vec postoji) i deklarativni
ugovori baze (`tables` prazne, `public_read_policy` `owner_only`): sheme se
definiraju prije stvarnog rada aplikacija (§13).

Katalog aplikacija je osvjezen alatom `qnc-app-catalog refresh apps target/debug
data/application-catalog.json` (`data/` nije pod gitom; prethodni katalog je
spremljen izvan repozitorija).

Odobrenje je zatvoreno. Status: zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-19 (trinaesto)

Ingest, nalaz 2 iz audita: playback guard je blokirao odabir klipa tijekom
pripreme ili reprodukcije playera ("Zaustavi Broadcast Player prije ove radnje").
Popravak u `crates/qnc-ingest-application`:

- `playback_guard.rs`: `blocks_action` vise ne sadrzi `ingest_clip_toggle`,
  `ingest_select_all` i `ingest_clear_selection`.
- `lib.rs`: `select_clips` vise ne provjerava guard.
- Test `playback_guard_blocks_background_source_work` sada provjerava
  `ingest_reload`; dodan `clip_selection_is_not_blocked_by_the_playback_guard`
  (pada na starom ponasanju).

Razlog: odabir pise samo oznaku `selected` kroz serijalizirani write transport,
ne dira izvor, scan ni thumbnailove. Ostaje blokirano: ponovno citanje, promjena
izvora, direktoriji, generiranje postera. Guard je sužen, ne prosiren.

Odobrenje je zatvoreno. Status: zamrznuto.

Ispravak zapisa (trinaesto): gradnja je provjerena za `qnc-ingest`. Shell
`qnc-app` nije ponovno izgradjen jer je exe bio zauzet pokrenutim procesom, pa
popravak u shellu (Ingest tab) tek nakon ponovne gradnje.

## Zatvoreno ograniceno odobrenje 2026-09-19 (cetrnaesto)

Preview monitor i source timeline u aplikacijama e, g, l, o spojeni na Broadcast
Player, popis klipova ispod monitora (virtualizirana mreza kartica). Novi javni
moduli bez ovisnosti o Ingestu i Projectu: `qnc-source-bindings`, `qnc-content-read`
(samo javni pogledi projektnog sadrzaja), `qnc-source-preview`; tanki composition
root `qnc-editorial-application`. Forma ostaje pasivna. Blokada pravila
"business app DB-only isolation" rijesena sesnaestim odobrenjem; conformance
prolazi. Odobrenje je zatvoreno. Status: zamrznuto.

## Povuceno ograniceno odobrenje 2026-09-19 (petnaesto)

Predlozeni prvi korak kartice klipa (`qnc-clip-status`, sličica i točkice) nije
izvrsen i nista nije otkljucano. Nakon audita (`docs/v5-book/08-audit-and-plan.md`)
opseg se prepisuje uz pravilo: UI je zakljucan, dopusteno je samo dodavanje.

## Zatvoreno ograniceno odobrenje 2026-09-19 (sesnaesto)

Pravilo provjere "business app DB-only isolation" (`tools/qnc-conformance`):
`[dev-dependencies]` tranzitivnih crateova se ne broje (ne povezuju se u
aplikaciju). Broje se i dalje runtime ovisnosti tranzitivnih crateova i
dev-ovisnosti samog provjeravanog cratea. Tri nova testa (dev tranzitivno ne
racuna, runtime tranzitivno racuna, vlastita dev-ovisnost racuna). Verificirano:
20 testova conformancea, pun conformance prolazi. Odobrenje je zatvoreno. Status:
zamrznuto.

## Zatvoreno ograniceno odobrenje 2026-09-19 (sedamnaesto)

Kamera kao samostalna komponenta po uzorku kataloga i pravilo probea (XML na kartici =
nikad probe; bez XML-a jednom). Novi crateovi `qnc-camera-adapter` i
`qnc-camera-sony-fx6-v6` (+ ugovori modula). `qnc-ingest-select`: popis adaptera dolazi
izvana; Declared kamera postaje Final bez probea. `qnc-ingest-application`: composition
root sastavlja registar. Verificirano: testovi select (14), application (27), adapter (4),
fx6 (2), conformance prolazi. Nisu mijenjani `qnc-media-records`, `qnc-media-metadata`,
`qnc-media-metadata-compose`. Otvoreno: `qnc-ingest-store::ready()` (Declared klip je Final +
Partial pa jos nije u redu za uvoz), odabir citaca po `pattern_id` u `qnc-scanner`,
generički adapter za kamere bez indeksa. Status: odobrenje otvoreno do rjesenja `ready()`.

Dopuna (sedamnaesto, zatvoreno): korisnik je izricito otkljucao i `crates/qnc-ingest-store`
(`content/mod.rs`: `ready()` i testovi). Uvoz je moguc kad je zapis Final i (Complete ili bez
ffprobe dokaza); probani nepotpun zapis i Camera faza ostaju blokirani. Testovi: store 26,
select 15, application 27, adapter 4 i 2; conformance prolazi. Nije izvedeno: odabir citaca
po `pattern_id` u `qnc-scanner`, genericki adapter za kamere bez indeksa (zasebna
odobrenja). Odobrenje je zatvoreno. Status: zamrznuto.
