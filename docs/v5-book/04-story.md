# 04 — Story: operacije, zapisi, izvedeni izlazi

Kod: `qnc-host/src/story/{db,markers,covers,object_history,timeline_model,api}.rs`, `editorial_playlist.rs`, `program_playlist.rs`, `export_*`, `qnc-program-playlist`, `qnc-app/src/story*.rs`. Detaljna pravila i korisnički tok: **docs/93** (pravila) i **docs/94** (marker); ovdje je popis operacija i zapisa u obliku knjige. v5 kod nije mjerodavan (00).

## 1. Zapisi Storyja (projektna baza)

| Tablica | Ključ | Sadržaj | Piše |
|---|---|---|---|
| `story_state` (1 red, `id=1`) | – | `selected_part_id`, `selected_shot_id`, `selected_slot_id`, `selected_cover_id`, `draft_updated_at`, `committed_at`, `updated_at` | sve operacije (`touch_draft`), `commit_story` |
| `story_parts` | `part_id` | `kind` (`tonovi`/`offovi`), `sort_index`, `title`, `text`, `clip_id`, `virtual_shot_id`, IN/OUT (`in_frame`, `out_frame`, sekunde, TC), `fps`, `source_fps_num/den`, `duration_frames/label/color_key`, `active`, vremena | `create_part_with_range`, `update_part`, `set_part_mark_in/out(_frame)`, `delete_part` (`active=0`), `reorder_part`, `sync_story_part_source_fps` |
| `story_markers` | `marker_id` | `timeline_frame`, `timeline_sec`, `tc`, `label`, `sort_index`, `system_role` (`program_start`/`program_end`/''), `origin_part_id`, `origin_local_frame/sec` | create/update/move/delete marker, `recompute_marker_slots`, brisanje dijela |
| `story_marker_slots` | `slot_id` (= potpis `start:S|end:E`) | `slot_index`, `start/end_frame`, `duration_frames`, sekunde, `start/end_marker_id`, `slot_signature` | `recompute_marker_slots` (briše i gradi ispočetka) |
| `story_covers` | `cover_id` | `slot_signature`, `slot_index`, `timeline_start/end_frame/sec`, `clip_id`, `virtual_shot_id`, `title`, `note`, `source_in/out_frame`, `source_fps(_num/den)`, TC, `sort_index` | `create_cover` (briše postojeći u slotu), `update_cover`, `delete_cover`, `normalize_covers_for_slots`, `restore_cover_from_snapshot` |
| `story_object_history` | (`object_type`,`object_id`) | `state` (`active`/`undone`), `snapshot_json`, `updated_at` | `store_snapshot` (samo `cover`) |
| `virtual_shots` | `shot_id` | source/short/cover kadrovi (03) | Story čita; `create_cover` ga i piše (prepis kategorije) |

## 2. Operacije (ruta → funkcija → zapis)

