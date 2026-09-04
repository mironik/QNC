# Ingest recoding proposal (QNC)

Status: prijedlog, nije implementacija  
Datum: 2026-09-04

UI mora biti 100% identican v4 Ingest formi. Poslovni kod se ne kopira kao
monolit. Razdvajanje je po `AGENTS.md`.

## Cilj

Samostalna Ingest aplikacija koja:

- radi bez QNC.app
- radi i kao shell tab (isti factory obrazac kao Project)
- nakon zavrsetka ingest posla prestaje raditi
- pise samo Ingest DB / javni Ingest contract
- cita Project registry (aktivan projekt, workspace URI)
- koristi module; ne posjeduje tudje workflowe

## Sto se ne smije prekopirati

- `qnc-host` kao vlasnik Ingest + Project + Story + jobs
- HTTP `/api/ingest/*` kao javna veza izmedu QNC aplikacija
- `QncApp` koji drzi IngestScreen + Story + Media Assist u jednom owneru
- privatni poziv Ingest store funkcija iz druge aplikacije
- raw OS path kao javni clip/source identitet
- OS `rfd` dialog umjesto ugradenog browsera na ovoj formi
- novi probe iz playera, filma, wavea ili UI-ja

UI/layout kod iz v4 **smije** se koristiti kao doslovni izvor preslike
(geometrija, redoslijed, labeli, font, fokus). Aktivni workflow mora ici u
Ingest owner + javne module.

## Ciljna struktura

```text
apps/qnc-ingest                 standalone exe
apps/qnc-ingest/qnc-app.json    shell registry (embedded + standalone)
crates/qnc-ingest-desktop       pasivna forma (view + action_id)
crates/qnc-ingest-desktop-adapter
crates/qnc-ingest-store         owner SQLite + javni read views

postojeći moduli (proširiti, ne forkati po app imenu):
  qnc-dir-browser          dir.list / dir.select → QNC URI
  qnc-transport-resolver
  qnc-keyboard-shortcut
  qnc-db-contract
  (novo) qnc-scanner
  (novo) qnc-camera-detector
  (novo) qnc-media-probe
  (novo) qnc-media-browser     card grid paint + select intent
  (novo) qnc-filmstrip
  (novo) qnc-wave
  (novo) qnc-editorial-shell   editorial_shell + chrome + preview + dock chrome
  (kasnije) qnc-broadcast-player
  (postoji) qnc-frame-timebase ako postoji, inace izdvojiti
```

Shell Cargo ovisi samo o adapteru, ne o `qnc-ingest-store` workflowu kao
privatnom API-ju. Store zivi u desktop crate-u Ingest procesa (standalone ili
embedded factory), isto kao Project.

## Granice

```text
UI (qnc-ingest-desktop)
  -> prikazuje view model
  -> salje action_id
  -> ne scan, ne probe, ne filmstrip, ne wave, ne decode

Ingest owner (qnc-ingest-store + workflow)
  -> cita Project public registry (koji je projekt aktivan)
  -> pise ingest_registry + ingest_content
  -> zove module: dir.list, source.scan.roles, source.camera.detect,
     media.probe.full, filmstrip.generate14, wave.generate
  -> jedini tko smije pokrenuti probe

Dir Browser
  -> listing + select URI
  -> ne scan, ne probe, ne clip katalog

Scanner + camera detector
  -> role map original/proxy/support
  -> proxy nije clip

Media Probe
  -> jednom, original + proxy metadata u istom prolazu
  -> ne zove Filmstrip/Wave/Player

Filmstrip / Wave
  -> citaju probe iz Ingest DB
  -> 14 stvarnih frameova / peaks
  -> ne ffprobe

Media Browser
  -> crta kartice iz Ingest view modela
  -> vraca select/toggle intent
```

## UI preslika (obavezna)

Koristiti `contracts/ui/ingest.layout.json` i audit `docs/11-ingest-ui-reference-audit.md`.

Isti raspored kao v4:

1. Bottom dock (fixed height) iznad shell footera.
2. Editorial split 0.365 | 5px | remainder.
3. Lijevo: 16:9 monitor, chrome All/Virtual + transport, Dir Browser.
4. Desno: clip kartice s checkom i imported dotom.
5. Isti stringovi, ukljucujuci prazan grid koji kaze "U redu" dok je gumb
   "Odaberi". To je v4, ne ispravljati.

Shared editorial chrome (preview, chrome_row, content_panel, source dock
okvir, media card) ide u **neutralni UI modul**. Ingest i kasnije Story/MA
koriste isti modul s feature flagovima kao v4 `HeadFeatures` / `DockFeatures`
/ `MediaCardFeatures`. Modul ne zna Ingest workflow.

Postojeci Project `location_browser.rs` treba izdvojiti u Dir Browser UI
dio tog modula (jedan widget). Ingest ga koristi s `id_salt = ingest` i
`confirm_label = Odaberi`. Project i dalje smije koristiti isti widget s
svojim labelima.

## Workflow rezovi (redoslijed)

AGENTS: moduli prije pune Ingest app. Project vec postoji.

### Rez 0 — ugovori (ovaj korak)

- UI audit + `ingest.layout.json` (napravljeno uz ovaj prijedlog)
- Dopuniti DB contract tablicama/kolonama koje player/filmstrip/wave trebaju
  (ne ostaviti prazne `probe_records`)
- Keyboard: Ingest akcije u `qnc-keyboard-shortcuts.json` preko action_id.
  V4 runtime koristi storyboard scope za play/step. Za 100% tipkovnicu
  zadrzati iste akorde iz kataloga; ne hardkodirati u formu.
