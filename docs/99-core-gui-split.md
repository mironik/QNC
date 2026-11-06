# 99. Logika i egui: gdje su pomijesani i plan razdvajanja

Korisnik 2026-11-06: "Analiziraj QNC i pronadi svako mjesto gdje je procesna logika pomijesana s egui
kodom: egui tipovi u logici, obrada podataka unutar update(), stanje koje zivi samo u GUI strukturi.
Predlozi podjelu na qnc-core i qnc-gui crate te popis koraka refaktoriranja, od najmanje rizicnog prema
vecem." Analiza je napravljena citanjem `Cargo.toml` svih paketa i koda; nista nije mijenjano.

## 1. Stanje

- 83 paketa izravno ovise o `eframe`/`egui` (paneli, ploce, forme, aplikacije, ali i nekoliko kocki
  koje su vecinom logika).
- 25 logickih paketa (bez vlastitog egui-ja) izravno ovisi o nekom UI paketu, pa preko njega vuce egui.
  Tranzitivno to znaci da se gotovo cijeli sloj price i Ingesta (prica, scenarij, preview, fokus) ne moze
  izgraditi ni testirati bez egui-ja.

### 1.1 Logika koja ovisi o UI paketu samo zbog tipa podataka (najlaksi rez)

| UI paket | Sto logika uzima | Tko uzima |
|---|---|---|
| `qnc-timeline` | `TimelineProjection`, `TimelineSourceMarkFocus`, `TimelineIntent`, `AudioLane`, `LaneKey`, `lane_key`, `lane_message`, `lane_choice` | `qnc-source-panel`, `qnc-source-mark-focus`, `qnc-pending-marks`, `qnc-keyframe-jump`, `qnc-player-timeline`, `qnc-script`, `qnc-side-pieces`, `qnc-transcript`, `qnc-subsegment-edit`, `qnc-segments-keys`, `qnc-source-preview`, `qnc-ingest-preview-source`, `qnc-ingest-application`, `qnc-panel-focus` |
| `qnc-alert` | `Alert` | `qnc-program-segments`, `qnc-program-view` |
| `qnc-choice-box` | `Choice`, `ChoiceOption` | `qnc-short-save`, `qnc-ingest-import-run`, `qnc-ingest-selection-write`, `qnc-ingest-application` |
| `qnc-monitor` | `MonitorState`, `MonitorFrame`, izbor Auto/On (`choose`, globalno stanje) | `qnc-source-preview` (i sve iznad njega) |
| `qnc-pool-pointer` | `Follow` | `qnc-source-preview` |
| `qnc-wheel-jog` | `Wheel` | `qnc-gesture-steps` |
| `qnc-shell-desktop-api` | `DesktopApplicationRef` | `qnc-desktop-tabs` |
| `qnc-program-output` | `screens`, `output_screen` (popis ekrana) | `tools/qnc-player-acceptance` |

### 1.2 Kocke koje su logika i crtanje u istom paketu

| Paket | Logika u njemu | egui u njemu |
|---|---|---|
| `qnc-source-timeline` | stanje A1/A2 (kanal, razina, off), zvuk klipa iz baze (`qnc-clip-sound`), timecode, par rezova i rijeci, tipke (`route`, `route_input`) | `show`, gumbi zaglavlja, traka razine |
| `qnc-timeline` | model projekcije (playhead, raspon, IN/OUT, `preserving_source_marks_from`), pravila A1/A2 traka (`lanes.rs`) | crtanje, pogodak misa, mapa gesta |
| `qnc-focus-walk` | krug panela, redovi, popisi, koraci | `list_keys`, `list_focus` (citaju egui unos) |
| `qnc-keyboard-shortcut` | citanje kataloga, akordi, pomoc | `egui_shortcut_events`, `pressed_while_typing`, `consume_egui_action_presses` |
| `qnc-newsroom-sheet` | listovi obrasca priloga: citanje i pisanje JSON datoteka racunala (`sheets.rs`) | overlay i uredivac lista |
| `qnc-monitor` | stanje monitora, Auto/On | crtanje slike |
| `qnc-panel-focus` (logika) | redoslijed kocki tipki | ponovno izvozi `qnc_ui_kit::paint_focus` |

`qnc-project-session` (Project, zamrznut) drzi stanje u UI paketima: `qnc-location-field`,
`qnc-newsroom-sheet`, `qnc-dir-browser-panel` (njihovi tipovi radnji i odgovora).

### 1.3 Stanje koje zivi samo u egui memoriji

Nadeno u 24 paketa (`insert_temp`, `data_mut`). Dvije vrste:

- **Izbor prikaza** (legitimno po AGENTS: "izbor prikaza ovog procesa, nigdje se ne pise"): je li
  otvoren prozor medija (`qnc-media-overlay`), koji je tab desnog panela (`qnc-right-panel`), koja je
  vrsta otvorena u editoru MA Scripta (`qnc-script-writer-panel`), skrol do oznacenog teksta.