| Ruta | Funkcija | Tablice koje piše | Ključni preduvjeti/greške |
|---|---|---|---|
| `GET /api/story/state` | `load_state` → `load_snapshot` | – (čita sve, sastavlja `all_clips`, `virtual_shots`, `cover_shots`, `parts`, `markers`, `marker_slots`, `covers`) | – |
| `POST part/create` | `create_part_with_range` | `story_parts`, `story_state`, markeri/slotovi | `invalid kind`; izvor iz kadra ili `clip_id` + frame IN/OUT (`Segment source range nedostaje`, `OUT mora biti poslije IN`) |
| `POST part/update` | `update_part` | `story_parts` (`kind`, `title`, `text`) | `part not found` |
| `POST part/mark_in` / `mark_out` | `set_part_mark_in/out(_frame)` | `story_parts` + preračun | lokalni frame u `[0, trajanje]`; `OUT mora biti poslije IN` |
| `POST part/delete` | `delete_part` | `story_parts.active=0`, `story_markers` (unutar prozora brisano, iza pomaknuto), `story_state` | `part not found` |
| `POST part/reorder` | `reorder_part` | `story_parts.sort_index` | `invalid direction` |
| `POST part/select` | `select_part` | `story_state.selected_part_id` | `part not found` |
| `POST shot/select` | `select_shot` | `story_state.selected_shot_id` | `virtual shot not found` |
| `POST marker/create` | `create_marker(_from_frame/_from_part_frame)` | `story_markers`, `story_marker_slots`, `story_covers`, `story_state` | frame ≥ 0; program fps valjan |
| `POST marker/update` | `update_marker(_frame)` | isto | unutar `[0, trajanje]`; početni/završni zaključani; nema dva na istom frameu |
| `POST marker/move` | `move_marker` | isto | zamjena frameova sa susjedom (`up`/`down`) |
| `POST marker/delete` | `delete_marker` | isto | zaključani početni/završni; `marker not found` |
| `POST marker_slot/select` | `select_marker_slot` | `story_state.selected_slot_id` | `slot not found` |
| `POST cover/create` | `create_cover` (host) | `story_covers`, po potrebi `virtual_shots` (novi kadar + kategorija `cover`), `story_state.selected_shot_id/selected_cover_id`, `story_object_history` | slot mora postojati; izvor: `virtual_shot_id` ili `clip_id` + frame IN/OUT; `odaberi virtualni kadar za pokrivanje` |
| `POST cover/update` | `update_cover` | `story_covers` (`title`, `note`, `clip_id`, `virtual_shot_id`) | – |
| `POST cover/delete` | `delete_cover` | `story_covers` | – |
| `POST cover/select` | `select_cover` | `story_state.selected_cover_id` | – |
| `POST object/undo` / `redo` | `undo_object` / `redo_object` | `story_covers`, `story_object_history` | samo tip `cover` |
| `POST commit` | `commit_story` | `story_state.committed_at` | – |
| `GET playlist` | `build_editorial_playlist_with_broker` | (čita; `sync_story_part_source_fps` može pisati fps u dijelove) | – |
| `GET program-snapshot` | montažna lista + flat program (`PlaybackInput`) | – | prazan Story: bez programa |
| `GET timeline-model`, `…/source` | `timeline_model` | – | model za timeline (Wrap/Source) |
| `GET play-media` | `media_gateway.resolve_sync(PlaybackInput)` | – | `clip_id required` |
| `POST export/submit`, `export/hires/submit`, `GET export/status`, `POST export/cancel` | export (05) | `ingest_jobs` (tip `export_hires`) | montažna lista mora biti valjana i neprazna |

Sve rute serijalizira `serialize_project_write` po projektu (osim čitanja).

## 3. Izvedeni izlazi (ne trajni izvor istine)
1. **Montažna lista** (`EditorialPlaylist`): segmenti (`part_id`, `kind`, `clip_id`, globalni raspon, izvor u frameovima, `covers[]` s lokalnim rasponima i `source_offset`).
2. **Flat program** (`FlatProgramPlaylist`): `items[]` s izvorima `base_video`/`base_audio` (segment, A1) i `cover` (slika sloja `Cover`, zvuk A2); pravila u docs/93 §4.
3. **Preview** koristi `MediaAccessKind::PlaybackInput`, **export** `OriginalMaster`.
4. `revision` flat programa je u v5 tvrdo `0`.

## 4. Procedure korisničkog toka
Vidi docs/93 §5 (P1–P16), docs/94 (sve procedure markera) i docs/95 (jedinstveni obrazac naredbi I/O/M + Ctrl).

## 5. Zapisi koje Story mora imati da bi export i preview radili
- Za svaki segment: `clip_id` + `in_frame/out_frame` + `source_fps_num/den` (timebase mora odgovarati probeu klipa).
- Za svaku pokrivalicu: `clip_id`, `source_in/out_frame`, `source_fps_*`, `virtual_shot_id`, slot.
- Izvor medija po klipu razrješava se iz baze (ingest zapisi + postavke); Story ne pretpostavlja lokalni direktorij.
