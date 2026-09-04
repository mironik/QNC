# Ingest UI reference audit

Status: qnc_v4 layout snapshot + prijedlog prekodiranja  
Datum: 2026-09-04  
Root: `C:\Users\miron\Projects\QNC`  
Referenca: `C:\Users\miron\Projects\qnc_v4`

## Izvori

Pregledani qnc_v4 izvori:

```text
seed/tabs/ingest/plugin.json
qnc-app/src/ingest/mod.rs
qnc-app/src/ingest_player.rs
qnc-app/src/app.rs
qnc-app/src/composition.rs
qnc-app/src/qnc_ui.rs
qnc-app/src/qnc_theme.rs
qnc-app/src/qnc_location_browser.rs
qnc-app/src/qnc_source_dock.rs
qnc-app/src/qnc_media_card.rs
qnc-app/src/editorial/media_pool.rs
qnc-app/src/components/source_import_command.rs
qnc-host/src/ingest/api.rs
qnc-host/src/ingest/db.rs
qnc-host/src/ingest/store.rs
qnc-host/src/ingest/scanner.rs
qnc-host/src/ingest/import_pipeline.rs
qnc-host/src/ingest/orchestrator.rs
seed/keyboard-shortcuts.json
```

Pregledani QNC ugovori:

```text
AGENTS.md
contracts/applications/ingest.application.json
contracts/databases/ingest-registry.database.json
contracts/databases/ingest-content.database.json
docs/00-architecture-model.md
docs/01-application-ownership-matrix.md
docs/04-module-contract-matrix.md
docs/07-ui-layout-reference.md
```

## Zakljucak

Ingest je aplikacija/forma. Nije modul.

Aktivni qnc_v4 Ingest layout ide ovim putem:

```text
QncApp
  -> TopBottomPanel ingest_source_dock (exact_height = dock_height)
       -> IngestScreen::ui_timeline_dock
            -> qnc_source_dock::show (show_import_actions = true)
  -> CentralPanel
       -> IngestScreen::ui
            -> qnc_ui::editorial_shell
                 left:  qnc_ui::media_column_monitor
                        preview / broadcast monitor
                        media_pool::show_head (HeadFeatures::INGEST)
                        qnc_ui::content_panel
                          qnc_location_browser::show (id_salt = ingest)
                 right: media_pool::show_ingest_strip
```

Ingest **ne racuna** vlastitu geometriju shella. Koristi isti Story editorial
shell (`LEFT_RATIO = 0.365`). To nije redizajn: to je v4 izvor.

## Zamrznuta geometrija

Editorial shell (isto kao Story):

```text
left_ratio = 0.365
divider_width = 5.0
left_min_width = 280.0
right_min_width = 200.0
left_column_face = theme.surface
right_column_face = theme.bg
shell_margin_x = 8
block_pad = 10
gap = 8
preview_reserve_below = 190
preview_min_h = 160
preview_aspect = 16:9 contain, centered on preview_black
empty_preview_label = Odaberi klip
empty_preview_font = 14
```

Preview visina:

```text
preview_w = (left_w - 32).max(240)
preview_h = min(max(height - 190, 160), preview_w * 9/16).max(160)
```

Pool head chrome:

```text
chrome_row_h = 28
chrome_pad_x = 8
chrome_pad_y = 2
chrome_ctrl_h = 24
font_ui = 14
tabs_left = All, Virtual   (bez B-roll, bez Segment)
tab_gap = 10
transport_right = Play/Pause (> / ||), [, ]
nema: B (Quick cover), Export HI-res
```

Dir Browser u lijevom bodyju:

```text
label = Izvori
kinds = Računalo | LAN | Internet
kind_gap = 10
nav = Gore (42) + Diskovi (58) + breadcrumb
nav_gap = 12
tree_footer_reserve = chrome_ctrl_h + 8
error_gap = 4
tree_to_footer_gap = 8
footer_rtl = Odaberi (primary, enabled samo Local + mapa) | Odustani
confirm_label = Odaberi
empty_lan = Nema konfiguriranih LAN izvora.
empty_internet = Nema konfiguriranih Internet izvora.
```

Desni clip grid:

```text
content_panel full right height
card min_w = 160
card text_h = 34
card_h = card_w * 9/16 + 34
grid_gap = 10
features = selection_check + imported-only status dot
empty_message = Nema klipova — lijevo odaberi mapu → U redu.
id_salt = ingest_media_grid
```

Bottom source dock:

```text
show_header = true
show_edit_actions = false
show_import_actions = true
header_to_timeline_gap = 4
dock_height = timeline.content_height + 2 + chrome_row_h + 4
inner_margin_x = 8
left = clip_label (strong, 14)
right RTL, item_spacing.x = 8:
  AI mining checkbox
  Kopiraj original checkbox (samo ako archive_original_available)
  Očisti
  Odaberi sve
  Uvezi (primary, disabled ako nema selekcije ili command_busy)
  Generiraj postere (N) + Nema postera na kartici  (samo no_card_thumb)
  Osvježi
  status: "{imp} uvezeno · {sel}/{total}" ili
          "{imp} uvezeno · {pending} u tijeku · {sel}/{total}"
```

