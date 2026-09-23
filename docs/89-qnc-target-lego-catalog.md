# QNC ciljna slika: katalog lego kockica i forme kao ploče

Datum: 2026-09-23 · QNC grana `dev/2026-09-22`, checkpoint commit `9fd8b07`
· referenca procedure: `C:\Users\miron\Projects\QNC_v5`, git `main` commit `dfd53ac` (samo čitanje).

Ovaj dokument je obvezna ciljna slika (Paket A, zapisano u §18 `AGENTS.md`). Pravila iz
odjeljka 6 provodi `qnc-conformance`.

Ovaj dokument je **cilj**, ne audit. Kaže kako QNC mora izgledati kad je gotov, koje kockice postoje,
što smije biti u formi i u application sloju, kojim pravilima se to zaključava i kojim redom se radi.
Nalazi iz dosadašnjih audita ovdje su samo „put od sadašnjeg stanja do cilja“.

---

## 0. Obvezne upute za agenta

1. Prvo pročitati `AGENTS.md`. Ovaj dokument ga ne mijenja i ne slabi. Ako nešto ovdje proturječi
   `AGENTS.md`, vrijedi `AGENTS.md` i to se javlja korisniku.
2. Obitelj je zamrznuta (§14, §17, §18). Rad ide **po paketima** iz odjeljka 7. Za svaki paket korisnik
   daje jedno otključavanje koje pokriva cijeli paket. Izvan opsega paketa se ne dira ništa; ako paket
   traži više, stani i pitaj.
3. **Izgled se ne mijenja** (§0.6, §10). Premještanje crtanja iz forme u kockicu mora dati pikselski isti
   ekran. Snimka prije i poslije za Ingest, MA/Story i Project je obvezna.
4. **Istina je u bazi aktivnog projekta.** Nijedna komponenta ne drži projektno stanje kao istinu, nema
   vlastite postavke, defaulte ni rezervne putanje.
5. Testovi nikad ne pišu u stvarni projekt (§0.4).
6. `qnc-ingest-application` i `qnc-editorial-application` se samo smanjuju.
7. QNC_v5 je **samo referenca procedure i ponašanja**. Iz njega se ne kopira arhitektura (središnji host,
   HTTP veza, forme s workflowom, monolitni crateovi). U QNC_v5 se ništa ne mijenja.
8. Svaki paket završava: ciljani `cargo test`, `cargo run -p qnc-conformance`, `cargo build -p qnc-app`,
   live provjera, zapis zatvorenog odobrenja u §18 `AGENTS.md`, commit samo na korisnikov zahtjev.

---

## 1. Pojmovi

| Pojam | Što je | Smije | Ne smije |
|---|---|---|---|
| **Forma** (`*-desktop`) | Ploča. Raspored slotova iz layout ugovora u koje se stavljaju UI kockice. | Računati pravokutnike iz ugovora; pozvati UI kockicu s gotovim view modelom; vratiti intent s `action_id`. | Crtati primitive (`ui.painter()`), graditi ulaz kockice iz domenskih podataka, imati tekstove i boje izvan ugovora, znati za bazu, projekt, player stanje ili workflow. |
| **Application** (`*-application`) | Composition root jedne aplikacije. Spaja kockice, prima intent, daje **gotov view model**. | Držati instance kockica; prosljeđivati intent; mapirati domenske podatke u view model kockica; držati lokalno UI stanje (odabrani tab, fokus, lokalna oznaka odabira do Uvezi). | Imati vlastiti SQL, čitati postavke mimo porta, raditi posao koji ima ili treba imati kockicu, rasti. |
| **UI kockica** | Pasivni javni prikaz. | Ulaz: gotov view + stil iz ugovora. Izlaz: akcija/intent. | Baza, datoteke, niti, sat, znanje o aplikaciji. |
| **View-model kockica** | Čisto preslikavanje javnih podataka u ulaz UI kockice. | Bez I/O. | egui ovisnost (osim tipova UI kockice koju hrani). |
| **Servis/worker kockica** | Pozadinski posao (filmstrip, wave, uvoz, posteri, player klijent). | Čitati kroz javne readere, vraćati rezultat pisaču. | Izravni upis u bazu, znanje o formi, konkurirati playeru. |
| **Podatkovna kockica** | Čitač ili vlasnik-pisač baze. | Jedini SQL nad svojom bazom. | Znanje o aplikaciji koja ga koristi. |

