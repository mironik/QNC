# 98. Plan alata koji QNC-u nedostaju

Korisnik 2026-11-04: "napravi plan za dodavanje opcija koje nam nedostaju". Polaziste je
`docs/97` (odjeljak 3.7, alati suvremenih montaznih programa; 3.6, prekrivanje reza). Svaki alat
je kocka ili par kocki s jednom odgovornoscu (pravila 0.11 i 0.12): forma i aplikacijski sloj ne
rastu, timeline i zoom imaju iste radnje, svaka nova tipka ima `action_id` u katalogu prije koda,
svaki trajni podatak je u bazi projekta kroz javnog pisca. Red je po vaznosti za news cutter
(laptop + HDMI, brzina montaze).

## 0. Ostaci objedinjavanja (docs/97)

| Stavka | Kocka | Stanje |
|---|---|---|
| `control read header timed out` | `qnc-broadcast-engine` | IZVEDENO 2026-11-04: nakon skoka zvuk klipa otvarao je dekoder za svaki kanal kamere na niti playera (4 ffmpega na kartici, preko 2 s); sada u pozadini kao slika (`Opening`) |
| J/L slike u slozenoj izjavi | `qnc-script-db` (`v_lead/v_tail`, `SetJoinPicture`), `qnc-program-input`, `qnc-script` | IZVEDENO 2026-11-04: V u zoomu uzetog reza spoja uzima sliku spoja; Ctrl+I J, Ctrl+O L, strelice/kotacic, Enter/Spremi, Esc; pravilo `qnc-edit-cut` bez rupa |
| Rub slota u prici na `qnc-edit-edge` | - | NE PRENOSI SE: rub slota je rub izmedju dva M markera na osi price (ripple, prazni slot, zadnji slot, zakljucani markeri), ne komad u klipu; granice klipa ima vec `past_clip_end`. Isti alat (Ctrl+I/O, strelice, zoom, Spremi) vec vrijedi |
| Klik lijevo/desno od uzetog reza | mapa misa | CEKA ODLUKU: danas lijevi klik dok je nesto uzeto sprema (zakljucano 20. 10.) |

## 1. Brzina montaze (prvo)

| # | Alat | Kocke | Sto radi | Ovisi o |
|---|---|---|---|---|
| 1.1 | **JKL shuttle** | `qnc-shuttle` (brzine i smjer, ponovljeni pritisak 1x/2x/4x/8x, K stop, K+J/K+L slika po slika) + naredba playera `SetRate` | J nazad, K stop, L naprijed; brzina na izlazu zvuka (bez zvuka iznad 2x) | Broadcast Player: obrnuti i brzi play (otkljucavanje playera) |
| 1.2 | **Snapping** | `qnc-snap` (cista: najblizi rez, marker, playhead, IN/OUT unutar N piksela) | rub, marker i playhead se lijepe za rezove pri povlacenju i kotacicu | `qnc-timeline` (geometrija), `qnc-edit-edge`, `qnc-edit-cut` |
| 1.3 | **Match frame** | `qnc-match-frame` (iz price: klip i izvorna slika pod playheadom; iz izvora: mjesto u prici) | tipka F: iz Wrapa na isti kadar u source timelineu i natrag | `qnc-program-view`, `qnc-source-preview` |
| 1.4 | **Trim mode (dinamicni)** | `qnc-trim-play` (petlja oko reza: 2 s prije, 2 s poslije; JKL pomice uzeti rub dok svira) | rub se mice dok svira, zoom ostaje otvoren (Spremi ga vec ne zatvara) | 1.1, `qnc-edit-edge`, zoom |
| 1.5 | **Slip** | `qnc-edit-edge` (`slipped`: oba ruba za isto, duljina ista) | sadrzaj dijela/segmenta/pokrivalice se pomice unutar istog IN/OUT-a; zoom pokazuje prvu i zadnju sliku | `qnc-edit-edge`, vlasnici (prica, slozena izjava) |
| 1.6 | **Lift / Extract, ripple delete, brisanje rupe** | `qnc-edit-lift` (pravilo: izvadi raspon s rupom ili je zatvori) + operacije `qnc-program-db` | izvadi oznaceni dio price (IN/OUT na Wrapu) i ostavi crno ili zatvori | `qnc-program-db`, markeri i slotovi |
| 1.7 | **Insert / 3-point edit** | `qnc-edit-insert` (pravilo: izvor IN/OUT na playhead price, ostatak se pomice) | umetanje izvora usred segmenta (dijeli segment) | `qnc-program-db` (dijeljenje segmenta) |

