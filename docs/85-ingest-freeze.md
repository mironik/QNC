# Ingest freeze

Status: zamrznuto

Zatvoreno ograniceno odobrenje 2026-09-13: Ingest ulaz ne skenira diskove.
`shell_next_group` pokaze povrsinu; aktivni projekt se cita iz baze.

Zatvoreno ograniceno odobrenje 2026-09-13: engine ne cita diagnostics log;
monitor mailbox je sekvenciran; skip slike ne prekida audio queue.

Zatvoreno ograniceno odobrenje 2026-09-13: GPU/DMA Broadcast Player i preview
monitor contract/API. `qnc-player-frame-transport` je GPU/DMA descriptor ugovor
bez aktivnog CPU RGBA preview fallbacka. `qnc-monitor` razumije DMA payload, ali
ga bez platformskog backenda ne slika kao stvarni frame. Stvarni
DXGI/IOSurface/DMA-BUF backend nije implementiran, pa monitor-output mora pasti
jasnom greskom. Freeze ponovno vrijedi.

Datum: 2026-09-13

Zatvoreno ograniceno odobrenje 2026-09-13: prvi preview blit je crtao cijeli
kadar, ali premali. Povecanje ne smije puniti egui callback viewport.
Istom NDC-u se vraca puni framebuffer viewport. Forma i shell ne vežu GPU.
Broadcast Player nije diran. Freeze ponovno vrijedi.

Razlog: korisnik je izricito zatrazio da se zamrzne kompletan `qnc-ingest` i
komponente/moduli koje Ingest koristi. Nema izmjena bez izricite dozvole koja
imenuje Ingest ili imenovani zamrznuti modul i vrstu promjene.
Cijela QNC obitelj je od 2026-09-13 takoder zamrznuta: `docs/86-family-freeze.md`.

## Opseg: kod i ugovori, ne radni podaci

Freeze se odnosi na izvorni kod, manifeste, module contracts i Ingest dijelove
conformancea. Ne odnosi se na radne projektne direktorije, Ingest baze ni
artefakte (katalog, filmstrip JPEG, wave zapisi). Ti se i dalje smiju
zapisivati kroz vec zamrznute javne write putove. Project ostaje zamrznut po
`docs/11-project-freeze.md`.

`qnc-ingest-components` i dalje ne smije postojati. Freeze nije dozvola za
novi umbrella sloj.

## §8.3

Broadcast Player prihvat iz `AGENTS.md` odjeljka 8.3 nije zatvoren. Freeze
zaustavlja daljnji rad na tom prihvatu kroz Ingest i kroz module koje Ingest
koristi. Nastavak zahtijeva novo izricito otkljucavanje.

## Ne smatra se dozvolom

- "nastavi", "idemo dalje", "sredi QNC", "dodaj Story"
- rad na Projectu, shellu ili drugoj aplikaciji ako dira zamrznuti Ingest scope
- "player jos nije gotov" ili otvoreni §8.3 kao razlog za izmjenu

Dozvola mora imenovati Ingest ili tocno ime zamrznutog modula i promjenu, npr.
"otkljucaj Ingest za Import" ili "otkljucaj qnc-broadcast-engine za A/V sync".

## Zamrznuti scope

### Ingest aplikacija i forma

- `apps/qnc-ingest/**`
- `apps/qnc-ingest/qnc-app.json`
- `crates/qnc-ingest-desktop/**`
- `crates/qnc-ingest-desktop-adapter/**`
- `crates/qnc-ingest-application/**`
- `crates/qnc-ingest-store/**`
- `crates/qnc-ingest-select/**`
- `crates/qnc-ingest-catalog/**`
- `crates/qnc-ingest-work-plan/**`
- `contracts/applications/ingest.application.json`
- `contracts/databases/ingest-registry.database.json`
- `contracts/databases/ingest-content.database.json`
- `contracts/databases/source-index.database.json`
- `contracts/databases/media-records.database.json`
- `contracts/ui/ingest.layout.json`
- Ingest dijelove `contracts/qnc-keyboard-shortcuts.json`
- Ingest conformance pravila u `tools/qnc-conformance/**`

### Javni moduli i komponente koje Ingest koristi

- `crates/qnc-dir-browser/**`
- `crates/qnc-keyboard-shortcut/**`
- `crates/qnc-ui-kit/**`
- `crates/qnc-work-settings/**`
- `crates/qnc-monitor/**`
- `crates/qnc-timeline/**`
- `crates/qnc-timeline-assets/**`
- `crates/qnc-player-timeline/**`
- `crates/qnc-filmstrip/**`
- `crates/qnc-filmstrip-worker/**`
- `crates/qnc-wave/**`
- `crates/qnc-wave-worker/**`
- `crates/qnc-wave-view/**`
- `crates/qnc-player-client/**`
- `crates/qnc-player-input/**`
- `crates/qnc-player-launcher/**`
- `crates/qnc-player-contract/**`
- `crates/qnc-player-frame-transport/**`
- `crates/qnc-broadcast-player/**`
- `crates/qnc-broadcast-engine/**`
- `tools/qnc-player-runner/**`
- `crates/qnc-decoder-catalog/**`
- `crates/qnc-media-decode/**`
- `crates/qnc-media-stream/**`
- `crates/qnc-media-probe/**`
- `crates/qnc-media-thumbnail/**`
- `crates/qnc-media-metadata/**`
- `crates/qnc-media-metadata-compose/**`
- `crates/qnc-media-record-db/**`
- `crates/qnc-media-records/**`
- `crates/qnc-ffprobe-metadata/**`
- `crates/qnc-image-assets/**`
- `crates/qnc-source-reader/**`
- `crates/qnc-source-groups/**`
- `crates/qnc-source-index-db/**`
- `crates/qnc-scanner/**`
- `crates/qnc-camera-detector/**`
- `crates/qnc-camera-patterns/**`
- `crates/qnc-sony-metadata/**`
- `crates/qnc-audio-output/**`
- `crates/qnc-video-output/**`
- `crates/qnc-ffmpeg-decode/**`
- `crates/qnc-gpu-raster/**`
- `crates/qnc-pixel-convert/**`
- `crates/qnc-transport-resolver/**`
- `crates/qnc-json-transport/**`
- `crates/qnc-db-contract/**`
- `crates/qnc-dev-diagnostics/**`
- odgovarajuci `contracts/modules/*.module.json` za gore navedene module
- `docs/84-broadcast-player-protocol.md` samo kao ugovor; izmjena protokola
  zahtijeva otkljucavanje player modula

Shell (`apps/qnc-app/**`) nije zamrznut kao cijeli host. Promjena Ingest
`desktop_entry` factoryja, Ingest adapter ovisnosti ili Ingest hostanja u
shellu spada u ovaj freeze.

## Dopusteno bez otkljucavanja

- citanje zamrznutog koda
- audit bez izmjena
- pokretanje testova i live `qnc-ingest.exe` / shell-hosted Ingest
- zapis Ingest rezultata u projektne direktorije i vlastite tablice kroz
  postojeci javni write put
- razvoj Story/Media Assist ili drugog koda koji ne mijenja gore navedene
  putanje

Ako nova aplikacija treba izmjenu zamrznutog javnog modula, rad stati i
traziti otkljucavanje tog modula. Ne granati privatnu kopiju "za Story".

## Postupak otkljucavanja

1. Zaustaviti implementaciju koja dira zamrznuti scope.
2. Navesti tocne datoteke.
3. Traziti izricitu korisnicku dozvolu.
4. Nakon odobrene promjene ponovno vratiti status na zamrznuto u ovom fileu
   i u `AGENTS.md` odjeljku 17.