**Univerzalna kockica**: nema ime aplikacije, nema grane po aplikaciji, ulaz i izlaz su neutralni, i
svaka aplikacija koja treba taj posao koristi **tu** kockicu. Kockica s imenom aplikacije
(`qnc-ingest-*`) smije postojati samo za posao koji stvarno pripada samo toj aplikaciji.

---

## 2. Ciljni slojevi

```
 baza aktivnog projekta (qnc-projects.db -> project.db)
        │ čitanje: qnc-active-project-read, qnc-work-settings, qnc-content-read
        │ pisanje: samo qnc-content-store (i owneri media-record/source-index za Select)
        ▼
 servis/worker kockice  (player klijent, preview, posteri, filmstrip, wave, uvoz, Select)
        │ rezultati → view-model kockice
        ▼
 application (composition root)  ── gotov view model ──▶  forma (ploča)
        ▲                                                   │ slotovi iz layout ugovora
        └──────────── intent s action_id ◀──────────────────┘ UI kockice crtaju
```

Veza između aplikacija ostaje **samo baza**. Shell samo hosta površine i čita slijed aplikacija iz baze.

---

## 3. Katalog kockica

Oznake statusa: ✅ gotovo i u skladu · 🔧 postoji, treba popravak · 🆕 treba stvoriti · 🗑 ukloniti ili spojiti.

### 3.1 Projekt i postavke

| Kockica | Ulaz → izlaz | Korisnici sada | Cilj | Status |
|---|---|---|---|---|
| `qnc-active-project-read` | QNC root → snapshot aktivnog projekta, potpis kataloga, usporedba | Ingest, MA/Story, workeri, dijagnostika | isto | ✅ |
| `qnc-work-settings` | registar + `project.db` → `WorkSettings` (uklj. `products`) | svi | ukloniti tihi rezervni put (`lib.rs:140`); `original`/`proxy` u `products` ili storage blok | 🔧 |
| `qnc-application-sequence` | postavke → slijed aplikacija | shell, Ingest | isto | ✅ |
| `qnc-project-close` | intent → isprazni `active_project_id` | shell | isto | ✅ |
| `qnc-project-store` | Project owner | Project | direktoriji projekta iz `products`/template, ne iz `PROJECT_DIRECTORIES` konstante | 🔧 (§14) |

### 3.2 Baza sadržaja projekta

| Kockica | Ulaz → izlaz | Korisnici sada | Cilj | Status |
|---|---|---|---|---|
| `qnc-content-store` | jedini pisač `project.db` sadržaja | svi pisači | sheme i SQL po domenama u modulima unutar ownera; owner ostaje jedan | 🔧 |
| `qnc-content-read` | javni viewovi → sažeci, klip, artefakti | MA/Story, preview, active-project-read | **jedini** čitač kataloga za sve aplikacije | 🔧 |
| `qnc-ingest-catalog` | drugi čitač kataloga (Ingest) | Ingest | spojiti u `qnc-content-read`; ostaje samo Ingest-specifično (lokalna oznaka odabira, Novi/Sve) ako treba | 🗑/🔧 |
| `qnc-ingest-store` | `content` alias + host `ingest_registry.db` | Ingest | alias ukloniti; registar izvora ukloniti ili prebaciti u projekt (odluka korisnika) | 🗑 |

### 3.3 Izvori i Select (Ingest domena)

