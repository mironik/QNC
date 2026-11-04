# 97. Objedinjeni model timelinea i zooma

Korisnik 2026-11-04, pravilo 0.12 (ZAKLJUCANO): timeline i zoom imaju iste alate; zoom je
uvecani isjecak timelinea oko playheada; razlika je samo graficki prikaz. Ovaj dokument je
audit stanja (grana `libav-decoder`, fa1eb18c) i model po kojem se kod objedinjuje.

## 1. Prikazi koji danas postoje

| Prikaz | Vlasnik (os i pravila) | Put geste | Mapa misa |
|---|---|---|---|
| Source timeline | klip (`qnc-source-timeline`, `qnc-source-panel`, `qnc-source-mark-focus`) | `qnc-timeline` -> `events.rs` -> `qnc-mouse-map` | da |
| Wrap redovi, program, uvecani slot | prica (`qnc-program-segments`, `qnc-segments-keys`) | `qnc-segment-panel` -> `qnc-timeline` -> `SegmentCommand::from_event` | da |
| Slozena izjava (SubSegment) | scenarij (`qnc-script` trim/slide/join/sound) | `qnc-subsegment-panel` rucno mapira intente | NE |
| Zoom reza | isti vlasnici puni `ZoomSound`/`EditStill` | `qnc-trim-overlay` vlastiti klikovi | NE |
| Rijeci | prijepis, scenarij, zoom | tri razlicita puta | NE |

## 2. Razlike koje krse pravilo 0.12

1. Strana reza: zoom `zoom_pick_side` (klik na sliku), S-Story `clip_half` (lijeva/desna polovica
   klipa), SubSegment `subsegment_select` - tri znacenja iste geste (klik lijevo/desno od reza).
2. Trake A1/A2: timeline ima oznaku, izbor kanala, razinu i OFF; zoom samo gumb koji uzima ton
   reza, i samo dok je rez uzet (source i S-Story zoom nemaju trake).
3. Zoom salje radnje s panelom `segments` i kad je vlasnik source ili SubSegment.
4. V gumb (`select_mark_in`) u Storyju znaci "samo slika reza", u SubSegmentu "IN dijela".
5. J/L: tri hvatanja strane (`qnc-timeline` `cut_edge_hit`/`cut_side_at`, zoom `paint_cut_sound`,
   SubSegment `join_key` koji timeline nikad ne salje) i dva modela (`cut_part` `SoundEdge`,
   `qnc-script` `JoinSound`).
6. Rub: pet izvedbi (`SlotTrim`, S-Story `SlotTrim`+ripple, `EdgeTrim`, SubSegment `Trim`+roll,
   source `mark_focus`); povlacenje ruba postoji samo za source i SubSegment.
7. Desni klik na marker: Wrap `select_marker`, SubSegment `subsegment_take_edge`, S-Story rub klipa.
8. SubSegment zaobilazi mapu misa; smjer kotacica racuna se na pet mjesta.
9. Srednji klik i dvoklik: source rez, Wrap marker/zoom, SubSegment i zoom nista.
10. Rijeci: prijepis (klik cue, dvoklik rez), scenarij (svoj par), zoom (svaki klik rez), Story
    zoom (koraci strelica), SubSegment (`zoom_word`), source (`drag`/`cut_word`).
11. Dva "zooma": uvecani slot (pun Wrap alat) i zoom reza (smanjen alat).
12. Tipke traka: `route_lane`, `segments_keys::draft`, `qnc-script` `sound_key` (vlastite granice).

## 3. Model

### 3.1. Objekti na osi (isti u svakom prikazu)

- **Os**: slike od-do koje prikaz pokazuje (timeline: cijeli; zoom: prozor oko playheada ili reza).
- **Playhead**.
- **Rez**: mjesto gdje se dva komada spajaju (M marker, spoj dva klipa, rub dijela, IN/OUT klipa).
  Ima tri dijela: **slika**, **A1 ton**, **A2 ton**; svaki ton ima J (ulazi prije reza) i
  L (izlazi poslije reza). Rez ima **dvije strane**: prije i poslije.