## Korisnicke akcije (forma)

| Akcija | V4 UI | QNC action_id predlozak |
| --- | --- | --- |
| Vrsta izvora Računalo / LAN / Internet | tab | `ingest_source_kind_*` |
| Gore | link | `ingest_dir_up` |
| Diskovi | link | `ingest_dir_roots` |
| Otvori mapu u stablu | klik retka | `ingest_dir_open` |
| Odaberi (potvrdi mapu) | primary | `ingest_dir_confirm` |
| Odustani | action | `ingest_dir_cancel` |
| Klik kartice | fokus preview | `ingest_preview_focus` |
| Check na kartici | toggle select | `ingest_clip_toggle` |
| Play / Pause | head `>` / `||` | `play_pause` |
| Mark IN / OUT na Ingestu | `[` / `]` = ±1 frame | `step_back_frame` / `step_forward_frame` |
| Uvezi | dock | `ingest_import_selected` |
| Odaberi sve | dock | `ingest_select_all` |
| Očisti | dock | `ingest_clear_selection` |
| Osvježi | dock | `ingest_reload` |
| Kopiraj original | checkbox | `ingest_set_archive` |
| AI mining | checkbox, lokalno | `ingest_set_ai_mining` |
| Generiraj postere | dock | `ingest_approve_proxy_posters` |
| Scrub timeline | dock | `ingest_cue_frame` |
| Expand A1/A2 | klik labele | `ingest_toggle_audio_lane` |

Plugin backend akcije (v4 host): `ingest.discover`, `ingest.import`,
`source.change`, `clip.toggle`, `ingest.select-all`, `ingest.browse`,
`ingest.archive-original`, plus thumbs/waveform rute.

Tok nakon **Odaberi**: browse path + discover (scan + grupiranje original/proxy
+ jedini probe). **Uvezi** pokrece import batch (poster/proxy/filmstrip/wave).

## Sto Ingest nije

- Nije Project, Media Assist ni Story.
- Filmstrip, Wave, Media Probe, Dir Browser, Media Browser, Timeline,
  Broadcast Player nisu dio forme kao vlasnici. To su moduli.
- Lijevi All/Virtual tabovi su isti chrome kao Story. Na v4 Ingestu desni
  panel **uvijek** crta ingest clip grid; Virtual ne mijenja sadrzaj.
- `[` / `]` na Ingestu nisu Story mark IN/OUT. Mapiraju se na ±1 frame.
- `AI mining` u v4 je lokalni UI flag, nije zapis u ingest bazi.
- Dock filmstrip u v4 prije artefakata crta **12 ponovljenih postera**. To je
  v4 prikaz, ne Filmstrip zakon.

## Sto je provjereno

- Kompozicija forme (shell + dock + head + browser + grid)
- Zamrznuti brojevi iz `qnc_ui::space` i `qnc_theme`
- Labeli i redoslijed dock/browser gumba
- IngestAction i plugin actions
- Host schema: `ingest_meta`, `ingest_assets`, `ingest_jobs`,
  `ingest_import_batches`, `ingest_import_batch_items`, `playback_cache`
- Import pipeline faze: preparing → filmstrip → waveform → done
- Scanner: filesystem + grupiranje, upis u store
- QNC ownership: Ingest pise vlastite DB contracte, cita Project registry
- Postojeci QNC Dir Browser crate: `list_directory` postoji, UI widget je
  u Project desktopu, nije jos javni shared UI modul

## Sto nije provjereno

- Live pixel usporedba novog QNC Ingesta (ne postoji)
- LAN/Internet stvarni izvori (v4 prikazuje empty copy)
- Camera detector pravila po modelu kamere (grupiranje je u host media)
- Puni probe_json field set potreban za player/export
- Playback cache kao javni vs privatni Ingest contract
- Keyboard scope: v4 Ingest ucitava **storyboard** scope, ne poseban ingest scope
- Filmstrip 14 vs v4 dock 12 tiles u runtimeu

## Sljedeci rizik

1. Kopirati v4 host API kao komunikaciju izmedu aplikacija.
2. Pustiti UI da zove scan/probe/ffprobe.
3. Prikazati proxy kao zaseban clip.
4. Ponoviti poster kao filmstrip i proglasiti to Filmstrip modulom.
5. OS folder dialog umjesto ugradenog Dir Browsera.
6. Hardkodirati shortcut akorde u Ingest formu.
7. Ucitati cijeli Story/Media Assist desktop da se dobije isti chrome.

## Prijedlog prekodiranja

Vidi `docs/12-ingest-recoding-proposal.md`.
Layout contract: `contracts/ui/ingest.layout.json`.
