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

### 3.5. Kocke, ne monolit (provjera 2026-11-04)

Model se ne pise kao jedan crate s "dispatcherom" koji zna Story, klip i scenarij - to bi bio
novi centralni kontroler (zabranjen, odjeljak 0.11 i 12). Podjela:

| Kocka | Jedna odgovornost | Stanje |
|---|---|---|
| `qnc-edit-verbs` | rjecnik radnji (3.2) <-> sistemske radnje kataloga; bez stanja, bez pravila vlasnika | IZVEDENO (korak 1) |
| `qnc-mouse-map` (postojeca) | gesta na mjestu -> radnja; ista mapa za timeline i zoom | postoji, zoom ga jos ne koristi |
| `qnc-timeline` (postojeca) | crtanje osi i pogodak mjesta (rez, strana, traka) | postoji |
| `qnc-trim-overlay` (postojeca) | crtanje uvecanog prozora; dogadaje gradi kroz `qnc-edit-verbs` | izvedeno za sve dogadaje zooma |
| `qnc-edit-cut` | pravila reza: strane, slika, ton, J/L (spaja `cut_part` i `JoinSound`) | predvideno (korak 4) |
| `qnc-edit-edge` | pravila ruba: take/nudge/drag/mark/keep/drop, roll, ripple (spaja `SlotTrim`, `EdgeTrim`, SubSegment `Trim`, `mark_focus`) | predvideno (korak 5) |
| `qnc-edit-lane` | traka komada: kanal, razina, off (spaja `route_lane`, `segments_keys::draft`, `sound_key`) | predvideno |
| `qnc-edit-words` | rijec na osi: cue, rub na granicu rijeci, par IN/OUT | predvideno (korak 6) |

Vlasnici (`qnc-program-segments`, `qnc-source-timeline`, `qnc-script`) zovu te kocke sa svojom
osi i svojim upisom; sami ne mapiraju geste.

### 3.6. Prekrivanje reza bez pokrivalice (predvideno, kocke se ne pisu odmah)

Izbor na spoju dvaju komada istog govornika (korisnik 2026-11-04: pitanje; "predvidi ih"):

| Nacin | Sto radi | Kocka (predvidena) | Gdje se racuna |
|---|---|---|---|
| Rez | kao danas | - | - |
| Pretapanje | 3-6 slika miksa slike preko reza | `qnc-cut-dissolve` (miks dviju slika, opis prijelaza na spoju) | player uzivo + prerender |
| Punch-in | drugi komad povecan 10-15 %, kao promjena plana | `qnc-cut-punch-in` (izrez i skaliranje slike komada) | player uzivo + prerender |
| Smooth cut (AI, offline) | medjuslike preko reza (RIFE ncnn-Vulkan, MIT; FILM za vece pokrete) | `qnc-cut-smooth` (radnik u pozadini, dodatak po ugovoru kao prijepis) + `qnc-frame-interp` (program dodatka) | pozadinski render u prerender, player pusta gotov spoj |
| Bljesak | bijeli bljesak | (ne za vijesti) | - |

Zapis: nacin prijelaza i duljina uz spoj (kao pauza i rez/crossfade dijela), u bazi vlasnika.
Uredivacka napomena: smooth cut skriva montazu izjave; neke redakcije to ne dopustaju, pa nije zadano.

### 3.7. Alati suvremenih montaznih programa koje QNC nema (Premiere Pro, Resolve, Final Cut, Avid, Edius)

Iz poznavanja tih programa (nije provjereno na internetu). Ono sto QNC vec ima: roll, ripple
ruba, slide komada, J/L, overwrite (pokrivalica), markeri, undo i undo izabranog, montaza po
tekstu i prijepis, analiza kadrova, normalizacija glasnoce (EBU R 128), proxy, XML, HDMI izlaz.

| Alat | Opis | Vaznost za vijesti |
|---|---|---|
| JKL shuttle | J nazad, K stop, L naprijed; ponovljeni pritisak brze (2x, 4x) | visoka |
| Trim mode / dinamicni trim | rub se mice dok svira, JKL u trimu, prikaz dviju slika (imamo zoom bez playa) | visoka |
| Slip | pomak sadrzaja komada unutar istih IN/OUT (bez promjene trajanja) | visoka |
| Lift / Extract | izvadi komad i ostavi rupu / zatvori rupu | visoka |
| Ripple delete i brisanje rupe | | srednja |
| Insert edit / 3-point i 4-point edit | umetanje izvora na playhead s pomicanjem ostatka | srednja |
| Match frame | iz price na isti kadar izvora i obrnuto | visoka |
| Snapping | rub, playhead i marker se lijepe za rezove | visoka |
| Brzina i freeze frame | usporeno, ubrzano, zamrznuta slika | srednja |
| Keyframeovi razine (rubber band) i automatsko stisavanje ambijenta pod govorom (ducking) | | visoka |
| Ciscenje govora (noise reduction, dialog isolation) | offline, npr. RNNoise / DeepFilterNet | visoka |
| Titlovi i potpisi (lower thirds), predlosci grafike | | visoka |
| Titlovi iz prijepisa (SRT/burn-in) | | visoka |
| Zamagljivanje lica i tablica (GDPR) s pracenjem | offline detekcija lica | visoka |
| Smart reframe 16:9 -> 9:16 / 1:1 za drustvene mreze | pracenje lica | srednja |
| Stabilizacija | | niska |
| Osnovna korekcija boje / LUT | | niska |
| Multicam i sinkronizacija po zvuku | | niska za news cutter |
| Biljeske na markerima, boje markera | | srednja |
| Usporedni prikaz izvor / program (dva monitora) | | srednja |

