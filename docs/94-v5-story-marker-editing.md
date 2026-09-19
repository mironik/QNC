# 94 — QNC v5 Story: sve procedure editiranja M markera

Dopuna dokumentu 93 (odjeljak 3, marker/slot). Izvor: `qnc-host/src/story/{markers,db,api}.rs`, `qnc-app/src/{story.rs,qnc_timeline.rs,qnc_segment_timeline.rs}`, `editorial/{segment_program,marker_cover_panel}.rs`, `components/editorial_edit.rs`, `seed/keyboard-shortcuts.json`. „(zaključak)” = izvedeno iz koda, nije izričito testirano.

## 1. Operacije nad markerom (host, `/api/story/marker/*`)

| Operacija | Ruta | Ulaz | Pravila i greške |
|---|---|---|---|
| Stvori | `create` | `timeline_frame` (ili `part_id`+`local_frame`, ili sekunde), `label?`, `part_id?` | frame ≥ 0 (`timeline_frame must be >= 0`); mora postojati valjan program fps (`timeline_fps_invalid`); ako na tom frameu već postoji marker, **ažurira se taj** (naziv, podrijetlo) umjesto duplikata; naziv zadano = timecode; nakon toga preračun slotova |
| Ažuriraj/pomakni na frame | `update` | `marker_id`, `timeline_frame` (ili sekunde), `label?` | frame u `[0, trajanje]` (`M marker mora biti unutar trajanja storyja`); početni/završni zaključani; nema dva markera na istom frameu (`marker already exists at timeline_frame=`); naziv se čuva ako nije poslan |
| Pomak za susjeda | `move` | `marker_id`, `direction` = `up`/`down` | **zamjena frameova** s prethodnim/sljedećim markerom (ne pomak za frame); zaključani markeri i granice ne sudjeluju; oznaka koja je bila timecode se prepisuje novim timecodeom |
| Obriši | `delete` | `marker_id` | početni (frame 0) i završni (`= trajanje` ili `program_end`) zaključani; nepostojeći → `marker not found`; nakon toga preračun slotova |
| Odaberi slot | `marker_slot/select` | `slot_id` | slot mora postojati (`slot not found`); upisuje `story_state.selected_slot_id` |

Svaka izmjena ide kroz `finalize_story_mutation`: preračun početnog/završnog markera → `sort_index` → slotovi → pokrivalice → `draft_updated_at` (dokument 93, R14, R21–R26). Sve rute serijalizira `serialize_project_write` po projektu.

**Što aplikacija zapravo koristi:** samo `create`, `delete` i `marker_slot/select` (`EditorialEditComponent`). Rute `update` i `move` postoje na hostu, ali **u živoj aplikaciji nema UI-ja za pomicanje markera** (fokus ima samo Playhead/IN/OUT; marker-nudge postoji samo u isključenom `qnc-client`, izvan workspacea).

## 2. Ulazne točke u aplikaciji

| Radnja | Kako | Učinak |
|---|---|---|
| Dodaj marker | tipka `M` / `Shift+M` (`add_marker`, `add_marker_continue`, obje isti učinak), gumb **M marker** u panelu | `create` na **Wrap playhead** frameu; `part_id` = **odabrani** dio (ne dio pod playheadom) |
| Odaberi marker | klik na pin (tolerancija ±5 px), Prev/Next marker (panel), strelice po objektima (kad je fokus na Segment panelu) | `selected_marker_id`, playhead skače na marker; **početni marker (frame 0) se ne može odabrati** („Početni M marker je zaključan.”) |
| Obriši | `Delete` / `Backspace` (`delete_marker`), `Ctrl+Delete` | `DeleteSelection`: prvo odabrani marker; ako ga nema, odabrana pokrivalica; inače odabrani dio |
| Odaberi slot | klik na traku slota, Prev/Next slot, „Selektiraj M–M slot pod playheadom”, „Fokus prvi prazni slot” | `select_marker_slot`; odabir slota briše odabrani marker |
| Navigacija | Prev/Next marker, Prev/Next slot, Prev/Next segment, Prev/Next objekt | vidi odjeljak 3 |
| Vlastiti odabir M (`select_marker`, Ctrl+M) | postoji u katalogu prečaca | **nema pripadne radnje u aplikaciji** (nije u `PlaybackAction`), mrtav prečac |

Redoslijed prioriteta klika na timelineu: **marker → pokrivalica → slot → virtualni raspon → pomicanje playheada**. Povlačenje (drag) samo pomiče playhead; markeri se ne povlače.

## 3. Odabir i navigacija (pravila)

1. Odabir je međusobno isključiv: odabir slota briše odabrani marker; odabir pokrivalice briše odabrani marker; odabir dijela briše marker, slot i pokrivalicu.
2. **Prev/Next marker:** pretraga po frameu playheada (`< frame` / `> frame`); rezultat mora imati frame > 0 (početni se preskače); na rubu poruka `Nema prethodnog/sljedećeg M markera`.
3. **Prev/Next slot:** polazi od odabranog slota, inače slota pod playheadom, inače prvog praznog; ne kruži (na rubu `Nema prethodnog/sljedećeg M-M slota`); playhead skače na početak slota, u Wrap pogledu slijedi tihi scrub.
4. **Prev/Next objekt** (Segment panel): tip cilja ovisi o tome što je odabrano: marker → susjedni marker; slot → susjedni slot; inače susjedni segment.
5. **Slot pod playheadom:** `[start, end)`, a za **zadnji** slot vrijedi i `frame == end`.
6. **Prvi prazni slot** = prvi po redu bez pokrivalice. Odabrani slot ima prednost; ako ga nema, „efektivni” slot je prvi prazni.

