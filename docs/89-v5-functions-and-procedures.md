# 89 — QNC v5: popis funkcija i procedura za Media Assist i Story

Izvor: `C:\Users\miron\Projects\QNC_v5` (monolit; koristi se kao izvor **funkcija i prikaza**, ne koda).
Cilj: svaka funkcija dobiva javni, univerzalni modul (samostalan i u shellu), po pravilima `AGENTS.md`.
Status: **popis, nije implementacija.** Ništa od ovoga još nije zapisano u ugovore.

Napomena o v5: Media Assist i Story su u v5 **isti ekran** (`StoryScreen`) s dvije uloge
(`EditorialRole::MediaAssist` / `Story`). Razlikuju se po glavi bazena (`show_segment_tab`,
`show_cover_tab`, `show_export_hires`, `show_quick_cover`) i po desnom panelu. Podaci su u
`/api/story/*` (jedna baza priče). Kod nas su to 4 aplikacije (e, g, l, o) na istom layoutu.

## A. Status klipa (točkice na kartici)

Dvije točkice: **proxy** (lijeva) i **original** (desna). Izvor: `import_status` klipa + gdje leži original.

| import_status | proxy | original |
|---|---|---|
| `error` | error (crvena) | error (crvena) |
| `queued`, `processing` | pending (žuta) | pending (žuta) ako je arhiviranje originala uključeno, inače idle |
| `original_ready`, `generating_proxy` | pending (žuta) | ready |
| `imported`, `done` | ready | ready ako je original u projektu, inače idle |
| ostalo | idle | idle |

- Ako postoji materijalizirani play-path (proxy datoteka postoji), proxy je `ready` bez obzira na status.
- Ako klip nije uvezen, a odabran je ili je u tijeku, proxy je `pending`.
- Boje: proxy `ready` zelena; original `ready` plava (0x0a84ff); `pending` žuta (0xffd60a); ostalo crvena (0xff453a); `idle` se crta kao crvena (v5 to ne razlikuje od greške — treba odluka).
- Točkice se crtaju samo kad je uvoz započeo (`queued` i dalje). Režim: `Pipeline` (dvije), `ImportedOnly` (jedna), `Off`.
- Kod nas: čisti javni modul `qnc-clip-status` (ulaz: import_status, putanja originala, arhiviranje, ima li proxy; izlaz: dva stanja). Čita iz javnih pogleda `public_clips`, `public_clip_proxy`.

## B. Kartica klipa i bazen (media pool)

- Prikaz: sličica 16:9 (filmstrip frame), kvačica odabira 16 px (pad 6 px, dolje lijevo), red 34 px: naziv, dvije točkice, trajanje.
- Trajanje: samo iz probe (`ingest_assets`); oznaka `mm:ss`, boja trajanja po kategoriji (`duration_color_key`).
- Akcije bazena (`MediaPoolAction`): odaberi kadar, (od)označi kadar, promijeni tab, odaberi/obriši/pomakni dio (part), play/pauza, MarkIn, MarkOut, QuickCover, Export HI-res.
- Tabovi: `All`, `Virtual`, `B-roll` (cover), `Segment` (samo Story). Uloga ograničava tabove.
- Navigacija tipkovnicom po mreži (fokus, stupci mreže, aktiviranje fokusirane stavke).
- Kategorije virtualnih kadrova: `import_root` (korijen klipa), `short`, `cover`.

## B2. Vrste klipova (dogovoreno s korisnikom)

Svaki klip ima virtualni status. Četiri vrste, **strogo razdvojene**:

| Vrsta | Nastaje | Zapis | Tab |
|---|---|---|---|
| Source virtual | iz uvezenog klipa; IN/OUT = početak/kraj klipa; iz njega nastaju sve ostale | `virtual_shots`, klasa `source` | All |
| Virtual Short | iz source klipa (Add virtual clip), ima IN/OUT i `source_shot_id` | `virtual_shots`, klasa `short` | Virtual |
| B-roll (pokrivalica) | iz source klipa, namijenjen prekrivanju slotova između markera | `virtual_shots`, klasa `b_roll` | B-roll |
| Virtual Segment (Tonovi, OFFovi) | baza Story priče, IN/OUT kopiran iz izvora pri kreiranju | `story_parts` (vlasnik Story) | Segment |

v5 nedostatak koji ne preuzimamo: short se definira **odsutnošću** oznake (sve što nije `cover` je short), a B-roll nastaje tako da se kadar stvori kao short pa mu se naknadno prepiše `category_key = 'cover'`. Kod nas: eksplicitan stupac klase (`source | short | b_roll`) s ograničenjem, zasebne operacije zapisa (`add_short`, `add_b_roll`), zasebni javni pogledi (`public_source_clips`, `public_short_clips`, `public_b_roll_clips`), a boja/ponašanje po klasi.

## C. Izvorni monitor i timeline (source dock)

