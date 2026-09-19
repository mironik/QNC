# 93 — QNC v5 Story: procesi i pravila (iz koda)

Izvor: `QNC_v5/qnc-host/src/story/{db,markers,covers,object_history}.rs`, `virtual_shots/db.rs`, `editorial_playlist.rs`, `program_playlist.rs`, `qnc-program-playlist/src/lib.rs`, `qnc-app/src/story.rs`, `story/story_edit.rs`, `components/sync_cover_capture.rs`.
Svako pravilo ispod je pročitano u kodu; „(zaključak)” označava izvedeno. Kod se **ne** preuzima, pravila i procedure da.
Povezano: 89 (funkcije), 90 (baza), 91 (lanac), 92 (procesi/postavke).

## 1. Model: objekti i ključevi

| Objekt | Tablica | Ključ | Veze |
|---|---|---|---|
| Source virtual (root) | `virtual_shots` (`kind=import_root`, zaključan) | `root_<clip_id>` | `clip_id` |
| Short / B-roll | `virtual_shots` | `<clip>_shot_NNN` | `clip_id`, `source_shot_id` |
| Segment (Ton / Off) | `story_parts` (`kind` = `tonovi`/`offovi`) | `part_<uuid>` | `clip_id`, `virtual_shot_id` (opcionalno) |
| Marker | `story_markers` | `marker_<uuid>` | `origin_part_id` |
| Slot (M–M) | `story_marker_slots` | `slot_id` = potpis `start:S.SSS\|end:E.EEE` | `start/end_marker_id` |
| Pokrivalica (cover) | `story_covers` | `cover_<uuid>` | `slot_signature`, `clip_id`, `virtual_shot_id` |
| Stanje | `story_state` (1 red) | `id=1` | odabrani part/shot/slot/cover, `draft_updated_at`, `committed_at` |
| Undo | `story_object_history` | (`object_type`,`object_id`) | samo `cover` |

## 2. Vrijeme i jedinice (temelj svih pravila)

1. **Sve je u frameovima**; sekunde su izvedene i zaokružene na 3 decimale (`round3`), tolerancija usporedbe `TIMELINE_EPS = 0.001`.
2. **Program timebase = source fps prvog segmenta s valjanim fps-om.** Bez segmenta s valjanim fps-om nema markera ni slotova (`timeline_fps_invalid`). Prazan Story nema fps.
3. Dva vremenska sustava: **source koordinate** (frame u izvornom klipu; `in_frame/out_frame` segmenta i covera) i **programske koordinate** (`timeline_*`, kumulativno). Preslikavanje: `local_to_timeline_frame`, `part_timeline_window_frames`.
4. Svaki segment nosi svoj `source_fps_num/den`; playlist odbija segment/cover čiji timebase ne odgovara probe zapisu klipa (`editorial timebase … ne odgovara spremljenom probe timebaseu`).
5. Trajanje: `duration_frames`, oznaka `duration_label`, boja `duration_color_key` izračunati iz broja frameova (i ponovno računaju pri promjeni fps-a: `sync_story_part_source_fps`).
6. Programsko trajanje = **zbroj trajanja aktivnih segmenata**; segmenti idu jedan za drugim, bez rupa i preklapanja.

## 3. Pravila po objektu

**Source / Short / B-roll (`virtual_shots`)**
- R1. Kadar se smije stvoriti samo iz klipa koji je uvezen (postoji proxy; v5 `proxy_path_for_clip`), inače `Klip '…' nije uvezen u ingest`. (Kod nas mora poštivati `ingest_media = link`, vidi 92: uvjet = klip ima izvor i probe, ne nužno kopiju.)
- R2. `OUT` mora biti najmanje 1 frame iza `IN` (`OUT mora biti najmanje jedan frame nakon IN`); IN se ograničava na ≥ 0.
- R3. Id kadra `<clip>_shot_NNN`, redni broj po klipu; ime iz imena korijena (`virtual_name_for_derived_shot`).
- R4. Uz svaki kadar zapisuju se dvije slike, `cover.jpg` (IN kadar) i `out_cover.jpg` (OUT kadar), u `virtual_shots/<shot_id>/`.
- R5. Root se održava iz probea: IN=0, OUT=trajanje, zaključan, `source=import`. Ne uređuje se.
- R6. Iz kadra se može izvesti novi (`derive_virtual_shot`): lokalni IN/OUT su relativni na IN izvora, novi kadar pamti `source_shot_id`.
- R7 (v5 zamka). Klasa (short/B-roll) je u v5 samo `category_key`; B-roll nastaje prepisom `cover` na već stvoren short. Kod nas eksplicitna klasa (dokument 89, B2).