## 4. Procedure koje mijenjaju markere neizravno (važno)

| Događaj | Učinak na markere (v5) |
|---|---|
| Dodan novi dio na kraj | završni M marker se **pomiče** na novi kraj programa; granica stare i nove priče nema markera (slot preko obje) |
| Obrisan dio | markeri unutar prozora dijela se brišu; svi iza pomiču ulijevo za trajanje dijela (ripple); slotovi i pokrivalice se preračunavaju |
| Skraćen/produljen dio (Mark IN/OUT u Wrap) | završni marker se pomiče; **ostali markeri se ne pomiču ni ne brišu** (zaključak: mogu ostati izvan trajanja ili odvojeni od sadržaja) |
| Promjena redoslijeda dijelova | **markeri ostaju na istim frameovima**; sadržaj se seli, `origin_part_id` može zastarjeti (zaključak) |
| Promjena fps-a izvora (`sync_story_part_source_fps`) | markeri se ponovno računaju u frameove iz sekundi (`backfill_marker_frames`) samo ako im je frame 0 |
| Prazan program | brišu se svi markeri i slotovi, odabir slota se poništava |
| Marker se doda unutar slota s pokrivalicom | stari slot nestaje; v5 briše pokrivalicu (R26) osim ako se koristi postupak iz 93/5a (marker + pokrivalica u jednoj transakciji) |

## 5. Zaključavanje i granice (sažetak)

- Početni M: frame 0, uvijek postoji, ne briše se/ne pomiče/ne uređuje/ne odabire.
- Završni M: na `trajanje`; ako ručni marker stane točno na kraj programa, **on postaje završni** (v5 čuva onaj na tom frameu i briše prijašnji sistemski).
- Ograničenja između: nema dva markera na istom frameu; slot mora imati ≥ 1 frame (prazni se preskaču).
- `create` **ne provjerava gornju granicu** (`> trajanje`), samo `update` (zaključak: aplikacija to sprječava jer se playhead ograničava na program).

## 6. Nedostaci i zamke (ne preuzimamo bez odluke)

1. Nema uređivanja markera u UI-ju (pomak, preimenovanje) iako host to podržava.
2. `move` zamjenjuje frameove susjeda, što korisniku vjerojatno nije očekivano.
3. `origin_part_id` je odabrani dio, ne dio koji sadrži frame; vrijednost nakon reorder/trim ne odgovara sadržaju.
4. Markeri ne slijede sadržaj pri trim/reorder; slijede ga samo pri brisanju dijela.
5. `create` bez gornje granice; `update` s gornjom granicom.
6. Mrtvi prečac `select_marker`.
7. Identitet slota = potpis vremena (dokument 93, zamka 1).

## 7. Što naš marker modul mora nuditi (prijedlog, čiste funkcije bez UI-ja i baze)

- `validate_create / validate_update / validate_delete` (svi uvjeti iz odjeljka 1).
- `normalize_markers` (početni i završni po položaju, jedna vrsta M).
- `derive_slots` (M–M slotovi iz markera, prazni preskočeni).
- Upiti: marker prije/poslije playheada, slot pod playheadom, prvi prazni slot, susjedni objekt.
- `split_slot_at(frame)` za postupak „prekratka pokrivalica” (93/5a).
- Zapis u bazu ostaje kod vlasnika Storyja; modul samo računa.

## 7a. Dogovoreno: uređivanje postojećeg markera (odluka korisnika)

Postojeći **odabrani** marker mijenja se na tri načina, svi završavaju istim zapisom (`update` na frame, jedna transakcija):
1. **±frame** kontrolnom naredbom (poravnato s v5 obrascem za IN/OUT: `Ctrl+I`/`Ctrl+O` daju fokus na IN/OUT, pa strelice ±1 frame; ovdje `Ctrl+M` = fokus na M marker (prečac `select_marker` u katalogu, koji v5 nije spojio), zatim ←/→ ±1 frame (zaključak o tipkama, potvrditi).
2. **Povlačenje** pina na timelineu (pretpostavka: pretpregled tijekom povlačenja, jedan zapis pri otpuštanju).
3. **Novi položaj playheada + `M`**: kad je marker odabran, `M` **premješta odabrani marker** na playhead umjesto da stvara novi; bez odabira `M` stvara marker (kao u v5).

Pravila (ista kao `update`): frame u `[0, trajanje]`; početni i završni zaključani; nema dva markera na istom frameu; naziv se čuva.

Posljedice koje treba riješiti u razvoju:
- Pomak markera mijenja dva susjedna slota. V5 bi obrisao njihove pokrivalice (potpis slota se mijenja, R26). Uz stabilan `slot_id` pokrivalica ostaje na slotu koji se samo produlji ili skrati; tada vrijedi provjera trajanja iz 93/5a (crveni indikator ako izvor postane prekratak) i pravilo `min(izvor, slot)` iz programa.
- Prelazak preko susjednog markera mijenja redoslijed slotova: **dogovoreno** da pomak (±frame i povlačenje) ostaje ograničen između susjednih markera; za promjenu redoslijeda korisnik briše i ponovno stvara marker.
- **Dogovoreno:** stabilan `slot_id`; pokrivalica ostaje vezana uz slot koji se produlji ili skrati, a provjera trajanja (93/5a) se ponovno izvodi.

## 8. Za odluku prije razvoja

1. ~~Pomak markera~~ — riješeno u 7a (±frame, povlačenje, novi playhead + `M`).
2. **Trim/reorder dijela:** neka markeri prate sadržaj (kao pri brisanju) ili ostaju na frameovima (kao u v5)?
3. **Podrijetlo markera (`origin_part_id`):** zapisivati dio koji sadrži frame (točno) umjesto odabranog?