- **Komad**: prostor izmedju dva reza (slot, dio slozene izjave, segment, klip izmedju IN i OUT),
  s trakom duljine.
- **Traka**: A1 ili A2 komada: kanal, razina, off.

### 3.2. Radnje (jedan skup, isti za timeline i zoom)

| Radnja | Znacenje |
|---|---|
| Cue(slika) | playhead |
| Take(objekt, dio, strana) | uzmi rez cijeli, sliku, ton A1/A2, stranu J ili L, rub komada, komad, traku |
| Nudge(n) | pomak uzetog za n slika (strelice, kotacic) |
| Jump(gore/dolje) | uzeto na kljucnu sliku |
| DragTo(slika) | uzeto na sliku (povlacenje, klik na rijec) |
| Mark(IN/OUT) | I/O: rub komada pod playheadom na playhead |
| Cut(slika) | srednji klik, dvoklik: rez (source: par IN/OUT; prica: M marker) |
| Lane(kanal/razina/off) | traka uzetog komada |
| Keep / Drop / Delete / Undo | Enter / Esc / Delete / Backspace |

### 3.3. Geste -> radnje (jedna mapa, `qnc-mouse-map`, za timeline i zoom)

| Gesta | Radnja |
|---|---|
| lijevi klik | Cue; dok je nesto uzeto Keep |
| lijevi klik lijevo / desno od reza (u zoomu: lijeva / desna slika) | Take(strana prije / poslije) tog reza |
| desni klik (Ctrl+) na rez | Take(rez cijeli); drugi desni klik: slika ili ton po mjestu klika |
| desni klik na traku komada | Take(komad) |
| klik na oznaku A1/A2 (timeline i zoom) | Take(traka), izbor kanala, razina, off |
| povlacenje uzetog | DragTo |
| srednji klik / dvoklik | Cut |
| klik na rijec | Cue na rijec; dok je rub ili strana uzeta DragTo na blizu granicu rijeci |
| dvoklik na dvije rijeci | Mark IN i OUT komada |
| kotacic / Ctrl+kotacic | Nudge / razina uzete trake |

### 3.4. Vlasnik

Vlasnik daje samo **os** (slike, komadi, rezovi kakvi jesu) i **pravila objekta** (roll, ripple,
slide, granice klipa, sto smije J/L) te **upis** u bazu. Vlasnici: klip (source), prica (Story,
S-Story), scenarij (SubSegment). Vlasnik ne mapira geste i nema vlastite radnje.

## 4. Plan objedinjavanja (svaki korak zakljucan zlatnom tablicom)

1. **Ugovor radnji i jedan dispatcher**: nova cista kocka `qnc-edit-model` (objekti 3.1, radnje
   3.2, svojstvo `EditOwner`); zoom salje radnje panela svog vlasnika (bez hardkodiranog
   `segments`); zlatna tablica gesta -> radnja za timeline i zoom (mora biti ista).
2. **SubSegment na mapu misa**: prestaje rucno mapiranje; dobiva srednji klik, dvoklik, J/L klik i
   povlacenje, izbor kanala na traci.
3. **Trake u zoomu = trake timelinea**: iste oznake A1/A2, izbor kanala, razina i off, crta ih
   ista kocka traka iz `qnc-timeline`.
4. **Jedan model reza** (J/L slike i tona, strana prije/poslije): `cut_part` i `JoinSound` iza
   `EditOwner`; klik na sliku u zoomu = klik lijevo/desno od reza.
5. **Jedan model ruba**: `SlotTrim`, `EdgeTrim`, SubSegment `Trim`, source `mark_focus` iza istih
   radnji Take/Nudge/DragTo/Mark/Keep/Drop.
6. **Rijeci**: jedna radnja s istim znacenjem u prijepisu, scenariju i zoomu.