- **Stanje rada koje bi trebalo imati vlasnika**:
  - nacrt teksta izjave prije Entera: `qnc-script-panel/text_editor.rs` (drzi ga egui, ne `qnc-script`);
  - pojam pretrage i nalazi prijepisa: `qnc-transcript-panel/search.rs`;
  - povlacenje M markera, strane J/L i IN/OUT: `qnc-timeline` ga sprema u egui memoriju, a paneli ga
    citaju kroz `take_marker_drag(ctx)` / `take_marker_drag_event` - skriveni kanal izmedu kocki preko
    egui konteksta umjesto povratne namjere (`TimelineIntent`);
  - "pritisak koji je spremio" (`qnc_timeline_kept_press`) i slicne geste - granicno, to je stanje geste.

### 1.4 Obrada unutar `update()`

- Same `update()` funkcije su tanke (`qnc-desktop`, `qnc-editorial-desktop`, `qnc-ingest-desktop`,
  `qnc-project-desktop`): crtaju plocu. To je dobro.
- Ali sav rad aplikacije tece iz petlje crtanja: `qnc-form-host::before_paint` zove
  `FormApplication::poll` (citanje baze, player, valovi, prijepisi, prerender budenje) i `dispatch`;
  Project zove `session.poll()` u `show_desktop`; ljuska cita launch bar u `draw`. Kad se crtanje
  uspori ili stane (minimiziran prozor, `paint-pace` dok player svira), staje i taj rad.
- Poll citanja su vecinom vec u pozadinskim nitima (`qnc-clip-follow`, `qnc-store-jobs`, citaci price),
  pa je glavni rizik rad koji se radi *u samom* pollu: slaganje prikaza (`refresh_view`), prijedlog
  pomocnika tona, val programa, kad se pozivaju svaki frame.

## 2. Prijedlog podjele

Doslovno dva paketa `qnc-core` i `qnc-gui` bila bi dva monolita, a AGENTS to zabranjuje (odjeljak 0.11:
kocka -> modul -> panel -> forma; odjeljak 12: "nije dozvoljeno sakriti monolit"). Predlazem istu podjelu
kao **pravilo slojeva**, ne kao dva paketa:

- **core sloj**: svaka kocka bez `eframe`/`egui` u stablu ovisnosti (model, pravila, baza, player,
  radnici). Tu ide sve sto se moze testirati bez prozora.
- **gui sloj**: kocke koje crtaju ili citaju egui unos (`*-panel`, `*-blocks`, `*-desktop`, `qnc-ui-kit`,
  crtanje timelinea, monitora...). Smiju ovisiti o core, nikad obrnuto.
- Mjesovita kocka `qnc-x` se dijeli na `qnc-x` (core, isto ime da se potrosaci ne mijenjaju) i
  `qnc-x-view` (gui). Granicu cuva conformance: core paket s egui u stablu je greska.

Ako se zeli i fizicka podjela, moze biti direktorij (`crates/core/*`, `crates/gui/*`) bez novih
"umbrella" paketa.

## 3. Koraci, od najmanje rizicnog

1. **Conformance pravilo, samo citanje (rizik: nikakav).** Popis core paketa s egui u stablu
   ovisnosti, upisan kao baseline koji smije samo padati (kao C1-C13). Danas 25 izravnih.
2. **Cisti tipovi podataka u core kocke (rizik: nizak; samo premjestanje, UI paket ih ponovno izvozi).**
   - `qnc-timeline-model`: `TimelineProjection`, `TimelineSourceMarkFocus`, `TimelineIntent`,
     `AudioLane`, `LaneKey` i pravila traka (`lanes.rs`). Oslobada ~14 logickih paketa.
   - `Alert` i `Choice` u svoje male core kocke (crtanje ostaje u `qnc-alert`, `qnc-choice-box`).
   - `MonitorState`/`MonitorFrame` i izbor Auto/On iz `qnc-monitor`; `Follow` iz `qnc-pool-pointer`;
     `Wheel` iz `qnc-wheel-jog` (`qnc-gesture-steps` je onda core); popis ekrana iz `qnc-program-output`.
   - Ukloniti ponovni izvoz `paint_focus` iz `qnc-panel-focus`.
3. **Mjesovite kocke podijeliti (rizik: srednji; potpisi ostaju, mijenjaju se ovisnosti).**
   - `qnc-keyboard-shortcut` -> core katalog + `qnc-keyboard-egui` (pretvorba egui dogadaja).
   - `qnc-focus-walk` -> core krug + egui citanje popisa u gui kocki.
   - `qnc-source-timeline` -> core (`SourceTimeline` stanje, tipke, zvuk klipa) + `qnc-source-timeline-view`
     (`show`, zaglavlje). Nakon toga `qnc-editorial-application`, `qnc-ingest-application`,
     `qnc-panel-focus` i `qnc-segments-keys` vise ne vuku egui.
   - `qnc-newsroom-sheet` -> core listovi (datoteke) + overlay. Trazi otkljucavanje Projecta zbog
     `qnc-project-session`.
