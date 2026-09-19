# 07 — Popis API ruta v5 (107, mehanički izvučeno)

Izvor: svi literalni `"/api/…"` u `qnc-host/src` (uključujući test URL-ove s parametrima). Grupirano po području; **P** = piše u bazu/FS, **Č** = samo čita. Poglavlje uz rutu navodi gdje je procedura opisana. v5 rute nisu mjerodavne; popis pokazuje koje su operacije bile potrebne.

## Project / shell
| Ruta | Č/P | Poglavlje |
|---|---|---|
| `GET/POST /api/projects` (popis / kreiranje) | Č/P | 01 P1.3 |
| `POST /api/projects/open`, `/delete`, `/cleanup-orphans`, `/from-template` | P | 01 P1.8, P1.9, P1.2 |
| `GET/POST /api/projects/ui-state` | Č/P | 01 P1.12 |
| `GET/POST /api/projects/{id}/settings` | Č/P | 01 P1.4 |
| `GET /api/projects/{id}/workspace`, `/data-revision` | Č | 01 P1.6, P1.14 |
| `GET/POST /api/project-templates`, `/delete`, `GET /{template_id}` | Č/P | 01 P1.10 |
| `POST /api/collab/session`, `/touch` | P | 01 P1.11 |
| `GET/POST /api/settings/keyboard-shortcuts`, `GET …/presets`, `GET/POST /api/settings/appearance` | Č/P | 01 P1.12 |
| `GET /api/modules`, `POST /api/modules/{id}/enable` | Č/P | 01 P1.13 |
| `/api/shell/{tabs, components, components/sync, db-first, diagnostics, fs-list, fs-roots, pick-directory, pick-files, projects-root, runtime, keyboard-shortcuts, background/playback}` | Č (osim `background/playback`: stanje reprodukcije za prioritet poslova) | 01, 05 |
| `GET /api/health`, `/api/sdk-demo/*`, `/api/design-tools/*` | pomoćne/razvojne | – |

## Ingest
| Ruta | Č/P | Poglavlje |
|---|---|---|
| `GET /api/ingest/state`, `/options` | Č | 02 §4 |
| `POST /api/ingest/source`, `/browse`, `/discover`, `/register-files` | P | 02 I2.1–I2.3 |
| `POST /api/ingest/selection`, `/selection/toggle`, `/selection/select-all` | P | 02 I2.6 |
| `POST /api/ingest/import` | P | 02 I2.7 |
| `POST /api/ingest/thumbs/from-proxy` | P | 02 I2.9 |
| `GET /api/ingest/thumbnail`, `POST /thumbnails/batch` | Č | 02 I2.9 |
| `GET /api/ingest/waveform/status`, `/peaks` | Č | 02 I2.11 |

## Poslovi (worker ↔ host)
`POST /api/jobs/claim`, `/heartbeat`, `/complete`, `/complete-batch`, `/fail`, `GET /api/jobs/status` — sve P (osim status) — 05 §3.

## Media / Media Assist
| Ruta | Č/P | Poglavlje |
|---|---|---|
| `GET /api/media/resolve` | Č | razrješenje medija po `MediaAccessKind` (proxy / original) |
| `GET …/clips`, `…/media`, `…/virtual-stream`, `…/thumbnail`, `…/filmstrip`, `…/filmstrip/package`, `…/filmstrip/placeholder`, `…/waveform/status` (prefiks `/api/story`) | Č | 03 M3.7 |
| `POST …/virtual-shot`, `…/virtual-shot/update` | P | 03 M3.5 |
| `GET …/virtual-shot/{id}/thumb` | Č (stvara sliku ako nedostaje) | 03 M3.5 |
| `POST /api/ai-search/transcribe-stream`, `/translate-transcript`, `GET /api/asr/health`, `/api/translation/health` | P/Č | 03 M3.6 |

## Story
| Ruta | Č/P | Poglavlje |
|---|---|---|
| `GET /api/story/state`, `/playlist`, `/program-snapshot`, `/timeline-model`, `/timeline-model/source`, `/play-media` | Č | 04 §2 |
| `POST /api/story/part/{create,update,delete,reorder,select,mark_in,mark_out}` | P | 04 §2 |
| `POST /api/story/shot/select` | P | 04 §2 |
| `POST /api/story/marker/{create,update,move,delete}`, `/marker_slot/select` | P | 04 §2, docs/94 |
| `POST /api/story/cover/{create,update,delete,select}` | P | 04 §2 |
| `POST /api/story/object/{undo,redo}`, `/commit` | P | 04 §2 |
| `POST /api/story/export/submit`, `/export/hires/submit`, `GET /export/status`, `POST /export/cancel`; `POST /api/render/hires/submit`, `GET /api/render/hires/status` | P/Č | 05 §5 |

## Napomene
- UI (`qnc-app`) ne otvara bazu: sve ide kroz ove rute (`ui_db_access = host_api_only`).
- Sve `POST` rute za projekt prolaze kroz `serialize_project_write`.
- Rute su „broker-first” samo djelomično; dio Story koda otvara bazu izravno (08).