- Otvori klip: razriješi medij (`play-media`, `media/resolve`), pripremi ulaz playera, prikaži filmstrip i valni oblik.
- Transport: play/pauza, korak ±1 frame, cue/scrub (već u `qnc-source-preview`).
- Oznake: IN, OUT, trajanje (`mark_in`, `mark_out`, `MarkInFitDuration`, odabir IN/OUT).
- Gumbi docka: **Add virtual clip** (piše `virtual_shots`), **Talking Head** (ton segment, Shift+T), **Voice over** (off segment, Shift+V), **Pokrivalice** (cover).

## D. Story — dijelovi (parts) i programski timeline

Procedure (`/api/story/...`): `part.create`, `part.update`, `part.delete`, `part.reorder`, `part.select`, `part.mark_in`, `part.mark_out`, `shot.select`, `commit`.
- Stvaranje dijela iz raspona izvora (`source_range_for_segment`), tipovi: ton (Talking Head) i off (Voice over).
- Boja trajanja dijela i FPS izvora se sinkroniziraju (`sync_story_part_source_fps`).
- Program: playlista i snapshot (`/playlist`, `/program-snapshot`, `/timeline-model`).
- Skok na početak playliste, prethodni/sljedeći dio, brisanje odabrane stavke timelinea.
- Objektni undo/redo (`object/undo`, `object/redo`).

## E. Markeri, pokrivalice (cover), slotovi

- Markeri: `marker.create`, `delete`, `move`, `update`; `marker_slot.select`.
- Pokrivalice: `cover.create`, `update`, `delete`, `select`; privremene projekcije dok se ne zapiše u bazu.
- Panel radnji (`MarkerCoverAction`): početak playliste, prethodni/sljedeći segment, prethodni/sljedeći slot i marker, dodaj marker, kreiraj cover, prepiši cover (Overwrite), Sync/B-roll.
- **Sync cover capture** (procedura): start na Space, zapis IN, zapis OUT, automatski marker, potvrda Enterom, odgođena automatska potvrda (`sync_cover_capture.rs`).

## F. Desni panel „Segmenti” (samo Story, o)

- Zaglavlje: Segment, Playhead, frame, Trajanje.
- Gornji timeline: programski stog A1/V/A2 sa segmentima (tonovi, offovi), markerima i cover slotovima.
- Donji timeline: ulaz playliste (izvorni segment).
- Traka: `M marker`, `Cover slot`, `Overwrite`, transport (⏮ − ⚑ ⌂ ⚑ − ⏭), `Sync/B-roll`.
- Proširenje audio kanala po stogu (`ExpandedAudio`).

## G. Export

- `export/submit` (proxy/prikaz), `export/hires/submit`, `export/status` (poll), `export/cancel`.
- HI-res render: `hires_export_procedure`, `hires_render_procedures`, `hires_render_transport` (worker/`jobs`).
- Export preset već postoji kod nas: `qnc-export-preset`.

## H. AI / ASR (grupa e — Media Assist Audio AI)

Iz v5 (`qnc-host/src/asr`): `asr/health`, `translation/health`, `ai-search/transcribe-stream`, `ai-search/translate-transcript`.
- Kod nas mora biti **lokalno, bez plaćene usluge** (laptop faza). Modul: lokalni ASR iza neutralnog sučelja; mrežni ASR samo kao kasnija opcija.
- Rezultat (transkript) ide u bazu vlasnika (e), a g/l ga čitaju kroz javni pogled.

## I. Infrastruktura koju v5 ima, a mi već imamo ili ne trebamo

- Zadaci/poslovi (`jobs`: claim, heartbeat, complete, fail) — postoji naš ekvivalent (worker lanac); provjeriti prije novog.
- Proxy, filmstrip, valni oblik, poster — dolaze iz Ingesta (samo čitanje javnih pogleda).
- Prečaci (`shortcuts`): katalog akcija (`add_ton_segment`, `add_off_segment`, `add_marker`, `playlist_input_start`…) — kod nas `contracts/qnc-keyboard-shortcuts.json`.
- Sesija reprodukcije i most prema playeru — kod nas `qnc-source-preview` + `qnc-player-client`.

## Predloženi redoslijed (svaki korak = zaseban javni modul + ugovor, zasebno odobrenje)

1. `qnc-clip-status` + prikaz točkica i sličica na kartici (A, B).
2. Tabovi Virtual/B-roll: pogled nad virtualnim kadrovima (B, C: Add virtual clip).
3. Dock akcije i IN/OUT (C).
4. Story: dijelovi, markeri, cover (D, E), zatim desni panel (F).
5. Export (G).
6. Lokalni ASR za grupu e (H).

## Otvorena pitanja za vlasnika

1. `idle` točkica: v5 je crta crvenom kao grešku. Zadržati ili prikazati sivo?
2. Tko je vlasnik baze priče (`story_parts`, markeri, cover, virtualni kadrovi): jedna baza za o, ili posebni pogledi po grupama e/g/l?
3. Redoslijed iz odjeljka „Predloženi redoslijed” — odgovara li?
