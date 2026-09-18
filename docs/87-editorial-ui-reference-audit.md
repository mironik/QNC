# Editorial UI reference audit (Story i Media Assist grupe)

Status: qnc_v4 layout snapshot + mapa komponenti (samo dokument, bez koda)  
Datum: 2026-09-18  
Root: `C:\Users\miron\Projects\QNC`  
Referenca: `C:\Users\miron\Projects\qnc_v4`

## Cilj

Prije gradnje aplikacija po grupama utvrditi kompletnu UI shemu: koji su pasivni
dijelovi layouta i koje javne komponente treba Story, a koristi ih i Media
Assist, po grupama.

Planirane aplikacije (redoslijed grupa = redoslijed posla novinara):

| Grupa | Aplikacija | Namjena |
|---|---|---|
| e | Media Assist Audio AI | pregled izjava, transkripcija po rijecima, prijevod |
| g | Media Assist Audio | pisanje i snimanje offa |
| l | Media Assist Video | virtualni kadrovi, analiza slike i tehnike |
| o | Story | slaganje price |

Grupe a (Project) i b (Ingest) vec postoje.

## Odluka korisnika

Desni panel je prostor za promjenjive funkcije pojedine grupe. **Zasad ostaje
prazan**, kao u v4 Media Assistu (`RightPanelKind::None`). Sadrzaj desnog panela
za e, g i l nije dio ovog koraka i ne izmislja se bez odobrenja (AGENTS.md §10).

## Izvori

Pregledani qnc_v4 izvori:

```text
qnc-app/src/composition.rs
qnc-app/src/story.rs        (struktura, ui_main, ui_source_dock; ne cijela)
qnc-app/src/media_assist.rs
qnc-app/src/qnc_ui.rs
qnc-app/src/qnc_media_card.rs
qnc-app/src/editorial/      (samo nazivi i velicine datoteka)
seed/tabs/story/plugin.json
seed/tabs/media_assist/plugin.json
```

Pregledani QNC izvori:

```text
contracts/ui/ingest.layout.json
docs/11-ingest-ui-reference-audit.md
crates/qnc-ui-kit (javni API)
```

## Nalaz 1: Story i Media Assist su u v4 jedan ekran

`StoryScreen` (`story.rs`, 7106 redaka) se konfigurira ulogom
`EditorialRole::{Story, MediaAssist}`. `media_assist.rs` ima 6 redaka. Uloga
mijenja samo zastavice kompozicije (`composition.rs`), ne kod crtanja.

| Dio | Story | Media Assist | Ingest |
|---|---|---|---|
| Shell | `editorial_shell` | `editorial_shell` | `editorial_shell` |
| Glava pool-a: Segment tab | da | ne | ne |
| Glava pool-a: Cover tab | da | da | ne |
| Glava pool-a: Export HiRes | da | ne | ne |
| Glava pool-a: Quick Cover | da | ne | ne |
| Desni panel | `SegmentPanel` | **`None`** | `ClipGrid` |
| Donji dock (source timeline) | da, story akcije | da, story akcije | da, ingest akcije |
| Kartica: kvacica odabira | ne | da | da |
| Kartica: tockice statusa | `Pipeline` | `Pipeline` | `ImportedOnly` |

Novi QNC ne smije preuzeti model "jedna forma s ulogama" (AGENTS.md §2, §3:
zasebne aplikacije, bez zajednickog ekrana). Zajednicka je samo kompozicijska
tablica u ugovoru i javne komponente.

## Nalaz 2: mjere shella vec su u ugovoru

`qnc_ui::space` (v4) vec je preslikan u `contracts/ui/ingest.layout.json`
(`board`, `preview`, `pool_head`): `left_ratio 0.365`, `divider_width 5`,
`left_min_width 280`, `right_min_width 200`, `preview.reserve_below 190`,
`preview.min_height 160`. Ingest koristi Story omjere, pa ih ne treba
ponovno izmisljati; novi ugovor koristi iste vrijednosti (ingest.layout.json se
ne mijenja).

## Nalaz 3: pasivni dijelovi i njihov status u novom QNC-u

