# 06 — Rječnik podataka (sve tablice v5)

Izvor: DDL u kodu i mehanički popis SQL zapisa (289 pojava u ne-testnom kodu; multi-line naredbe i generički pisači `replace_object`/`set_meta` su nadopunjeni ručno). Stupac **Piše** navodi funkcije (datoteka:funkcija). v5 kod nije mjerodavan (00): ovo je popis podataka koje sustav treba.

## A. Globalna baza `project_store.db` (katalog)

| Tablica | Stupci | Piše |
|---|---|---|
| `projects` | `project_id` PK, `name`, `project_dir`, `created_at`, `updated_at`, `created_by`, `updated_by`, `last_opened_at` | `store:upsert_project_meta`, `record_project_opened`, `delete_project_row`; `db:backfill_project_dirs`, `project_dir_from_conn` |
| `app_settings` | `key` PK, `value` | `db:set_setting` (`active_project_id`, `project_tab_ui_state`, `keyboard_shortcuts_user`, `ui_appearance_user`) |
| `users` | `user_id` PK, `display_name`, `role` (zadano `editor`), `active`, vremena | `collab:start_session` |
| `sessions` | `session_id` PK, `user_id` FK, `station_id`, `client_label`, `created_at`, `last_seen_at` | `collab:start_session`, `touch_session` |
| `source_templates` | `source_template_id` PK, `name`, `description`, `source_kind`, `system`, `config_json` (prazno; stvarno u `_kv`), audit polja | `templates:ensure_templates_seeded` |
| `source_template_kv` | (`source_template_id`, `setting_key`) → `setting_value` | isto |
| `project_templates` | `template_id` PK, `name`, `description`, `system`, `settings_json` (prazno), `source_template_ids_json` (prazno), audit | `ensure_templates_seeded`, `create_user_template`, `delete_user_template`, `repair_*` |
| `project_template_kv` | (`template_id`, `setting_key`) → `setting_value` | isto, `migrate_global_json_blobs_to_kv` |
| `project_template_sources` | (`template_id`, `source_template_id`) | `ensure_templates_seeded`, `delete_*` |
| `module_state` | `module_id` PK, `enabled`, `updated_at` | `db:upsert_module_enabled` |

## B. Shell baza `shell.db`
`shell_settings(key PK, value)` ← `shell_store:set_setting`.

## C. Projektna baza `qnc_project.db`

### C.1 Project
| Tablica | Stupci | Piše |
|---|---|---|
| `project_settings` | `project_id` PK, `template_id`, `settings_json` (uvijek `'{}'`), `created_by`, `updated_by`, `created_at`, `updated_at` | `templates:save_project_settings`, `persist_project_settings_kv` |
| `project_settings_kv` | (`project_id`, `setting_key`) → `setting_value` (točkasti ključevi) | `kv:replace_object` iz `save_project_settings` |
| `project_template_snapshot` | `project_id` PK, `template_id`, `template_name`, `template_version`, `snapshot_json` (`'{}'`), `created_at` | `templates:save_template_snapshot` |
| `project_snapshot_kv` | (`project_id`, `setting_key`) → `setting_value` | `replace_object` u `save_template_snapshot` |
| `project_members` | (`project_id`, `user_id`) PK, `role`, `joined_at`, `last_seen_at` | `collab:start_session`, `touch_session` |
| `project_workflow_steps` | `step_id` PK, `project_id`, `plugin_id`, `tab_id`, `label`, `position`, `status` (`locked`/`active`/`complete`), `next_step_id`, `settings_json` | `templates:write_project_workflow`, `migrate_legacy_ingest_workflow` |
| `project_workflow_step_kv` | (`step_id`, `setting_key`) → `setting_value` | `migrate_workflow_step_kv`, `migrate_legacy_ingest_workflow` |
| `project_workflow_state` | `project_id` PK, `active_step_id`, `entry_step_id`, `updated_at` | `write_project_workflow`, migracija |
| `project_data_revisions` | `scope` PK, `revision`, `updated_at` | `db:bump_project_data_revision` (samo `ingest`) |

