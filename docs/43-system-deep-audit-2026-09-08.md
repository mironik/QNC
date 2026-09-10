# QNC - dubinski audit cijelog sustava 2026-09-08

Datum: 2026-09-08.
Predmet: cijeli sustav na Git HEAD, cisto stablo.
Root: `C:\Users\miron\Projects\QNC`.
Referenca: `C:\Users\miron\Projects\qnc_v4`.
HEAD: `875d981` - `Persist incremental Ingest catalog and refine selection UI`.
Prethodni auditi: [docs/36](36-deep-audit-2026-09-07.md),
[docs/37](37-deep-audit-repass-2026-09-07.md).
Koracni zapisi od tada: docs/38-42.

Kod, AGENTS, Project i korisnicke baze nisu mijenjani ovim auditom.
Ovo je nalaz, ne novo pravilo i ne odobrenje sljedeceg koraka.

## Zakljucak

QNC je i dalje obitelj aplikacija, ne monolit. Shell hosta Project i Ingest
preko javnih adaptera. Project je zamrznut. Ingest vise nije samo Select u
memoriji: katalog, probe zapis i odabir zive u Ingest tablicama unutar
postojece `qnc_project.db`. Story i Media Assist jos nemaju crateove, ali
njihov read ugovor (`ingest_content`) sada ima sto citati.

To jos nije puni proizvodni lanac. `Uvezi` je stub. Filmstrip, Wave i
Broadcast Player imaju samo ugovore. Select i dalje trazi
`ingest-transport.json` izvan repoa. AGENTS §16 ostaje unutarnje proturjecan
za scanner/probe u manifestu.

Najvazniji sljedeci blokator nije novi media modul. Treba izvrsni import
koji primjenjuje vec zapisane projektne politike, zatim generatori koji
citaju javni `ingest_content`, bez drugog probea i bez odmrzavanja Projecta.

## Sto se promijenilo od docs/37 (HEAD 9a09e93)

| docs/37 | Sada (875d981) |
| --- | --- |
| `ingest_content` shema bez INSERT-a | INSERT u Ingest tablice unutar `{project}/qnc_project.db` |
| UI katalog samo u memoriji | Startup i `INGEST_RELOAD` hidriraju katalog iz baze |
| Globalni `data/ingest_content.db` kao stari katalog | Runtime ga vise ne stvara |
| F1 P1 prazan javni katalog | Zatvoreno za Select/reload/odabir |
| Story/MA citaju prazan ugovor | Ugovor isti; tablice se mogu napuniti |
| Filter / checkbox vs preview | Novi/Sve i odvojeni fokus implementirani (docs/41, 42) |
| Incremental Select | Uskladjuje razlike, ne prazni plocu (docs/40) |
| AGENTS 4.1 / 5.1 | Dodani; freeze = kod, ne radni direktoriji |

## Karta sustava

```text
QNC.app (shell)
  registry: apps/*/qnc-app.json
  factory:  qnc_project | qnc_ingest  (javni adapteri)

Project (zamrznut)
  pise: project_registry, project_workspace
  cita: vlastito
  standalone + embedded

Ingest
  cita: aktivni projekt + settings (qnc-work-settings, query_only)
  pise: ingest_registry;
        source_index + media_records (Select evidencija);
        ingest_content tablice u istoj qnc_project.db
  standalone + embedded

Media Assist / Story
  samo application + DB contract
  read: project_registry + ingest_content
  nema crate, nema qnc-app.json
```

| Sloj | Broj | Stanje |
| --- | --- | --- |
| Workspace clanovi | 41 | 35 crateova + 3 app + 3 tool |
| Module contracti | 36 | 28 s crateom/toolom; 8 samo ugovor |
| Application contracti | 5 | Project, Ingest, Shell, MA, Story |
| App registry | 2 | Project, Ingest |
| DB contracti | 8 | 4 ziva owner zapisa; MA/Story prazni |

Samo ugovor, bez cratea: filmstrip, wave, broadcast-player, export,
media-browser, monitor, timeline, test-adapter.

## Project

Freeze status: zamrznuto. `git diff` na freeze scopeu prazan je naspram HEAD.
Zadnja izmjena Project koda ostaje `0a46c1a` (2026-09-06).

