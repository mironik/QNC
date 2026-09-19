# 01 — Project: globalna baza, predlošci, projekt, postavke, tijek

Kod: `qnc-host/src/project/{db,store,templates,kv,collab,ui_state,keyboard_settings,appearance_settings,db_broker}.rs`, `shell_store.rs`.

## 1. Baze i direktoriji

| Baza / mjesto | Datoteka | Sadržaj |
|---|---|---|
| Globalna (katalog) | `<data_dir>/project_store.db` | `projects`, `app_settings`, `users`, `sessions`, `source_templates`, `project_templates`, `module_state`, `project_template_kv`, `project_template_sources`, `source_template_kv` |
| Shell | `<data_dir>/shell.db` | `shell_settings` (ključ/vrijednost) |
| Projektna | `<projects_root>/<project_dir>/qnc_project.db` | sve po projektu (vidi 06) |
| Direktorij projekta | `project_dir` iz `projects.project_dir` | `original/ proxy/ audio/ incoming/card incoming/ftp ingest/thumbnails filmstrip` (+ `virtual_shots/`, `exports/` nastaju kasnije) |

Pravila:
- `project_id` = `slug_id(naziv)` (slug + sufiks za jedinstvenost; primjeri iz prakse: `novi13_1789816756`).
- `open_project` (projektna baza) **odbija** nepoznat projekt (ne stvara mape za obrisane/nepoznate; u testovima dopušta).
- Broker (`ProjectDbBroker`): globalna kapija i kapija po projektu (`serialize_project_write`); UI ne otvara bazu (`ui_db_access = host_api_only`); u praksi dio koda (Story) otvara vezu izravno.

## 2. Procedure

### P1.1 Inicijalizacija globalne baze i seed predložaka
- Okidač: start hosta; `ensure_templates_seeded(conn, seed_path)` prije svakog čitanja predložaka.
- Čita: seed JSON (`seed_path`: `source_templates`, `project_templates`).
- Piše: `source_templates` (`ON CONFLICT DO NOTHING`); ako je red nov: `source_template_kv` (konfiguracija); `project_templates` (`ON CONFLICT DO NOTHING`); ako je red nov: `project_template_kv` (postavke, spljoštene u `setting_key` = točkasti put) i `project_template_sources` (popis izvora).
- Zatim: `repair_system_template_workspaces` (ispravlja `workspace.tabs` sistemskih predložaka), `repair_breaking_news_system_template` (upisuje predložak Breaking news ako nedostaje).
- Postuslov: sistemski predlošci postoje; korisnički se ne diraju.

### P1.2 Kreiranje projekta iz predloška (`create_project_from_template`)
- Okidač: `POST /api/projects/from-template` (`name`, `template_id`, `settings_override?`, `user_id`, `session_id?`).
- Čita: predložak (`get_project_template`: `project_templates` + `_kv` + izvori).
- Koraci:
  1. `label` = naziv ili „QNC projekt”; `project_id = slug_id(label)`.
  2. `settings` = postavke predloška; ako postoji `settings_override`, **dubinsko spajanje** (`deep_merge`); zatim `materialize_workspace_tabs`.
  3. `validate_project_export_settings` (nevaljan export → greška, ništa se ne piše).
  4. Dodaje se `settings.template` (`template_id`, `name`, `system`) i `settings.source_template_ids`.
  5. `projects_root` iz postavki (`storage.projects_root`, standardni token → zadani korijen) inače zadani; `project_dir = korijen/id`.
  6. `projects` ← `upsert_project_meta` (id, naziv, `project_dir`, vremena).
  7. FS: `ensure_project_dirs_at` (standardni raspored); ako postoji `export.directory`, stvara se i on.
  8. `save_project_settings` (P1.4) → projektna baza.
  9. `project_template_snapshot` + `project_snapshot_kv` ← snimka predloška u trenutku kreiranja (`save_template_snapshot`).
  10. `write_project_workflow` (P1.5).
  11. `app_settings.active_project_id` ← novi projekt; `projects.last_opened_at`.
  12. API još: `touch_collab_session` (ako je `session_id`), `save_ui_state` (zadnje odabrani predložak i naziv).
- Piše: globalna: `projects`, `app_settings`; projektna: `project_settings`, `project_settings_kv`, `project_template_snapshot`, `project_snapshot_kv`, `project_workflow_steps`, `project_workflow_state`; FS: direktoriji.

### P1.3 Kreiranje projekta bez predloška (`create_project`)
- Naziv zadano „Projekt N”; `projects` (upsert), `ensure_project_dirs`, `record_project_opened`, `set_active_project_id`. Bez postavki i bez workflowa (zaključak: koristi se rijetko).

### P1.4 Spremanje postavki projekta (`save_project_settings`)
- Okidač: `POST /api/projects/{id}/settings`.
- Piše: `project_settings` (jedan red: `template_id`, `settings_json = '{}'` (namjerno prazno), `created_by/updated_by/created_at/updated_at`); **stvarne postavke**: `project_settings_kv` (`replace_object`: briše sve retke projekta, upisuje spljoštene `setting_key`/`setting_value`); zatim `write_project_workflow`.
- Vraća `get_project_settings`.
- Pravilo: postavka nije objekt u jednom polju, nego ravni ključevi (`storage.ingest_media` …).