### C.2 Ingest
| Tablica | Stupci (ključni) | Piše |
|---|---|---|
| `ingest_meta` | `key` PK, `value` | `db:set_meta`, `store:save_selection` (ključevi: `active_source_id`, `browse_path`, `source_scan_root`, `card_root`, `card_locked`, `media_probe_batch`, `selection_revision`, `archive_original`, `poster_proxy_approved`) |
| `ingest_assets` | PK (`source_id`,`clip_id`); `name`, `media_id`, `duration_sec`, `resolution`, `codec`, `fps`, `source_fps_num/den`, `source_width/height`, `source_frame_count`, `source_duration_frames`, `has_video`, `probe_json`, `field_order`, `interlaced`, `source_class`, `proxy_recipe`, `has_audio`, `audio_channels`, `audio_sample_rate_hz`, `audio_sample_format`, `status`, `import_status`, `selected`, `thumb_color_a/b`, `thumb_status`, `thumb_error`, `source_path`, `original_path`, `proxy_path`, `project_proxy_path`, `thumb_path`, `card_thumb_path`, `file_extension`, `poster_source`, `read_from_card`, `card_locked`, `metadata_json`, `virtual_name` | `store:upsert_media_group`, `reconcile_source_assets`, `purge_non_video_clips`, `write_media_probe_result`, `record_media_probe_result_prepared_conn`, `save_selection`, `toggle_clip_selection`, `select_all_clips`, `queue_import`, `row_import_error`; `import_finish:complete_imported_clip`; `audio_wrap:*`; `poster_copy:apply_poster_copy_result`; `db:set_thumb_*`, `set_poster_proxy_generation_approved`, `backfill_virtual_names_for_selected`, `migrate_*`; `jobs:apply_thumb_proxy_job_result`, `mark_original_archive_copied_conn`, `payload_for_ingest_media_prepare_claim`; `media_pool/ingest_db:persist_clip_probe`, `repair_proxy_asset_index` |
| `ingest_jobs` | vidi 05 | `db:queue_ingest_job_payload`, `mark_ingest_job_*`, `requeue_terminal_*`; `jobs:*` |
| `ingest_import_batches` | `batch_id` PK, `source_id`, `status` (`preparing`/`filmstrip`/`waveform`/`done`), vremena | `store:queue_import`, `import_pipeline:set_batch_phase` |
| `ingest_import_batch_items` | PK (`batch_id`,`source_id`,`clip_id`), `media_job_type`, FK batch | `store:queue_import` |
| `playback_cache` | PK (`source_id`,`clip_id`); vidi 05 | `db:upsert_playback_cache_status` |
| `audio_waveforms` | `clip_id` PK, `status` (`missing`…), `a1_peaks`, `a2_peaks` (JSON), `peak_count`, `a1_path`, `a2_path`, `error`, `render_version`, `updated_at` | `waveform/store:mark`, `invalidate_legacy_waveforms` |

### C.3 Media pool / virtualni kadrovi
| Tablica | Stupci | Piše |
|---|---|---|
| `pool_clips` | `clip_id` PK, `status`, `added_at`, `updated_at` | `media_pool/db:sync_pool_from_ingest_db` |
| `media_pool_displayed_clips` | `clip_id` PK, `snapshot_signature`, `displayed_at`, `updated_at` | `media_pool/store:mark_displayed_clips`, `list_incremental_updates` |
| `media_pool_workflow` | `id=1`, `current_clip_id`, `playhead_frame/sec/tc/fps`, `mark_in_sec`, `mark_out_sec`, `active_virtual_shot_id`, `state_json`, `updated_at` | `media_pool/db:migrate_workflow_json`, `virtual_shots:rename_shot_id` |
| `media_pool_workflow_selection` | `clip_id` PK, `added_at` | `migrate_workflow_json` |
| `clip_transcripts` | `clip_id` PK, `status` (`none`/`processing`/`complete`), `text_body`, `transcript_json`, `updated_at` | `media_pool/transcripts:save_transcript_conn` |
| `clip_transcript_segments` | PK (`clip_id`,`segment_index`), `start_sec`, `end_sec`, `text` | isto |
| `virtual_shots` | vidi 03 (≈45 stupaca) | `virtual_shots/db:*` (reserve_root, add/derive/update, sinkronizacije, migracije, rename), `story/db:create_cover` (kategorija) |

### C.4 Story
Tablice i stupci: vidi 04, odjeljak 1 (`story_state`, `story_parts`, `story_markers`, `story_marker_slots`, `story_covers`, `story_object_history`).

## D. Ostalo
- `sdk_demo_state` (demo modul, `sdk_demo/store.rs`) — nije dio procedure.
- Datoteke kao zapisi (putanje su u bazi): `original/`, `proxy/`, `audio/`, `ingest/thumbnails/<clip>/poster.jpg`, `virtual_shots/<shot>/{cover,out_cover}.jpg`, filmstrip artefakti, `exports/`, `incoming/`.

## E. Sažetak vlasništva (potrebe, ne v5 stvarnost)
| Područje | Tablice |
|---|---|
| Project | `projects`, `app_settings`, `users`, `sessions`, `*_templates*`, `project_settings*`, `project_template_snapshot`, `project_snapshot_kv`, `project_members`, `project_workflow*`, `module_state` |
| Ingest | `ingest_*`, `playback_cache`, `audio_waveforms`, (kod nas: `clips`, `clip_sources`, `clip_proxy`, `probe_records`, `filmstrip_*`, `wave_artifacts`, registry, source_index) |
| Media Assist | `virtual_shots` (osim root), `clip_transcripts*`, (v5 `pool_clips` i `media_pool_*` nisu potrebe) |
| Story | `story_*` |
| Export | posao `export_hires` + izlazna datoteka |