| Kockica | Uloga | Status |
|---|---|---|
| `qnc-source-bindings` | jedini parser host konfiguracije izvora | ✅ |
| `qnc-source-input` | neutralni ulazi (kartica, direktorij, LAN, intranet) → browser sesija | 🔧 novo, dovršiti i dati mu korisnike |
| `qnc-dir-browser` | stanje preglednika, `dir.list`/`dir.select` | ✅ (stanje) |
| `qnc-source-browse` | neblokirajuće listanje | ✅ |
| `qnc-source-reader`, `qnc-scanner`, `qnc-source-groups`, `qnc-camera-*`, `qnc-sony-metadata`, `qnc-media-probe`, `qnc-ffprobe-metadata`, `qnc-media-metadata*`, `qnc-media-record*`, `qnc-source-index-*` | Select lanac, jedini probe | ✅ |
| `qnc-ingest-select`, `-selection-write`, `-clip-list`, `-cameras`, `-work-plan`, `-import-worker` | Ingest workflow | ✅ (opravdano Ingest-specifično) |

### 3.4 Player i preview

| Kockica | Ulaz → izlaz | Cilj | Status |
|---|---|---|---|
| `qnc-broadcast-player`, `qnc-broadcast-engine`, `qnc-player-contract`, `-client`, `-launcher`, `-frame-transport`, `-timeline`, `qnc-media-decode`, `-stream`, `qnc-ffmpeg-decode`, `qnc-decoder-catalog`, `qnc-pixel-convert`, `qnc-gpu-raster`, `qnc-audio-output`, `qnc-video-output` | player lanac | ne dirati u ovom planu (§8.3) | ✅ |
| `qnc-player-input` | spremljeni medij → `PreparedInput` | isto | ✅ |
| `qnc-source-preview` | klip → pripremljen player, potvrđena slika, timeline projekcija | **čita snimku medija iz projektne baze** (`clips.catalog_json` kroz content read/store), ne iz host `ingest_media_records.db` (`content.rs:18-45`) | 🔧 kritično |
| `qnc-ingest-preview-source` | Ingest adapter za preview | ukloniti kad `qnc-source-preview` čita projektnu bazu; Ingest koristi isti preview kao MA/Story | 🗑 |
| **`qnc-playback-activity`** | „player priprema/svira“ → zapis u bazi; čitanje za workere | nastaje iz `qnc-ingest-runtime` (neutralno ime i API); koriste ga **svi** previewi i **svi** generatori | 🆕 (iz 🔧 `qnc-ingest-runtime`) |
| `qnc-playback-priority` | stanje playera + teške akcije → odluka | koriste Ingest i MA/Story | 🔧 (samo Ingest) |

### 3.5 Artefakti

| Kockica | Uloga | Cilj | Status |
|---|---|---|---|
| `qnc-filmstrip`, `qnc-filmstrip-worker`, `qnc-wave`, `qnc-wave-worker`, `qnc-timeline-artifacts`, `qnc-timeline-assets`, `qnc-content-artifacts` | generiranje i čitanje filmstripa/wavea | svi generatori poštuju `qnc-playback-activity`, i u procesu i u `tools/qnc-ingest-worker` (`run_artifacts` sada ne pauzira) | 🔧 |
| `qnc-clip-posters` | posteri u pozadini, odabrani prvi | **jedini** put postera; koristi ga i Ingest (`thumbnails.rs` se uklanja) | 🔧 |
| `qnc-media-thumbnail`, `qnc-image-assets`, `qnc-poster-create` | niže razine | ✅ |

### 3.6 Virtualni kadrovi

| Kockica | Status |
|---|---|
| `qnc-virtual-shots`, `qnc-virtual-short-stills`, `qnc-virtual-short-cards` | ✅; `-short-cards` daje neutralne podatke kartice (vidi 3.7) |
| Budući Story segmenti, cover/B-roll, program playlista, export | 🆕 kasnije, kao nove kockice po v5 proceduri (paket H) |