**Segment (`story_parts`)**
- R8. Vrste samo `tonovi` (izjava, slika + zvuk) i `offovi` (voice over: **bez slike**, samo zvuk); drugo → `invalid kind`.
- R9. Nastaje iz Source IN/OUT (Talking Head / Voice over). IN/OUT se **kopira** u dio pri stvaranju; kasnije izmjene izvora ga ne mijenjaju. Put segmenta nije `virtual_shots` (komentar: „Segment-only path: story_parts trim, no virtual_shots insert”); `virtual_shot_id` je prazan, osim ako se segment stvori iz postojećeg kadra.
- R10. Segment bez izvora dopušten je kao prazan nacrt (`streamable = false`, fps 0), ali blokira markere/slotove dok nema valjan fps.
- R11. `Mark IN/OUT` na segmentu (Wrap): lokalni frame ograničen na `[0, trajanje]`; novi IN = stari IN + lokalno, OUT ≥ IN+1 (`OUT mora biti poslije IN`); nakon izmjene se markeri i slotovi preračunavaju.
- R12. Brisanje je **meko** (`active = 0`); nakon brisanja: markeri unutar prozora dijela se brišu, svi iza pomiču ulijevo za trajanje dijela (ripple), `sort_index` se renumerira, odabir prelazi na susjeda.
- R13. Redoslijed: pomak `up`/`down` zamjenjuje susjede; na rubu ne radi ništa.
- R14. Svaka promjena ide kroz `finalize_story_mutation`: preračun markera → slotova → covera i `draft_updated_at`.

**Marker**
- R15. Uvijek točno dva sistemska markera: **početni** (frame 0, `program_start`) i **završni** (na trajanju programa, `program_end`). Oba **zaključana** (`Početni/Završni M marker je zaključan`): ne brišu se, ne pomiču, ne uređuju.
- R16. Ostali markeri ručni; nikad automatski na početku segmenata.
- R17. Marker mora biti unutar `[0, trajanje]` (`M marker mora biti unutar trajanja storyja`); dva markera na istom frameu nisu dopuštena (`marker already exists at timeline_frame`); ponovno stvaranje na istom frameu samo ažurira taj marker.
- R18. Naziv zadano = timecode; pri pomaku se naziv koji je bio timecode ažurira, prilagođeni naziv ostaje.
- R19. Marker pamti podrijetlo (`origin_part_id`, `origin_local_frame`).
- R20. Prazan program (0 frameova) briše sve markere i slotove i poništava odabrani slot.

**Slot (M–M)**
- R21. Slot je razmak dvaju susjednih markera (u redoslijedu po frameu); prazan (start ≥ end) se preskače. Slotovi se **brišu i grade ispočetka** pri svakoj promjeni.
- R22. Identitet slota je potpis `start:…|end:…` (v5: `slot_id` == potpis). Zato se pomicanjem markera slot dobiva novi id.
- R23. Odabrani slot koji više ne postoji poništava se.

**Pokrivalica (cover)**
- R24. Pokrivalica pripada slotu (`slot_signature`); **najviše jedna po slotu**: stvaranje briše postojeću u istom potpisu (Overwrite je isto stvaranje).
- R25. Cover pamti puni izvorni raspon (`source_in/out_frame`, fps, timebase); naziv funkcije `trim_cover_source_to_slot` **ne skraćuje** na slot. Skraćivanje se događa tek pri gradnji programa.
- R26. Nakon preračuna slotova cover se **premješta** na slot s istim potpisom/frameovima/vremenima, a **briše se** ako takav slot više ne postoji (pomicanje ili brisanje markera briše pokrivalicu).
- R27. Cover je „streamable” samo ako ima `cover_id`, `virtual_shot_id`, valjan frame raspon i probe timebase; inače nosi `stream_error` (`missing virtual_shot_id`, `missing source frame range`…).
- R28. Undo/redo postoji samo za `cover` (`story_object_history`, snapshot JSON); restore briše drugi cover u istom slotu.

**Stanje i potvrda**
- R29. Odabir (part, shot, slot, cover) je u `story_state` (jedan red). Odabir mora ukazivati na postojeći objekt, inače se briše.
- R30. `commit` samo upisuje `committed_at`; draft i „committed” nisu razdvojeni (v5 nedostatak, vidi 90).

## 4. Iz baze u program (montaža → izlaz)