## 2. Ton

| # | Alat | Kocke | Sto radi |
|---|---|---|---|
| 2.1 | **Krivulja razine (keyframeovi)** | `qnc-level-keys` (tocke razine na osi komada) + tablica uz `story_items` + engine/prerender mnozi po tocki | razina A1/A2 se mijenja unutar segmenta/pokrivalice; crta se na traci; mis i Ctrl+klik dodaju tocku |
| 2.2 | **Automatsko stisavanje ambijenta pod govorom (ducking)** | `qnc-ducking` (iz glasnoce A1 po odsjecima: gdje je govor, A2 dolje za N dB s rampom) -> tocke 2.1 | jedna tipka za cijelu pricu, rezultat su obicne tocke razine koje se mogu ispraviti |
| 2.3 | **Ciscenje govora (offline)** | `qnc-voice-clean` (dodatak po ugovoru kao prijepis: RNNoise ili DeepFilterNet) + render u prerender | sum i vjetar s izjave; original ostaje, ocisceni zvuk je artefakt |
| 2.4 | Pretapanje tona na rubu komada | postoji (crossfade, J/L) | - |

## 3. Slika i grafika

| # | Alat | Kocke | Sto radi |
|---|---|---|---|
| 3.1 | **Potpisi (lower thirds) i titlovi iz predloska** | `qnc-graphics-template` (JSON predlosci: polja, raspored, boje; isti kao profil scenarija), `qnc-graphics-render` (slika s prozirnoscu), traka grafike u prici | potpis iz polja izjave (ime, funkcija u scenariju) automatski na segment; uredjuje se kao pokrivalica |
| 3.2 | **Titlovi iz prijepisa (SRT, burn-in)** | `qnc-subtitles` (prijepis segmenata -> retci s vremenom na osi price) + export SRT, render u sliku | prijepis izjava price postaje titl; ispravak u panelu prijepisa |
| 3.3 | **Zamagljivanje lica i tablica (GDPR)** | `qnc-face-detect` (offline dodatak, npr. YuNet/ONNX), `qnc-blur-track` (pracenje kroz komad), render u prerender | odabrano lice/tablica zamagljeno kroz kadar; izmjena okvira u zoomu |
| 3.4 | **Prekrivanje reza** (docs/97 3.6) | `qnc-cut-dissolve`, `qnc-cut-punch-in`, `qnc-cut-smooth` + `qnc-frame-interp` (RIFE) | izbor na spoju: rez, pretapanje, punch-in, smooth cut |
| 3.5 | **Brzina i freeze frame** | `qnc-retime` (omjer brzine komada, freeze kao komad jedne slike), engine i prerender | usporeno, ubrzano, zamrznuta slika |
| 3.6 | **Reframe 9:16 / 1:1** | `qnc-reframe` (izrez prati lice iz 3.3), export profil | verzija za drustvene mreze iz iste price |
| 3.7 | Osnovna boja / LUT, stabilizacija | `qnc-color-basic`, `qnc-stabilize` | nisko za vijesti, kasnije |

## 4. Organizacija

| # | Alat | Kocke | Sto radi |
|---|---|---|---|
| 4.1 | **Biljeske i boje markera** | stupci uz `story_markers` + `qnc-marker-notes` | tekst i boja M markera, popis markera, skok na marker |
| 4.2 | **Usporedni prikaz izvor/program** | panel drugog monitora (ista kocka `qnc-monitor-panel`, drugi player sesija) | lijevo izvor, desno prica |
| 4.3 | Multicam, sinkronizacija po zvuku | `qnc-sync-by-sound` | nisko za news cutter |

## 5. Redoslijed rada (prijedlog)

1. Ostaci docs/97 (0) i popravak `control read header timed out`.
2. 1.2 snapping, 1.3 match frame, 1.5 slip (ciste kocke, bez playera).
3. 1.1 JKL shuttle (otkljucavanje playera) pa 1.4 trim mode.
4. 2.1 krivulja razine pa 2.2 ducking.
5. 3.1 potpisi iz scenarija, 3.2 titlovi iz prijepisa.
6. 1.6 lift/extract, 1.7 insert.
7. 3.3 zamagljivanje lica, 3.4 prekrivanje reza, 2.3 ciscenje govora (AI dodaci, offline).
8. 3.5 brzina, 3.6 reframe, 4.x.

Svaki korak: zlatna tablica gesta i tipki (ista za timeline i zoom), testovi kocke, conformance,
zapis u AGENTS.md; nove tipke prvo u katalog i pomoc F1.