4. **Stanje rada iz egui memorije vlasnicima (rizik: srednji; mijenja ponasanje forme).**
   - povlacenje markera, J/L i IN/OUT vraca `qnc-timeline` kao namjeru iz `show()`, bez kanala kroz
     `ctx` (`take_marker_drag`);
   - nacrt teksta izjave u `qnc-script` (preziveljava prebacivanje panela, moze se testirati);
   - pojam pretrage prijepisa u `qnc-transcript`.
   Izbor prikaza (tabovi, otvoren prozor) moze ostati u egui memoriji.
5. **Rad aplikacije neovisan o crtanju (rizik: najveci; mjeriti prije i poslije).** Poll aplikacije u
   vlastitu nit ili ritam (ticker), forma samo uzima gotov prikaz (kanal ili zadnja snimka). Prvo
   izmjeriti sto `form-poll` stvarno trosi (`qnc-dev-diagnostics` ga vec mjeri) i raditi samo ako se
   pokaze da crtanje koci rad ili obrnuto.

Svaki korak: testovi dotaknutih kocki, zlatne tablice tipki i misa nepromijenjene, conformance,
zapis u AGENTS.md. Koraci 1-3 ne mijenjaju ponasanje.

## 4. Izvedeno

- Korak 1 (2026-11-06): conformance C14 "core piece pulls egui", 36 veza u baselineu.
- Korak 2 (2026-11-06): nove core kocke `qnc-timeline-model` (projekcija, fokus oznake, trake, namjere
  timelinea), `qnc-notice` (`Alert`, `Choice`), `qnc-monitor-state` (stanje monitora, Auto/On),
  `qnc-desktop-entry` (`DesktopApplicationRef`); `Wheel`/`WheelActions` u `qnc-edit-step`; `LaneKey`,
  `lane_key`, `lane_message` u `qnc-lane-choice`; `Follow` u `qnc-source-preview`; ponovni izvoz
  `paint_focus` uklonjen iz `qnc-panel-focus`. UI kocke ponovno izvoze premjestene tipove. C14: 36 -> 11.
  Ostaju veze na `qnc-source-timeline`, `qnc-focus-walk`, `qnc-keyboard-shortcut`, `qnc-newsroom-sheet`
  i popis ekrana iz `qnc-program-output` (korak 3).
- Gui-kit (2026-11-06, korisnik "mozda uvesti i gui-kit"): gui-kit su `qnc-ui-kit` (gumbi, okvir fokusa,
  slika, traka potvrde), `qnc-desktop-theme` (boje i mjere iz ugovora) i `qnc-theme-styles` (izgled svake
  kocke iz teme); nema novog paketa. Conformance C15: kocka koja crta (osim forme, C2, i samog gui-kita)
  ne pravi vlastiti gumb ili kvacicu (`Button::new`, `ui.button`, `ui.checkbox`, `Checkbox::new`) ni boju
  mimo teme (`from_rgb`, `from_rgba`); gui-kit ne ovisi ni o jednoj logickoj kocki osim dijagnostike i
  ploce. Danas 25 kljuceva u baselineu (najvise boje u `qnc-timeline`, `qnc-shell-footer`,
  `qnc-trim-overlay`, `qnc-media-card`; gumbi u `qnc-segment-panel`, `qnc-subsegment-panel`,
  `qnc-script-profile-editor`, dijagnostickom alatu); smiju samo padati. Polja za upis teksta
  (`TextEdit`) jos nisu u pravilu: gui-kit nema svoje polje.
- Tekstualno polje gui-kita (2026-11-06): `qnc-ui-kit` `text_field`, `row_field` (uz oznaku, visine reda,
  tekst u sredini), `text_area`, `search_field`. Sve kocke koje su pisale vlastiti `egui::TextEdit`
  koriste ga (`qnc-newsroom-sheet`, `qnc-settings-form`, `qnc-settings-rows`, `qnc-script-panel`,
  `qnc-text-source-editor`, `qnc-transcript-panel`); C15 broji i `TextEdit::`, bez novog kljuca. Ostaje
  Project ploca (`qnc-project-blocks`, forma, zamrznuta, C2 ga ne broji) i test kataloga tipki.
- Gui-kit ociscen (2026-11-07): sve kocke koje crtaju gumbe, kvacice, tekstualna polja i boje uzimaju iz gui-kita
  (`qnc-ui-kit`: `look_button`, `small_button`, `sized_button`, `framed_button`, `check_box`, `text_field`,
  `row_field`, `text_area`, `search_field`; `qnc-desktop-theme`: `rgb`, `danger`, `done`, `total_text`,
  `log_colours`; `qnc-theme-styles`: tema timelinea i trim zooma, kartice, palete ljuske). C15 baseline 25 -> 0.
  Ostaje Project ploca (zamrznuta forma, C2) i zadatak iz AGENTS 16: palete Soft/High contrast u UI ugovor.