1. **Montažna lista** (`EditorialPlaylist`): za svaki dio (redom) globalni raspon `[start,end)`, izvor `clip_id`+in/out frame, pa popis covera koji ga sijeku.
2. Cover se preslikava u lokalne koordinate segmenta: `local = timeline − početak segmenta`, odsječeno na segment. Cover koji počinje prije segmenta dobiva `source_offset` (zvuk/slika teče „ispod”).
3. **Flat program** (`qnc-program-playlist`), pravila:
   - **Ton**: slika baze + zvuk baze na **A1**.
   - **Off**: **nema slike baze**, samo zvuk na **A1**; sliku daju isključivo pokrivalice.
   - **Cover**: sloj slike `Cover` + vlastiti zvuk na **A2**; A1 (govor priče) teče dalje ispod pokrivalice.
   - Pokrivalica traje `min(dužina izvora, slot)`; ako je izvor kraći od slota, ostatak slota pokazuje bazu; ako je duži, reže se na kraju slota.
   - Više covera u istom segmentu: sortirani po početku, bez preklapanja (`cursor`).
   - Prazan program → `Program input je prazan.`
   - Segment bez `clip_id` → greška (`Segment '…' nema clip_id`).
4. Preview i export koriste **isti** flat program; razlika je samo `MediaAccessKind` (proxy za preview, `OriginalMaster` za export).
5. **Prolazni overlay** (Sync): `apply_transient_program_overlay` zamjenjuje sliku i A2 u rasponu bez zapisa u bazu; A1 ostaje; raspon izvora mora imati istu duljinu kao raspon programa.

## 5. Procedure u aplikaciji (korisnički tok)

| # | Procedura | Preduvjeti | Učinak |
|---|---|---|---|
| P1 | Odabir klipa (All) | uvezen klip | Source pogled: monitor + source timeline; IN/OUT pri odabiru kadra preuzimaju se iz kadra |
| P2 | Mark IN (Source) | poznat source fps | IN = playhead; ako OUT nije postavljen ili je ≤ IN, OUT = kraj klipa (ne IN+1 s) |
| P3 | Mark OUT (Source) | poznat source fps | OUT = max(playhead, IN+1) |
| P4 | Add virtual clip | klip odabran, OUT > IN | zapis kadra u `virtual_shots` (short) |
| P5 | Talking Head / Voice over | odabran klip + IN/OUT | `create_part` (tonovi/offovi) na kraj programa; odabire novi dio |
| P6 | Wrap pogled: Mark IN/OUT | odabran dio | skraćuje dio (R11) |
| P7 | M marker | Wrap playhead | marker na frame playheada (R17); porijeklo = odabrani dio |
| P8 | Slot odabir / prev-next slot / prazan slot | postoje slotovi | fokus slota; „fokusiraj prazan slot” traži prvi bez pokrivalice |
| P9 | Cover slot (Quick cover) | odabran **prazan** slot + Source IN/OUT, fps i timebase poznati | `create_cover_from_source`; greške: `Odaberi marker slot za pokrivalicu`, `Odabrani marker slot već ima pokrivalicu` (bez automatskog prelaska na drugi slot) |
| P10 | Overwrite | odabran slot ili cover | zamjena pokrivalice u slotu (R24) |
| P11 | Mark IN + trajanje slota (fit) | Source pogled, slot (odabran ili prvi prazan) | IN = playhead, OUT = IN + trajanje slota, ograničeno na kraj klipa |
| P12 | **Sync / B-roll** | uključen Sync | (a) Source IN arma; (b) Space pokreće program od sidra (anchor) uz prolazni overlay; (c) OUT završava: računa novi slot, dodaje marker na kraju ako ne postoji, odabire slot; (d) Enter dodaje pokrivalicu (samo ako slot nema pokrivalicu); optimistična projekcija dok se sprema |
| P13 | Brisanje odabranog (dio / marker / cover) | odabir | pravila R12, R15, R26 |
| P14 | Undo/Redo | odabran cover | R28 |
| P15 | Export HI-res | montažna lista nije prazna | posao `export_hires` na originalima (89 G) |
| P16 | Commit | — | `committed_at` (R30) |

**Fokus i navigacija (tipkovnica):** tri panela (media pool, source, segment panel) s kružnim fokusom; prev/next objekt, segment, marker, slot; „ClearFocus” vraća fokus na playhead; dok je fokus na frame-korekciji IN/OUT, strelice nude ±1 frame. Prečaci se čitaju iz kataloga (`add_ton_segment`, `add_off_segment`, `add_marker`, `quick_overwrite_cover`, `overwrite_cover`, `mark_in_fit_duration`, `playlist_input_start`, …); UI nikad ne ugrađuje tipke.

**Pogledi:** `Source` (klip u docku, IN/OUT u sesiji) i `Wrap` (program priče). Tab Segment prebacuje u Wrap. **Source IN/OUT su radno stanje sesije**, u v5 se ne zapisuju u bazu dok se ne pretvore u kadar, dio ili cover.