### 3.7 UI kockice

| Kockica | Ulaz → izlaz | Korisnici sada | Cilj | Status |
|---|---|---|---|---|
| `qnc-monitor` | potvrđena slika/poruka/poster → prikaz | obje forme | isto | ✅ |
| `qnc-source-dock` + `qnc-timeline` | projekcija + assets → dock, timeline intent | obje forme | isto | ✅ |
| `qnc-media-card` | redovi kartica → mreža, akcija | obje forme | **dodati neutralni vlasnički tip `CardData`** (bez egui tipova, boja kao token iz ugovora) i `show_card_grid` nad njim; application daje `Vec<CardData>`, forma samo predaje | 🔧 |
| `qnc-media-pool-head` | tabovi + transport traka → akcija | nitko | forme ga koriste; tabovi i gumbi iz view modela | 🔧 (neiskorišten) |
| `qnc-editorial-shell` | geometrija ploče (lijevi stupac, razdjelnik, desni panel, monitor slot) | nitko | obje forme ga koriste za ploču | 🔧 (neiskorišten) |
| `qnc-ui-kit` | action bar, switch, option columns, raster | forme | + gumbi, tabovi, linkovi, `format_duration`, `truncate` (sada kopirani u formama) | 🔧 |
| **`qnc-ui-theme`** | shell layout ugovor → tema (boje, font, mjere) | — | zamjenjuje tri kopije `theme.rs` | 🆕 |
| **`qnc-dir-browser-view`** | stanje `qnc-dir-browser` → prikaz (izvori, Gore, Diskovi, breadcrumb, redovi) → intent | — | zamjenjuje Ingest `location_browser.rs` + `browser_entries.rs` i Project `location_browser.rs` | 🆕 |
| **`qnc-surface-host`** | application → petlja površine (poll, osvježavanje po playeru, tipkovnica kroz `qnc-keyboard-shortcut`) | — | zamjenjuje gotovo identične `app.rs` petlje u formama | 🆕 |
| **`qnc-layout-contract`** | učitavanje i provjera layout ugovora | — | zamjenjuje tri kopije `layout_contract.rs` (oko 237 linija svaka) | 🆕 |
| `qnc-keyboard-shortcut` | događaj → `action_id` | Ingest, MA/Story | + Project (sada ima vlastitu tablicu tipki) | 🔧 |

### 3.8 Shell i adapteri

`apps/qnc-app`, `qnc-shell-desktop-api`, `*-desktop-adapter` (37–56 linija): ✅. Ne dirati u ovom planu.

---

## 4. Forme kao ploče

### 4.1 Ingest (`qnc-ingest-desktop`)

| Slot iz `ingest.layout.json` | Kockica | Ulaz iz view modela |
|---|---|---|
| ploča (lijevi stupac, razdjelnik, desno) | `qnc-editorial-shell` (ili neutralni naziv iste kockice) | geometrija iz ugovora |
| monitor | `qnc-monitor` | `view.monitor` |
| glava poola | `qnc-media-pool-head` | `view.pool_head` (tabovi, gumbi, `action_id`, enabled) |
| preglednik izvora | `qnc-dir-browser-view` | `view.browser` |
| mreža klipova | `qnc-media-card` | `view.cards: Vec<CardData>` |
| source dock + timeline | `qnc-source-dock` | `view.dock` (naziv, IN/OUT, akcije), `SourceTimeline::from_assets` |
| tema, gumbi | `qnc-ui-theme`, `qnc-ui-kit` | ugovor |
| petlja površine | `qnc-surface-host` | application |

