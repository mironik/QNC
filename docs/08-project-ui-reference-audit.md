# Project UI reference audit

Status: qnc_v4 layout snapshot  
Datum: 2026-09-04

## Izvori

Pregledani qnc_v4 izvori:

```text
seed/tabs/project/plugin.json
qnc-app/src/project/mod.rs
qnc-app/src/project/screen.rs
qnc-app/src/project/empty_story_layout.rs
qnc-app/src/project/layout.rs
qnc-app/src/project/project_list.rs
qnc-app/src/project/settings.rs
qnc-app/src/project/create.rs
qnc-app/src/project/template_picker.rs
qnc-app/src/project/ai.rs
qnc-app/src/qnc_ui.rs
qnc-app/src/qnc_form.rs
qnc-app/src/qnc_theme.rs
```

## Zakljucak

Project je aplikacija/forma. Nije modul.

Aktivni qnc_v4 Project layout ide ovim putem:

```text
ProjectScreen::ui
  -> empty_story_layout::project_board
  -> qnc_ui::column_shell
  -> left: project_list::show
  -> right: settings::show
  -> layout::pts_panel
```

`qnc_ui::project_workspace` postoji kao referenca/kompatibilni kod, ali aktivni
Project board iz `ProjectScreen::ui` koristi `empty_story_layout::project_board`
i `column_shell`.

## Zamrznuta geometrija

Project shell:

```text
left_ratio = 0.31
divider_width = 5.0
left_min_width = 280.0
right_min_width = 200.0
outer_padding = 0
```

Lijevi panel:

```text
panel_pad = 20
title = Projekti
title_row_height = 28
below_title_pad = 40
row_height = 60
row_gap = 10
delete_column_width = 28
column_gap = 8
```

Desni panel:

```text
panel_pad = 20
title = Postavke
subtitle = Odaberi radni tok i pregledaj postavke projekta.
inner_pad_x = 8
inner_pad_y = 8
section_gap = 8
inline_label_width = 168
inline_button_width = 120
field_min_width = 160
```

PTS redoslijed slotova:

```text
Head:
  Postavke
  subtitle

Fixed:
  TemplatePicker
  ProjectCreate
  AiSettings
  ProjectsRoot
  ExportDirectory
  TemplateActions

Scroll:
  Advanced
  CustomTemplate
```

## Pravilo za novi QNC

Novi Project UI mora biti doslovna preslika ovog layouta. Smiju se odvojiti
workflow i DB kod, ali vizualna forma, slotovi, redoslijed, font, razmaci,
poravnanja i labels ostaju isti dok korisnik eksplicitno ne odobri promjenu.

Kod koji se kasnije bude pisao za Project mora koristiti:

```text
contracts/ui/shell.layout.json
contracts/ui/project.layout.json
contracts/qnc-keyboard-shortcuts.json
```

UI ne smije pisati DB direktno niti sadrzavati Project poslovnu logiku.
