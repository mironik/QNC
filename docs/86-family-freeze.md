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