Iz forme odlazi: `clip_grid.rs` mapiranje (boja `from_rgb(55,210,145)`, `imported`→marker,
`SaveState`→tekst), `pool_head.rs`, `board.rs` crtanje, `location_browser.rs`, `browser_entries.rs`,
`buttons.rs`, `text.rs`, `theme.rs`, hardkodirane kontrole u `source_dock.rs` („Export“, „B“,
„Kopiraj original“, „AI mining“, „Nema postera na kartici“), rezervni tekstovi uz ugovor.
**Ostaje**: `lib.rs`, raspodjela slotova, predaja view modela kockicama, vraćanje intenta.

### 4.2 Media Assist e, g, l i Story o (`qnc-editorial-desktop`)

Isti slotovi kao Ingest, bez preglednika izvora; na njegovom mjestu popis klipova (`qnc-media-card`).
Kompozicija po grupi dolazi iz `editorial.layout.json`.
Iz forme odlazi: `render_clip_grid` mapiranje (`widgets.rs:346-385`, `pipeline_statuses`, odabrani id,
poruke praznog stanja), `render_pool_head`, `render_board`, `render_left_column`, `preview_height`,
`text_tab`, `small_button`, `action_button`, `format_duration`, `theme.rs`, petlja u `app.rs`.

### 4.3 Project (`qnc-project-desktop`), zamrznuto §14

Cilj: ista ploča. Tok (`perform_create_project`, `perform_save_custom_template`,
`confirm_projects_root_browser`, nacrt postavki, `dirty` zastavice, poruke „DB: …“) seli u
`qnc-project-application` kao view model + intent. Tipkovnica kroz `qnc-keyboard-shortcut`
(ukloniti `shortcut_events`, `catalog_key_name`, `catalog_key_code`). Preglednik kroz
`qnc-dir-browser-view`. `project_advanced.rs` najprije pregledati.

**Mjerilo gotovosti forme:** nema `ui.painter()`, nema `Color32::from_rgb`, nema korisničkih tekstova
izvan ugovora, nema tipova domenskih podataka (npr. `SaveState`, `ImportStatus`) osim gotovog view
modela, nema vlastitih petlji.

---

## 5. Application slojevi

`qnc-ingest-application` i `qnc-editorial-application` na kraju sadrže samo:
- instance kockica i njihovo povezivanje;
- dispatch intenta prema kockicama;
- mapiranje rezultata kockica u view model (uključujući `CardData`, stanje gumba i tabova);
- lokalno UI stanje (odabrani tab, fokus, lokalna oznaka odabira do Uvezi).

Iz njih odlazi: vlastiti put postera (Ingest `thumbnails.rs`), `IngestStore` i host registar,
`selection_config` kao izvor veza (kroz `qnc-source-bindings`/`qnc-source-input`), pravila zauzetosti
i prioriteta između komponenti (u `qnc-playback-priority` + `qnc-playback-activity`), petlje čekanja
na write transport u MA/Story (`wait_for_saved_short`, `wait_for_changed` u javni helper uz
`qnc-virtual-shots`/`qnc-content-store`).

Veličina se bilježi kao gornja granica u conformanceu i smije samo padati.

---

## 6. Conformance pravila (zaključavaju cilj)

Svako pravilo dobiva **popis poznatih iznimki** koji se smije samo smanjivati. Tako se pravilo uvodi
odmah, prije popravaka, i ništa novo ne može ući.