## 4. Plan objedinjavanja (svaki korak zakljucan zlatnom tablicom)

1. **Rjecnik radnji** (IZVEDENO 2026-11-04): kocka `qnc-edit-verbs` (bez dispatchera, vidi 3.5);
   svaki dogadaj zooma (strana, V, A1/A2, desni klik na ton, J/L klik i povlacenje, rijec, Undo,
   Save, Cancel) gradi se kroz nju na panelu vlasnika zooma (bez hardkodiranog `segments`).
   Zlatna tablica (`qnc-timeline` `events.rs`, test
   `the_timeline_and_its_zoom_send_the_same_verbs`): klik (Cue), klik dok je nesto uzeto (Keep;
   `segments_keep_edit` i `source_keep_mark` su Keep), drugi desni klik na sliku uzetog reza
   (TakePicture = V u zoomu), desni klik na A2 uzetog reza (TakeSound A2 = A2 u zoomu), klik i
   povlacenje J/L linije (TakeSoundSide / DragSoundSide) - timeline kroz mapu misa salje tocno
   radnju koju zoom gradi kroz rjecnik, na panelu vlasnika. Ostaje za korak 3: oznaka A1/A2 na
   timelineu uzima ton komada pod playheadom, u zoomu ton uzetog reza; klik na sliku u zoomu
   (PickSide) nema jos istu gestu na Wrap retku (klik lijevo/desno od uzetog reza).
2. **SubSegment na mapu misa** (IZVEDENO 2026-11-04): panel salje radnje mape misa za red programa
   (`segments`), isto sto Wrap red; vlasnik osi (`qnc-script` `pointer.rs` `own_action`) kaze
   sto svaka znaci za dijelove (klik playhead ili Keep, traka bira, desni klik na traku uzima dio,
   desni klik na A1 ton dijela, desni klik kod ruba najblizi rub ili rez, povlacenje uzetog ruba,
   klik i povlacenje J/L linije, kotacic i Ctrl+kotacic). `nearest_edge` i `drag_target` presli
   iz panela u vlasnika. Radnje reda koje SubSegment nema (M marker srednjim klikom, Wrap zoom,
   Edit segmenta) ne idu nikamo (`ROW_ACTIONS`). Jos nema: srednji klik/dvoklik (rez dijela),
   izbor kanala na traci (stupac kanala na timelineu; zasad gumbi u retku dijela) - korak 3.
3. **Trake u zoomu = trake timelinea** (IZVEDENO 2026-11-04): `qnc-timeline` ima javne alate
   trake (`LaneLook`, `paint_lane_button_of`, `paint_lane_heard`, `lane_channel_column`); i
   timeline i zoom ih crtaju. Zoom ima uvijek gumb A1/A2 (crven kad je off, zuti rub dok je
   uzet), razinu (`+N dB`/`OFF`) i, dok je traka uzeta, stupac kanala; klik na oznaku s uzetim
   rezom uzima ton reza, bez reza traku kao oznaka na timelineu (`TakeLane`), klik na kanal
   `PickChannel` - isti glagoli kao timeline (zlatna tablica prosirena). Izbor traka daje vlasnik:
   source timeline (`SourceTimeline::lanes_shown` -> `SourcePick::with_lane_choices` ->
   `ZoomSound.lane_choices`), prica (uzeti ton segmenta ili pokrivalice, `part_sound`), slozena
   izjava (A1 uzetog dijela). Klik na kanal u slozenoj izjavi bira kanal uzetog tona dijela.
4. **Jedan model reza** (J/L slike i tona, strana prije/poslije): `cut_part` i `JoinSound` iza
   `EditOwner`; klik na sliku u zoomu = klik lijevo/desno od reza.
5. **Jedan model ruba**: `SlotTrim`, `EdgeTrim`, SubSegment `Trim`, source `mark_focus` iza istih
   radnji Take/Nudge/DragTo/Mark/Keep/Drop.
6. **Rijeci**: jedna radnja s istim znacenjem u prijepisu, scenariju i zoomu.