| Pasivni dio | v4 izvor | Novi QNC | Potrebno |
|---|---|---|---|
| Monitor (preview) | `qnc_ui::preview`, player monitor | `qnc-monitor` | postoji |
| Timeline dock | `qnc_source_dock`, `qnc_timeline` | `qnc-timeline` | postoji |
| Filmstrip pozadina | `qnc_filmstrip_background` | `qnc-filmstrip` | postoji |
| Wave prikaz | `program_waveform`, wave | `qnc-wave-view` | postoji |
| Standardna akcijska traka | `qnc_form` | `qnc-ui-kit` | postoji |
| Editorial shell (lijevi stupac, razdjelnik, desni panel) | `qnc_ui::editorial_shell` | samo unutar Ingest forme | **novi javni modul iz v4 reference** |
| Glava pool-a (tabovi, transport) | `editorial/media_pool.rs` (515) | samo unutar Ingest forme | **novi javni modul iz v4 reference** |
| Kartica i mreza klipova | `qnc_media_card.rs` (367) | samo unutar Ingest forme | **novi javni modul iz v4 reference** |
| Panel segmenata / wrap / markera | `editorial/segment_panel.rs` (661) | nema | novi modul (samo Story) |
| Panel markera i covera | `editorial/marker_cover_panel.rs` (271) | nema | novi modul (samo Story) |
| Segment program | `editorial/segment_program.rs` (1431) | nema | novi modul (samo Story) |
| Program waveform | `editorial/program_waveform.rs` (443) | nema | novi modul (samo Story) |

**Ingest se ne dira** (odluka korisnika 2026-09-18). Ove komponente se ne
izdvajaju iz Ingest forme, nego se grade kao novi javni pasivni moduli izravno
iz v4 reference. Ingest zadrzava vlastite kopije; moguce dupliciranje je
svjesno prihvaceno dok Ingest ostaje zamrznut.

## Nalaz 4: predlozena kompozicija po grupama

Grupe e, g i l polaze od v4 Media Assist kompozicije. Story (o) polazi od v4
Story kompozicije.

| Dio | e | g | l | o |
|---|---|---|---|---|
| Shell | editorial | editorial | editorial | editorial |
| Glava: Segment tab | ne | ne | ne | da |
| Glava: Cover tab | da | da | da | da |
| Glava: Export HiRes | ne | ne | ne | da |
| Glava: Quick Cover | ne | ne | ne | da |
| Desni panel | **prazan** | **prazan** | **prazan** | `SegmentPanel` |
| Donji dock | da | da | da | da |
| Kartica: kvacica odabira | da | da | da | ne |
| Kartica: tockice statusa | `Pipeline` | `Pipeline` | `Pipeline` | `Pipeline` |

Sadrzaj desnog panela za e, g i l (transkript, tekst i snimanje, virtualni
kadrovi i analiza) nije u v4, pa je nova specifikacija i cekaju je.

## Nije provjereno

- Sadrzaj `segment_panel`, `marker_cover_panel`, `program_waveform` i
  `segment_program`: procitani su samo nazivi i velicine.
- Akcije donjeg docka (`SourceEditorAction`: `CueFrame`, `SaveVirtualShot`,
  `CreatePart`, `CreateCover`, `ToggleAudioExpand`) poznate su iz `story.rs`,
  ali ne po grupama.
- Tipkovni prečaci po grupama (mora ici kroz
  `contracts/qnc-keyboard-shortcuts.json`, AGENTS.md §10).
- Live usporedba s v4 ekranom.

## Sljedeci rizik

Story i Media Assist u v4 dijele ekran od 7106 redaka. Prijenos smije uzeti samo
layout i pasivne prikaze; poslovna logika (`save_virtual_shot`,
`create_part`, wrap sesije, playlista) mora u zasebne javne module s ugovorom
prije koda.

## Slijed koraka

1. Ovaj dokument.
2. Ugovor `contracts/ui/editorial.layout.json` s kompozicijom po grupama i
   provjerom u conformanceu.
3. Novi javni pasivni moduli za editorial shell, glavu pool-a i karticu,
   izgradjeni iz v4 reference. Ingest se ne dira.
4. Novi pasivni moduli za dijelove koje ima samo Story.
5. Kosturi aplikacija e, g, l, o: samo pasivni raspored, prazan desni panel,
   `qnc-app.json` s `priority_group`, samostalni exe.

Status: zamrznuto.

## Dopuna 2026-09-18: donji dock

Probni prozor (`crates/qnc-editorial-shell/examples/editorial_demo.rs`) je bez
docka bio nepotpun. Dodan je `qnc-source-dock` (header: naziv klipa, IN / OUT /
Trajanje, gumbi zadani popisom `actions_rtl`; mjesto za timeline) i probni
prozor sada crta stvarni `qnc-timeline` s lazniim podacima.

Odluka korisnika: raspored docka uzima se iz Ingest layouta (isti header,
razmak `header_timeline_gap` 4, inset 8, `header_item_gap` 8; ista ploha
`surface` i gornja crta). Ingest se ne dira; `qnc-conformance` cuva da mjere u
`editorial.layout.json` ostanu jednake `ingest.layout.json`.

Gumbi za e, g, l i o su zasad v4 popis: Pokrivalice, Voice over, Talking Head,
Add virtual clip. Popis po grupi je ugovorna stvar i mijenja se u
`editorial.layout.json`.

Jos nije u shemi: shell footer s karticama aplikacija (Project, Ingest, ...),
Close project i izbor teme. To pripada shellu (`qnc-app`), ne ovim modulima.