| # | Pravilo | Mehanička provjera |
|---|---|---|
| C1 | Forma ne crta primitive | u `crates/*-desktop/src` nema `ui.painter()`, `painter().` |
| C2 | Forma nema boje ni korisničke tekstove izvan ugovora | nema `Color32::from_rgb`/`from_rgba`; nema string literala u pozivima labela/gumba osim ključeva ugovora |
| C3 | Forma ne gradi ulaz kockica iz domenskih podataka | nema `CardRow {`, `pipeline_statuses`, domenskih tipova application sloja osim view modela |
| C4 | Forma nema vlastitu petlju ni tablicu tipki | nema `egui::Key` mapiranja, `consume_egui_action_presses` samo u `qnc-surface-host` |
| C5 | Javna kockica ima korisnika | svaki crate s `*.module.json` ima barem jednog runtime korisnika ili oznaku `"reserved": true` u ugovoru |
| C6 | Kockica s imenom aplikacije ne koristi druga aplikacija | `qnc-ingest-*` smiju koristiti samo Ingest crateovi i Ingest alati |
| C7 | Preview i player ne otvaraju host bazu | stablo ovisnosti `qnc-source-preview` ne sadrži `qnc-media-record-db` ni `qnc-source-index-db` |
| C8 | Svaki generator poštuje player | filmstrip/wave/uvoz/kopija workeri ovise o `qnc-playback-activity` i pozivaju ga |
| C9 | Svaki preview javlja aktivnost playera | `qnc-source-preview` ovisi o `qnc-playback-activity` |
| C10 | Application sloj ne raste | broj linija bez testova ≤ zapisana granica |
| C11 | Jedan čitač kataloga, jedan put postera | samo `qnc-content-read` čita `public_clips` sažetke; samo `qnc-clip-posters` pokreće `ThumbnailBatchService` |
| C12 | Nema tihih rezervnih putanja | `qnc-work-settings` bez zadane putanje registra; nema `unwrap_or_else(|| root.join("data")…)` u čitačima |
| C13 | Raspored mapa iz postavki | nema string literala `"original"`, `"proxy"`, `"products/…"` izvan `qnc-work-settings` i seeda |

Postojeće provjere (DB-only izolacija, player granica, izvori samo za čitanje…) ostaju.

### 6.1 Provedba (Paket A, 2026-09-23)

- Kod: `tools/qnc-conformance/src/lego.rs`, provjera „target picture C1-C13 (forms are boards of
  public pieces)“ u `cargo run -p qnc-conformance`.
- Poznate iznimke: `tools/qnc-conformance/lego-baseline.json` (51 ključ, snimljeno na `9fd8b07`).
  Ključ je `pravilo|datoteka` ili `pravilo|crate`, vrijednost je broj pojava.
  - broj veći od baselinea ili novi ključ: **greška**, conformance pada;
  - broj manji od baselinea: **upozorenje** da se baseline spusti ili ključ ukloni.
  Baseline se nikad ne podiže. Svaki paket iz odjeljka 7 spušta svoje ključeve.
- Trenutni brojevi: `cargo run -p qnc-conformance -- --print-lego-counts` (samo ispis, ne piše
  baseline).
- Ispitni kod (`*_tests.rs`, `tests.rs`, `examples/`, stavke pod `#[cfg(test)]`) i komentari se ne broje.
- Točna mehanika i svjesne iznimke:
  - C2 tekst: literal koji počinje velikim slovom, ima malo slovo i nema `::`, `/`, `_`, `\`
    (heuristika; služi da ništa novo ne uđe, ne kao popis prijevoda).
  - C4 broji `egui::Key`, `Key::`, `consume_egui_action_presses`, `egui_shortcut_events`,
    `request_repaint_after`, `notify_on_player_change` u formama.
  - C5 preskače ugovore bez cratea i crateove koji su samo proces (`main.rs` bez `lib.rs`);
    ugovor može nositi `"reserved": true`.
  - C6 dopušta shellu (`qnc-app`) ovisnost o `*-desktop-adapter` (§3).
  - C8 prihvaća `qnc-ingest-runtime` kao port aktivnosti playera dok ga Paket B ne preimenuje u
    neutralni `qnc-playback-activity`; C6 i dalje bilježi svaku upotrebu izvan Ingesta.
  - C10 broji neprazne linije bez testova (forme i application crateovi).
  - C11 vlasnici: katalog `qnc-content-store`, `qnc-content-read`, `qnc-ingest-store` (alias);
    posteri `qnc-media-thumbnail`, `qnc-clip-posters`.
  - C12 preskače `qnc-project-store` (vlasnik registra) i `qnc-dev-diagnostics` (logovi nisu
    projektni podaci).
  - C13 broji `"products/` izvan `qnc-work-settings`, te `"original"`/`"proxy"` u
    `qnc-ingest-import-worker` i `qnc-project-store`.