`docs/11` i AGENTS 4.1/5.1 pojasnjavaju da freeze nije zabrana pisanja
vlastitih tablica u projektni direktorij. To nije odobrenje za izmjenu
Project crateova.

Ostaje tehnicki rizik iz docs/38: OS delete-lock / rekurzivni read-only
moze na Unixu skinuti write bitove i tako blokirati radni upis. Nije
mijenjan Project kod. Linux/macOS live upis Ingest tablica nije
verificiran ovim auditom.

## Shell

- Footer iz `apps/*/qnc-app.json`.
- Embedded factory: dva javna adaptera, lookup po `desktop_entry`.
- Nema `if desktop_entry == "qnc_project"` u render putu.
- `shell_next_group` i dalje salje samo Project adapter.
- Layout contract i dalje nabraja media_assist i storyboard; runtime ih
  ne prikazuje jer nema registracije.

To je poznato §16 odstupanje, ne novi monolit.

## Ingest

Call chain ostaje: Odaberi -> work settings -> registry zapis -> scan ->
source-index -> media-records (Camera/Final, jedan probe) -> pozadinski
`publish_batch` u `ingest_content`.

Novi sloj:

- `qnc-ingest-store::content` je owner adapter. URI
  `.../db/project_workspace/...` pretvara u `.../db/ingest_content/...`,
  a binding ostaje ista `qnc_project.db`.
- SQL authorizer dopusta upis samo u `clips`, `clip_sources`, `clip_proxy`,
  `probe_records`, `filmstrip_artifacts`, `wave_artifacts`.
- `catalog::load` hidrira kartice; thumbnail piksele ponovno cita s izvora
  preko spremljenog URI-ja.
- Incremental Select preskace Final zapise; nedostajuće uklanja samo nakon
  potvrdenog NotFound u scopeu.
- Novi/Sve je projekcija `previously_seen`, ne import status i ne DB upis.
- Checkbox i preview fokus su odvojeni.

`INGEST_IMPORT_SELECTED` i dalje vraca "Media import jos nije implementiran."
Store ima `queue_selected` / `claim_next` / `FinishImport` bez potrosaca
u komponenti.

Bez `data/ingest-transport.json` ili `QNC_INGEST_TRANSPORT_CONFIG` nema
`transport_browser`; Odaberi odbija. Datoteka je u `.gitignore`. U repou
nema primjera.

## Moduli i probe

- Produkcijski ffprobe spawn samo u `qnc-media-probe`.
- Local Select koristi in-process `Executor`; remote `Client::connect`.
- Nema retrya failed/uncertain acquisition.
- Proxy nije zaseban clip.
- Filmstrip/Wave shema postoji (`filmstrip_artifacts`, `wave_artifacts`);
  nema writera. To je priprema ugovora, ne implementacija generatora.
- Camera catalog: `camera-patterns-2026.09.07.1`.

## Story / Media Assist

Samo contracti. `read_database_contracts` i dalje
`project_registry` + `ingest_content`. Fizicki zapis sada moze postojati
u projektnoj bazi. Nema citaca, nema workflowa, nema shell taba.

## Dokumentacijski drift

- AGENTS §16 i dalje kaze da scanner/probe ne smiju biti u Ingest manifestu
  dok ne postoje. Manifest ih ima i kod ih zove. Media Browser / Filmstrip /
  Wave nisu u manifestu i to je točno.
- docs/01 i dalje tvrdi da source-index i media-records nisu spojeni na
  Odaberi.
- docs/00 i dalje crta Ingest kao korisnika Filmstrip/Wave modula i
  "Ingest DB" kao zasebnu sliku, bez zajednicke projektne datoteke.
- docs/33 "Select jos nije povezan" je zastario.

## Prioritetni nalazi

### S1 - P1: Uvezi nije izvrsen

Intent postoji. Store ledger postoji. Komponenta odbija. Radne politike
(`storage.ingest_media`, proxy/original) vec su u projektnoj bazi.
Bez importa nema kopiranih/povezanih medija niti `imported = true`.

### S2 - P1: Filmstrip / Wave / Player nemaju runtime