## 5a. Dogovoreno: provjera trajanja pri dodavanju pokrivalice (novo pravilo, nije u v5)

Vrijedi za obični Quick cover i za Sync/B-roll.

1. Korisnik označi slot pokrivalice (M–M).
2. Odabere source ili short klip (izvor pokrivalice).
3. Skripta **usporedi trajanja**: trajanje slota (programski frameovi → sekunde) i raspoloživo trajanje izvora od IN-a (source: od IN do kraja klipa; short: od IN do kraja kadra).
4. Ako je izvor **kraći od slota**, source timeline dobiva **crveni indikator na svom kraju**, da korisnik unaprijed vidi da je prekratak. Indikator se prikazuje prije zapisa, ne nakon.

5. Ako korisnik svejedno doda prekratki klip, **na kraju pokrivalice se dodaje marker** (programski frame = početak slota + trajanje izvora pretvoreno u programske frameove). Slot se time dijeli: prvi dio (pokrivalica, točno njezine duljine) i **novi prazni slot** do starog kraja, koji se može popuniti drugom pokrivalicom. Ako je izvor dovoljno dug, marker se ne dodaje.

Redoslijed i posljedice (proizlaze iz pravila R21–R26):
- Marker i pokrivalica moraju nastati **u jednoj transakciji**: pokrivalica se veže na prvi novonastali slot. Inače bi preračun slotova (R26) obrisao pokrivalicu, jer stari slot (potpis početak–kraj) više ne postoji.
- Zato predlažem **stabilan `slot_id`** (odjeljak 6, točka 1); bez toga dijeljenje slota briše pokrivalicu ako se ne radi ispravnim redoslijedom.
- Marker se ne dodaje ako je izračunati frame jednak kraju slota (dovoljno dug izvor) ni ako je manji od jednog programskog framea (prekratko: odbiti).
- **Postoji samo jedna vrsta M markera** (odluka korisnika). Marker na kraju kratke pokrivalice je običan M marker: nema oznake „cover_end”, korisnik ga briše i pomiče kao svaki drugi. Početni (frame 0) i završni (kraj programa) M marker jednako su M markeri; zaključani su **položajem**, ne posebnom vrstom (u novoj shemi nema polja `system_role`).
- Posljedica: marker ostaje i kad se pokrivalica kasnije zamijeni duljim klipom (Overwrite); slot ostaje kratak, a dulji klip se reže na kraju slota. Spajanje slotova je isključivo korisnikovo brisanje markera.
- Brisanje tog markera spaja slotove pa (R26) briše pokrivalicu; potrebno je izričito upozorenje.

Usporedba se radi u sekundama/timebaseu (slot je u programskom fps-u, izvor u svom), ne u sirovim frameovima. Računanje je čista funkcija u zasebnom javnom modulu (ulaz: trajanje slota, IN, kraj izvora, dva timebasea; izlaz: dovoljno / nedostaje N frameova). Timeline komponenta samo crta indikator koji joj se preda (pasivna, bez logike).

Ovim je riješeno pitanje 2 iz odjeljka 7 (kraća pokrivalica): korisnik je upozoren unaprijed; ostatak slota i dalje pokazuje bazu (odjeljak 4, pravilo o `min(izvor, slot)`).

## 6. Zamke v5 koje ne preuzimamo

1. Identitet slota = potpis vremena → pomicanje markera briše pokrivalicu (R22, R26). Predlažem stabilan `slot_id`, a potpis samo kao izvedeni podatak; brisanje pokrivalice zahtijeva izričitu radnju ili upozorenje.
2. Klasa kadra (short/B-roll) određena naknadnom oznakom.
3. Commit bez snimke verzije (R30).
4. Ime `trim_cover_source_to_slot` ne odgovara ponašanju (R25).
5. Kopije fps-a i trajanja u `virtual_shots` i `story_parts` (90 nalaz 5).
6. Mekano brisanje dijelova (`active = 0`) bez čišćenja/arhive.
7. Program timebase ovisi o „prvom valjanom” segmentu; miješani fps klipova u istoj priči nije izričito riješen (zaključak).

## 7. Što ne razumijem dovoljno i treba odluku

1. **Miješani fps** u jednoj priči (npr. 25 i 50): v5 traži timebase po klipu i program po prvom segmentu; hoćemo li ih zabraniti, ili pretvarati?
2. **Pokrivalica kraća od slota**: v5 pokaže bazu za ostatak. Tako i kod nas, ili upozorenje u UI?
3. **Off bez slike**: dok nema pokrivalice, program ima zvuk bez slike (crno). Prikaz crnog ili posljednji kadar?
