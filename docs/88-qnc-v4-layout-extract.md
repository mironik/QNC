# qnc_v4 layout extract (tocne mjere i boje)

Status: izvadak iz koda qnc_v4, samo dokument (bez koda i ugovora)  
Datum: 2026-09-18  
Root: `C:\Users\miron\Projects\QNC`  
Referenca: `C:\Users\miron\Projects\qnc_v4\qnc-app\src`

Svaka vrijednost je procitana iz navedene datoteke (konstante `const` i
izracuni). Nije izmjereno na ekranu; live usporedba je obvezna prema
`docs/07-ui-layout-reference.md` i ovdje nije provedena.

## 1. Teme (`qnc_theme.rs`, `ThemeTokens`)

Tri teme: `Dark` (zadana), `Soft`, `High contrast`.

| Token | Dark | Soft | High contrast |
|---|---|---|---|
| bg | 11, 15, 25 | 22, 27, 38 | 0, 0, 0 |
| surface | 17, 24, 39 | 32, 40, 56 | 18, 18, 18 |
| raised | 31, 41, 55 | 45, 55, 74 | 36, 36, 36 |
| border | 55, 65, 81 | 75, 88, 110 | 180, 180, 180 |
| text | 229, 231, 235 | 236, 239, 244 | 255, 255, 255 |
| muted | 156, 163, 175 | 168, 178, 194 | 200, 200, 200 |
| accent | 16, 185, 129 | 52, 199, 148 | 0, 255, 170 |
| select_red | 239, 68, 68 | 248, 113, 113 | 255, 80, 80 |
| teal_wave | 45, 212, 191 | 94, 234, 212 | 0, 230, 200 |
| preview_black | 0, 0, 0 | 8, 10, 14 | 0, 0, 0 |
| tc_gold | 240, 180, 0 | 251, 191, 36 | 255, 220, 0 |
| focus | 255, 180, 60 | 255, 196, 90 | 255, 200, 0 |

## 2. Chrome i tipografija (`qnc_theme.rs`)

| Naziv | Vrijednost |
|---|---|
| FONT_UI | 14.0 |
| FONT_TC (timecode) | 13.0 |
| CHROME_ROW_H | 28.0 |
| CHROME_PAD_X / CHROME_PAD_Y | 8 / 2 |
| CHROME_CTRL_H | 24.0 |
| Globalni razmak gumba / stavki | button_padding (10, 6), item_spacing (8, 6) |
| Razmak u chrome retku | button_padding (8, 2), item_spacing (8, 0) |
| Transport gumb | sirina 40 (naziv na "Export": 120), visina 24, monospace 14 |
| Aktivni transport gumb | tekst i obrub accent, ispuna accent x 0.16 |

## 3. Shell (`qnc_ui.rs`, `space`)

| Naziv | Vrijednost |
|---|---|
| LEFT_RATIO | 0.365 |
| LEFT_MIN_W | 280.0 |
| DIV_W (razdjelnik) | 5.0 |
| RIGHT_MIN_W | 200.0 |
| SHELL_MARGIN_X | 8 |
| BLOCK_PAD | 10 |
| GAP | 8.0 |
| PREVIEW_RESERVE_BELOW | 190.0 |
| PREVIEW_MIN_H | 160.0 |
| BODY_MIN_H | 96.0 |
| Project: PROJECT_LEFT_RATIO / min lijevo / min desno | 0.32 / 260 / 360 |

Izracuni: `left_w = max(avail.x * 0.365, 280)`,
`right_w = max(avail.x - left_w - 5, 200)`; visina previewa =
`min((left_w - 32).max(240) * 9/16, (h - 190).max(160)).max(160)`.
Lijevi stupac ima plohu `surface`, desni `bg`, razdjelnik boju `border`.

## 4. Preview (`qnc_ui::preview`)

Ploha `preview_black`, slika "contain" i centrirana, prazan natpis centriran,
proportional 14, boja `muted`. Natpis Story/MA: "Odaberi klip"; u Wrap nacinu
"Playlist input".

## 5. Glava media pool-a (`editorial/media_pool.rs`)