Stanje baselinea po pravilu: C1 36 pojava u 9 datoteka, C2 boje 8 i tekst 208, C3 7, C4 101,
C5 3 crateova (`qnc-editorial-shell`, `qnc-media-pool-head`, `qnc-source-input`), C6 2, C7 1,
C8 2, C9 1, C10 6 granica, C11 1, C12 7, C13 3 ključa.

---

## 7. Redoslijed rada po paketima

Svaki paket je jedno otključavanje, jedna cjelina, jedan kriterij gotovosti.

### Paket A: Temelj
- **Cilj:** zaključati smjer prije popravaka.
- **Posao:** conformance pravila C1–C13 s popisom poznatih iznimki; zapis granica veličine (C10);
  dokument ove ciljne slike u `docs/` (uz dozvolu).
- **Otključati:** `tools/qnc-conformance/**`, `docs/` (novi dokument), `AGENTS.md` (zapis).
- **Gotovo kad:** conformance prolazi s iznimkama; svaka iznimka ima nalaz iz ovog dokumenta.

### Paket B: Podaci, player i prioritet
- **Cilj:** jedan put do playera iz projektne baze i prioritet playera za sve.
- **Posao:** `qnc-source-preview` čita snimku medija iz projektne baze; ukloniti
  `qnc-ingest-preview-source`; `qnc-ingest-runtime` → `qnc-playback-activity`; svi previewi javljaju,
  svi generatori (i `tools/qnc-ingest-worker`) poštuju; jedan čitač kataloga; jedan put postera;
  ukloniti `qnc-ingest-store` alias; odluka o `ingest_registry.db`.
- **Otključati:** `qnc-source-preview`, `qnc-ingest-preview-source`, `qnc-ingest-runtime` (+ novi
  `qnc-playback-activity`), `qnc-playback-priority`, `qnc-content-read`, `qnc-content-store`,
  `qnc-ingest-catalog`, `qnc-ingest-store`, `qnc-clip-posters`, `qnc-content-artifacts`,
  `qnc-filmstrip-worker`, `qnc-wave-worker`, `qnc-ingest-import-worker`, `tools/qnc-ingest-worker`,
  oba application cratea, ugovori modula, Cargo.
- **Gotovo kad:** C6, C7, C8, C9, C11 bez iznimki; MA/Story svira bez `data/ingest_media_records.db`;
  wave/filmstrip staju dok svira bilo koji preview (mjereno u logu).

### Paket C: UI kockice
- **Cilj:** sve što forme crtaju postoji kao javna kockica.
- **Posao:** `qnc-ui-theme`, `qnc-dir-browser-view`, `qnc-surface-host`, `qnc-layout-contract`;
  gumbi i tekst pomoćnici u `qnc-ui-kit`; `CardData` u `qnc-media-card`; `qnc-editorial-shell` i
  `qnc-media-pool-head` prošireni samo dodavanjem tako da crtaju pikselski isto kao kopije u formama.
- **Otključati:** novi crateovi i ugovori, `qnc-ui-kit`, `qnc-media-card`, `qnc-media-pool-head`,
  `qnc-editorial-shell`, `qnc-dir-browser` (samo ako treba javni view tip), Cargo.
- **Gotovo kad:** kockice imaju testove geometrije; još se ne koriste (C5 iznimke privremeno).

### Paket D: Forme Ingest i MA/Story kao ploče
- **Cilj:** forma = raspored slotova.
- **Posao:** forme prelaze na kockice iz paketa C; application daje gotov view model (4.1, 4.2).
- **Otključati:** `qnc-ingest-desktop`, `qnc-editorial-desktop`, `qnc-ingest-application`,
  `qnc-editorial-application`, aditivno `contracts/ui/ingest.layout.json` i `editorial.layout.json`.