Po AGENTS 13 to je ispravan redoslijed dok nije bilo kataloga. Katalog
sada postoji. Generator i player i dalje ne smiju raditi drugi probe.
Prazne artifact tablice nisu proizvod.

### S3 - P1: Select ovisi o transport bindingu izvan repoa

Isto kao docs/37 F3. Local/LAN dijele SourceReader API; zivi LAN nije
verificiran. Bez configa pad na stari Dir Browser i placeholder LAN.

### S4 - P2: §16 proturjecan; ownership docs zastarjeli

Kod je ispred docs/01 i dijela §16. Pravila 1-15 i 4.1/5.1 vrijede.

### S5 - P2: Conformance ne vidi semantiiku content adaptera

`all checks passed`. Alat ne usporeduje `schema.sql` s contract tablicama,
ne hvata prazan import, niti `media-records` / `json-transport` koji se
koriste a nisu u Ingest `module_dependencies`.

### S6 - P2: Poster approve i audio lane nisu spojeni

Katalog i gumb postoje. `dispatch` pada na "nije spojena".
Thumbnail je URI-trajan, ne pixel-trajan.

### S7 - P2: MultiOS upis u zakljucani projektni dir nije dokazan

Windows live je radio uz delete-deny ACL i PERSIST journal.
Unix write-bit rizik iz docs/38 ostaje otvoren.

### S8 - P3: Sitni driftovi

- `qnc-media-records` / `qnc-media-record-db` Cargo `0.1.0` vs contract `0.2.0`.
- Hydrate otvara content DB ReadWrite (bootstrap sheme pri reloadu).
- Nakon restarta svi klipovi su `previously_seen`; Novi je prazan do sljedeceg
  Selecta. To je ugovor, lako se cita kao bug.
- Stariji `camera-patterns-v1` ostaje u testovima.

## Granice koje drze

- Nema Ingest -> Project cratea niti obrnuto.
- Work settings: `query_only`, samo SELECT.
- Content authorizer blokira upis u Project tablice, ukljucujuci trigger.
- Nema `allowed_applications`.
- Nema app-to-app bridgea.
- Jedan probe prolaz; postojeći Final se ne proba ponovo.
- Forma salje `action_id`; ne zove scanner/probe/store write izravno.
- Project freeze scope prazan.
- Javni identitet lokacije ostaje QNC URI.

## Conformance

`cargo run -p qnc-conformance` na `875d981`: **all checks passed**.
Keyboard catalog: 87 akcija (ukljucen `ingest_set_clip_filter`).

## Provjereno

- HEAD, cist status, workspace/contract brojevi.
- AGENTS 4.1, 5, 5.1, 14, 16 naspram koda.
- Freeze scope, shell factory, Story/MA odsutnost crateova.
- Ingest content adapter, authorizer, catalog hydrate, import stub.
- Story/MA read ugovor vs fizicka `qnc_project.db`.
- Module contracti vs crateovi.
- ffprobe spawn granica (preko koda, ne live).
- docs/36-42 usklađenost.
- Conformance.

## Nije provjereno

- Novi live Select na kartici u ovom prolazu (docs/39-42 tvrde Windows live
  98 klipova; nije ponovljen ovdje).
- Fizicki LAN/Intranet host.
- Linux/macOS GUI i upis pod POSIX lockom.
- Pun `cargo test --workspace`.
- Pixel-doslovnost cijelog UI-ja naspram `qnc_v4` izvan docs/41-42.
- Prisutnost `qnc-media-probe` exe u korisnickom deploymentu.

## Sljedeci rizik

Pokrenuti Filmstrip/Player prije importa moze proci ako moduli citaju
original/proxy URI iz `ingest_content` i `media_records`. To je dopusteno
samo ako ne rade novi probe i ako playback politika dolazi iz projektnih
postavki. Veci proizvodni jaz je import: bez njega medij ostaje na kartici.

Predlozeni redoslijed, bez implementacije:

1. Uskladiti §16 i docs/01/00/33 s HEAD-om; ne dirati pravila 1-15.
2. Izvrsni import prema vec zapisanim storage politikama.
3. Filmstrip pa Wave, citanje samo iz baze.
4. Broadcast Player.
5. Primjer `ingest-transport.json` bez credentiala.
6. Tek onda Media Assist / Story crateovi.
)