### P1.5 Tijek rada projekta (`write_project_workflow`)
- Iz `settings.workspace.tabs` (ako nema, iz `tab_labels`; `project` je uvijek prvi).
- Briše `project_workflow_steps` projekta; za svaki tab: `step_id = "step_<tab>"`, `plugin_id` (npr. `pool`→`media_pool`, `storyboard`→`story`, `media_assist`→`media_assist`), `label`, `position`, `status`:
  - `project` → `complete`;
  - prvi tab nakon `project` (ulazni korak) → `active`;
  - ostali → `locked`;
  - `next_step_id` povezuje korake redom.
- `project_workflow_state`: `active_step_id` = ulazni korak, `entry_step_id`, `updated_at`.
- **Napredovanje koraka (`complete` → sljedeći `active`) u kodu ne postoji**; postoji samo popravak/migracija (`migrate_legacy_ingest_workflow`: preimenuje stari `ingest_proxy` u `ingest`, resetira statuse).

### P1.6 Priprema radnog prostora (`prepare_project_workspace`)
- Okidač: otvaranje projekta u UI-ju / `GET …/workspace`.
- Koraci: `migrate_legacy_ingest_workflow`; čita `project_settings` + KV; `hydrate_project_settings` (predložak kao osnova, projekt nadjačava); `normalize_settings_workspace` + `materialize_workspace_tabs`; ako se rezultat razlikuje od spremljenog, `persist_project_settings_kv`; `ensure_project_workflow` (ako popis koraka u bazi ≠ očekivani, `write_project_workflow`); vraća snimku workspacea.

### P1.7 Efektivne postavke (`project_effective_settings`)
- Čita: `project_settings_kv` (projekt) i predložak (`project_template_kv`) → **predložak je osnova, postavke projekta su nadjačavanje**. Prazan predložak → samo projekt. Koriste ih Ingest, Media, Story, Export (čita se pri svakoj proceduri, ne kešira).

### P1.8 Otvaranje projekta (`open_project`)
- `app_settings.active_project_id` = id; `projects.last_opened_at`, `updated_at`. `POST /api/projects/open`.

### P1.9 Brisanje projekta (`delete_projects`)
- Okidač: `POST /api/projects/delete`.
- Redoslijed: ako je obrisani aktivan, prije brisanja postavlja se sljedeći aktivan (radnici ne ciljaju obrisano); zatim `DELETE FROM projects`, pa brisanje direktorija (i rezervne lokacije); zaključane datoteke daju grešku „obrisan iz baze, folder postoji”. `POST /api/projects/cleanup-orphans` čisti sirote direktorije.

### P1.10 Korisnički predlošci
- `create_user_template` (`project_templates` + `_kv` + `project_template_sources`); `delete_user_template` (briše tri tablice); `delete_breaking_news_custom_templates`. Sistemski predlošci se ne brišu.

### P1.11 Sesije i članovi (`collab`)
- `POST /api/collab/session`: `users` (novi `usr_<uuid>`, `display_name` zadano „QNC korisnik”, `role` zadano `editor`), `sessions` (`ses_<uuid>`, `station_id` zadano `unknown-station`, `client_label`), te u **projektnoj** bazi `project_members` (upsert).
- `POST /api/collab/touch`: `sessions.last_seen_at`, `project_members.last_seen_at`.
- Napomena: svaki poziv stvara **novog korisnika** (nema prepoznavanja povratnika).

### P1.12 Korisničke postavke (globalna baza `app_settings`, ključ/JSON)
| Ključ | Sadržaj | Ruta |
|---|---|---|
| `active_project_id` | aktivni projekt | open/create |
| `project_tab_ui_state` | stanje forme Project (odabrani predložak, naziv, override) | `/api/projects/ui-state` |
| `keyboard_shortcuts_user` | korisnički prečaci i aktivni preset | `/api/settings/keyboard-shortcuts` |
| `ui_appearance_user` | tema/izgled | `/api/settings/appearance` |

### P1.13 Moduli i shell
- `module_state(module_id, enabled, updated_at)` ← `upsert_module_enabled` (`POST /api/modules/{id}/enable`); `shell_settings(key,value)` ← `set_setting` (`shell.db`; npr. zadnje stanje shella).
- `/api/shell/tabs`, `/api/shell/components`, `/api/shell/db-first`, `/api/shell/pick-directory`, `/api/shell/fs-*`: čitanje/pomoć shellu (bez zapisa u projekt).

### P1.14 Revizije (`project_data_revisions`)
- `bump_project_data_revision(scope)` upsert (`revision + 1`, `updated_at`). Poziva se **samo za scope `ingest`** (audio_wrap, import_finish, poster_copy, store, jobs). `GET /api/projects/{id}/data-revision` vraća reviziju `ingest`. Story/media/virtual_shots ne dižu reviziju.

## 3. Zapisi koje Project mora imati da bi Ingest i ostali radili
| Ingest/ostali čitaju | Tablica |
|---|---|
| lokaciju projekta | `projects.project_dir` |
| što se uvozi/kopira | `project_settings_kv` (`storage.*`, `playback.input`, `input.mode`) + predložak |
| koje aplikacije i kojim redom | `project_workflow_steps` / `project_workflow_state` |
| gdje ide export | `project_settings_kv` (`export.directory`, `export.*`) |
| tko radi | `sessions`, `project_members` |