- Zapisati UI odstupanja **prije** koda ako ih bude. Predlozena odstupanja
  dolje.

### Rez 1 — pasivna forma, prazan/fixture view

- `qnc-ingest-desktop` crta 100% layout s praznim stanjem
- fixture: prazan grid, empty monitor, LAN/Internet empty copy
- action_id izlazi iz UI-ja, store jos ne radi scan
- dual-mode: standalone + shell adapter
- live usporedba s v4 Ingest tabom (isti window size)

### Rez 2 — Dir Browser pravi listing

- `dir.list` / `dir.select` vraca **QNC URI** (AGENTS §16: prije Ingesta
  public output mora biti URI)
- Local tree, Gore, Diskovi, Odaberi, Odustani
- LAN/Internet ostaju v4 empty copy dok nema location registry

### Rez 3 — scanner + camera detector + Ingest registry/content

- Odaberi → scan.roles + grouping → clip rows u Ingest DB
- proxy metadata na istom clipu
- UI cita public views, ne host snapshot JSON

### Rez 4 — jedini Media Probe

- `media.probe.full` za original i proxy u istom prolazu
- upis `probe_records` dovoljan za player, filmstrip, wave, export
- UI poll status iz baze (v4 1500 ms), ne radi probe

### Rez 5 — import batch + poster + Filmstrip + Wave

- Uvezi selektirane
- Kopiraj original samo kao Ingest opcija / artifact, ne kao novi clip
- Filmstrip 14 pravih frameova; Wave peaks
- Generiraj postere ostaje v4 gumb za `no_card_thumb`

### Rez 6 — preview player

- Broadcast Player modul cita probe iz Ingest DB
- UI i dalje: poster dok player nije ready, label Odaberi klip
- Player ne smije ffprobe

### Rez 7 — batch lifecycle

- Kad nema queued/processing jobova i korisnik zatvori / batch zavrsi,
  proces staje (standalone). Shell tab ostaje vidljiv, ali Ingest workflow
  nije ziv.

## Predlozena UI odstupanja (traziti odobrenje, ne raditi tiho)

Ova lista **nije** odobrena. Bez odobrenja vazi 100% v4 pixels.

1. **Filmstrip sadrzaj u docku.** V4 crta 12 kopija postera. AGENTS trazi 14
   stvarnih frameova. Prijedlog: geometrija docka identicna; kad artefakt
   postoji, 14 frameova. To mijenja broj plocica. Zapisati prije koda.
2. **Keyboard scope ime.** V4 ucitava storyboard katalog za Ingest play/step.
   Prijedlog: isti akordi, action_id ostaju `play_pause` /
   `step_back_frame` / `step_forward_frame`. Novi ingest-* idovi samo za
   form akcije kojih nema u starom katalogu.
3. **All/Virtual tabovi.** Vizualno ostaju. Virtual ne filtrira ingest grid
   (v4 ponasanje). Ne dodavati Virtual sadrzaj.
4. **`playback_cache`.** U v4 je u istoj ingest SQLite. U novom modelu to je
   runtime cache playera, ne javni clip identitet. Smije ostati privatna
   Ingest/player tablica, ne public view.
5. **Nema HTTP hosta.** Ista forma, isti gumbovi, store in-process. To nije
   UI odstupanje.
6. **Batch exit.** Standalone se gasi nakon ingest-a. U shellu tab ostaje.
   Zapisati kao lifecycle, ne kao layout.

Nije odstupanje i **ne smije** se "popravljati":

- gumb `Odaberi` vs empty text `U redu`
- All/Virtual na Ingest chromeu
- `[` / `]` kao ±1 frame
- AI mining bez zapisa u bazu
- empty LAN/Internet recenice

## DB mapping (v4 → QNC contract)

V4 jedna project-local `ingest` baza. QNC contract dijeli registry/content.

| V4 | QNC predlozak |
| --- | --- |
| browse path / active source / options u `ingest_meta` | `ingest_registry.source_locations` + `source_sessions` |
| card/source identity | `ingest_registry.source_cards` |
| `ingest_assets` clip row | `ingest_content.clips` |
| original/proxy paths + container/codec | `clip_sources` + `clip_proxy` (proxy nije clip) |
| `probe_json` + fps/size/audio kolone | `probe_records` (puni set, jedan prolaz) |
| thumb path/status | clip artifact fields ili poster tablica uz clip |
| filmstrip/wave job + host moduli | `filmstrip_artifacts` / `wave_artifacts` |
| `ingest_jobs` + import batches | privatni Ingest job store, nije javni API |
| `playback_cache` | privatno, nije public_clips |

Javni identiteti: QNC URI + clip_id. Physical path samo u owner/resolver
runtime tablici.

## Test plan

- Contract/conformance: Ingest manifest, DB owner, workflow forbidden ops
- Unit: grouping (proxy nije clip), probe once, filmstrip 14, no ffprobe
  u Filmstrip/Wave/Player
- UI: layout contract brojevi, label order, empty/loading/error
- Live: isti window size pored v4 Ingest taba — pixel/struktura
- Dual-mode: standalone `qnc-ingest` i shell tab, ista baza
- Negative: Story/MA crate ne zove ingest workflow

## Sto nije u prvom Ingest rezu

- Media Assist / Story forme
- Pun LAN/Internet location catalog
- Export, Story markeri, virtual shots
- Centralni qnc-host

## Sljedeci implementacijski korak (nakon odobrenja ovog prijedloga)

Rez 0 zatvoren ovim dokumentima. Prvi kod: Rez 1 pasivna forma +
`contracts/ui/ingest.layout.json` kao izvor geometrije. Ne otvarati scanner
ni probe dok layout live test ne prode pored v4.