- **Gotovo kad:** C1–C5 bez iznimki za ove dvije forme; snimke pikselski iste; live Ingest i MA/Story.

### Paket E: Application slojevi
- **Cilj:** samo kompozicija.
- **Posao:** odjeljak 5.
- **Otključati:** oba application cratea, `qnc-virtual-shots`, `qnc-content-store` (helper),
  `qnc-source-input`, `qnc-ingest-select` (samo `selection_config` preko bindinga).
- **Gotovo kad:** C10 granice spuštene na novu vrijednost; nema vlastitih petlji čekanja ni puteva
  koji imaju kockicu.

### Paket F: Raspored mapa i rezervni putevi
- **Cilj:** jedan izvor za mape, bez tihih defaulta.
- **Posao:** `original`/`proxy` u postavke; `PROJECT_DIRECTORIES` iz templatea; ukloniti rezervnu
  putanju u `qnc-work-settings`.
- **Otključati:** `qnc-work-settings`, `qnc-ingest-import-worker`, `qnc-project-store` i `seed/system_seed.json`
  (Project, §14), ugovori.
- **Gotovo kad:** C12, C13 bez iznimki.

### Paket G: Project forma (§14)
- **Cilj:** Project forma kao ploča.
- **Posao:** 4.3.
- **Otključati:** `qnc-project-desktop`, `qnc-project-application`, Project dio conformancea.
- **Gotovo kad:** C1–C4 bez iznimki i za Project; Project ponašanje i izgled isti.

### Paket H: Nove funkcije iz v5 (tek nakon A–G)
- Story segmenti i segment timeline, program/playlista, sync cover, cover/B-roll, hi-res export, ASR.
- Svaka funkcija: prvo ugovor kockice i DB ugovor, pa kockica, pa slot u formi. v5 daje samo
  proceduru i ponašanje (`qnc-app/src/story.rs`, `qnc-host/src/story/**`, `editorial_playlist.rs`,
  `export_hires.rs`), nikad strukturu.

---

## 8. Odluke koje donosi korisnik

1. **Aktivni projekt** (`C:\Users\miron\Test projekt\ghjkjhghjk…_392fe8…`) nema `products` blok u
   `settings_json`, pa ga trenutni kod ne može učitati. Novi projekt s trenutnim templateom, ili agent
   samo prijavljuje grešku. Migracije nisu dopuštene.
2. **`data/ingest_registry.db`**: ukloniti ili premjestiti zapis odabranih izvora u projekt.
3. **Paket C/D**: ako javne kockice ne mogu crtati pikselski isto, prihvaća li se razlika (UI odobrenje)
   ili se kockica proširuje dok ne bude isto.
4. **Imena novih kockica** (`qnc-playback-activity`, `qnc-ui-theme`, `qnc-dir-browser-view`,
   `qnc-surface-host`, `qnc-layout-contract`) i treba li `qnc-editorial-shell` preimenovati u neutralno
   ime jer ga koristi i Ingest.
5. **`qnc-content-store`**: podjela SQL-a po domenama unutar jednog ownera, i ime sheme
   `ingest_content_schema` (promjena znači novu shemu).

---

## 9. Što nije provjereno

- `cargo test` cijelog workspacea i live UI nakon checkpointa `9fd8b07`.
- Crtaju li `qnc-editorial-shell`, `qnc-media-pool-head` i `qnc-media-card` pikselski isto kao kopije u
  formama.
- Je li `clips.catalog_json` dovoljan za sve formate koje player podržava (Ingest ga već koristi).
- `qnc-source-input` je nastao u tijeku rada i nije pregledan u detalje.
- `qnc-project-desktop/src/project_advanced.rs` nije pregledan liniju po liniju.