| Element | Vrijednost |
|---|---|
| Tabovi (redom) | All, Virtual, B-roll (Cover), Segment |
| Razmak izmedju tabova | 10 |
| Aktivni tab | boja `text`, strong, crta 2 px `accent` ispod (+1 px) |
| Neaktivni tab | boja `muted` |
| Transport zdesna nalijevo | Export HI-res ili "Export...", B (Quick cover), ], [, > ili \|\| |
| Tooltipovi | Quick cover, Mark OUT, Mark IN, Play / Pause |

## 6. Kartica i mreza (`qnc_media_card.rs`)

| Naziv | Vrijednost |
|---|---|
| GRID_GAP | 10.0 |
| CARD_TEXT_H | 34.0 |
| MIN_CARD_W | 160.0 |
| MIN_CARD_H | 160 * 9/16 + 34 |
| Iskoristiva sirina | `available - 8`, stupci = `floor((usable + 10) / 170)`, najvise koliko kartica |
| Visina sličice | `max(card_h - 34, 72)` |
| Fokus okvir | 2 px `select_red`, inace 1 px `border` |
| Ploha kartice / sličice | `raised` / `surface` |
| Kvačica | 16 x 16, razmak 6 od donjeg lijevog kuta, radijus 3 |
| Kvačica ukljucena | ispuna 255,149,0; kvačica 26,26,26 debljine 2 |
| Kvačica iskljucena | ispuna crna alfa 90, obrub bijela alfa 140 debljine 1.5 |
| Ime | Small font, max znakova `clamp(floor(w/7), 8, 42)` s "…" |
| Točkice | radijus 3.5; druga na +10 px; zauzimaju 12 ili 22 px |
| Tocka "ready" (proxy) | 48, 209, 88 |
| Tocka original "ready" | 10, 132, 255 |
| Tocka "pending" | 255, 214, 10 |
| Tocka ostalo/greska | 255, 69, 58 |

Napomena: `paint_media_card` u v4 koristi fiksne konstante Dark teme (`RAISED`,
`SURFACE`, `BORDER`, `TEXT`, `MUTED`, `SELECT_RED`), a ne `current(ui)`; kartica
dakle ne prati Soft / High contrast temu. Novi `qnc-media-card` dobiva boje od
forme, pa je to odstupanje (bolje) od v4.

## 7. Donji dock (`qnc_source_dock.rs`)

| Naziv | Vrijednost |
|---|---|
| Inset lijevo/desno | 8 |
| HEADER_TIMELINE_GAP | 4.0 |
| Header | chrome redak, visina 28, donja crta `border` |
| Visina docka | `timeline_content + 2 + 28 + 4` (s headerom) |
| Ploha | boja `bg` teme (timeline bg) |
| Slojevi (source) | carrier, A1, A2, shot_range, in_out, playhead |
| Story/MA gumbi zdesna nalijevo | Pokrivalice, Voice over, Talking Head, Add virtual clip |
| Ljevo | naziv klipa (strong 14), 10 razmaka, IN / OUT / Trajanje |
| Timecode labela | 13, vrijednost monospace strong boja `tc_gold`; fokus `focus` |

## 8. Timeline (`qnc_timeline.rs`)

| Naziv | Vrijednost |
|---|---|
| LABEL_COL_W | 28.0 |
| VIDEO_H | 64.0 |
| AUDIO_H | 15.0 |
| AUDIO_EXPANDED_H | 52.0 |
| ROW_GAP | 3.0 |
| Visina za source slojeve | 15 + 3 + 64 + 3 + 15 = 100, s obrubom 102 |
| BG | 11, 15, 25 |
| VIDEO_BG / AUDIO_PRIMARY_BG | 17, 24, 39 |
| AUDIO_SECONDARY_BG | 15, 23, 42 (u kodu docka bg teme) |
| LABEL_BG | 31, 41, 55 |
| LINE | 55, 65, 81 |
| WAVE_A1 / A2 / A3 / A4 | 16,185,129 / 107,114,128 / 75,85,99 / 55,65,81 |
| FOCUS | 255, 180, 60 |
| PENDING_COVER | 251, 146, 60 |
| SLOT_SELECTED | 15, 118, 110 |
| PLAYHEAD | **78, 201, 176**, debljina 1.5 (`qnc_timeline.rs:1065`) |
| IO_HANDLE | bijela |

## 9. Filmstrip pozadina (`qnc_filmstrip_background.rs`)

THUMB_W 112.0; SEAM (17, 24, 39) = `surface`; FRAME (31, 41, 55) = `raised`.

## 10. Location browser (`qnc_location_browser.rs`)

UP_COL_W 42.0, DISKS_COL_W 58.0, NAV_GAP_W 12.0.

## 11. Form kit, Project (`qnc_form.rs`)

PAD_X 8, PAD_Y 8, INSET_X 8, SECTION_GAP 8, INLINE_LABEL_W 168, INLINE_BTN_W 120,
INLINE_COL_GAP 8, FIELD_MIN_W 160, FIELD_GAP_X 8, FIELD_GAP_Y 8, LABEL_FS 12,
GROUP_TITLE_FS 12, ROW_H 24.

## 12. Story paneli

| Datoteka | Vrijednosti |
|---|---|
| `editorial/segment_panel.rs` | PANEL_MARGIN 10.0, BOTTOM_GAP 4.0, PLAYLIST_INPUT_H 128.0 |
| `editorial/marker_cover_panel.rs` | COMPACT_CTRL_H 22.0, EDIT_ACTIONS_W 250.0, RIGHT_ACTIONS_W 110.0, EDIT_ACTION_GAP 6.0, CONTROL_GROUP_GAP 10.0, TRANSPORT_BTN_W 30.0, TRANSPORT_GAP 5.0, TRANSPORT_CONTROLS_W = 30 * 7 + 5 * 6 |
| `editorial/program_waveform.rs` | PROGRAM_PEAK_BUCKETS 1200, MIN_PROGRAM_PEAK_BUCKETS 24 |
| `qnc_segment_timeline.rs` | ROW_GAP 3.0, BG (11,15,25), LINE (55,65,81) |

## 13. Razlike prema novom QNC-u

Usporedjeno s `contracts/ui/shell.layout.json`, `ingest.layout.json`,
`editorial.layout.json` i `crates/qnc-timeline`.

| Podrucje | v4 | Novi QNC | Ishod |
|---|---|---|---|
| Shell, preview, kartica, dock razmaci | vidi 3, 6, 7 | isto (`editorial.layout.json`) | jednako |
| Chrome i fontovi | 14 / 13 / 28 / 24 | `shell.layout.json` `theme_metrics` | jednako |
| Boja **playheada** | (78, 201, 176) | `qnc-timeline` koristi `accent` (16, 185, 129) | **razlicito** |
| Boja wave A2 | (107, 114, 128) | (107, 114, 128) | jednako |
| Teme | tri (Dark, Soft, High contrast) | jedna paleta u ugovoru | **nedostaju dvije teme** |
| Tokeni `select_red`, `teal_wave`, `tc_gold`, `preview_black` | u temi | nisu u ugovoru (`preview_black` i `select_red` vrijednosti su ugradjene u modul/demo) | **nedostaju u ugovoru** |
| Pozadina druge audio trake | u konstanti (15, 23, 42), u kodu docka `bg` | `bg` | nejasno, v4 ima dvije |

Zabiljezeno kao nalaz, ne kao izmjena: `qnc-timeline` je zamrznut (AGENTS.md
§17), pa ispravak boje playheada trazi izricito otkljucavanje.

## Nije provjereno

- Live usporedba na ekranu s v4.
- Tocne mjere `story.rs`, `segment_panel.rs` i `marker_cover_panel.rs`
  osim konstanti (izracuni unutar funkcija nisu izvadjeni).
- Shell (`app.rs`): footer, naslovna traka, izbor teme; izvadjene su samo
  konstante, a raspored nije.
- Tocna Soft/High contrast primjena na timeline boje.

Status: zamrznuto.
