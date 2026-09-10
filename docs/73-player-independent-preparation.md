# Broadcast Player: neovisna priprema i Ingest live rezultat

Datum: 2026-09-09. Opseg: odobreni prvi korak iz docs/72, odjeljak 4.
Status: implementirano i ciljano testirano; live prihvat NIJE zatvoren.
Mironik 2002 prosao je cijeli klip, Mironik 2679 u prvom prolazu nije.

## 1. Izmjena u postojecim javnim modulima

- `qnc-broadcast-player/src/transport_engine.rs`: zasebni audio/video kursori
  pripreme, isti source identitet, racionalna vremenska osnova i playback sat.
  Svaki red ima ograniceni horizont i ograniceni broj operacija po ticku.
- Spremni PCM dopunjava audio izlaz prije provjere buduceg video framea.
  `NotReady` jednog reda ne zaustavlja pripremu drugoga. Za dospjeli frame
  oba moraju biti spremna; stvarni propust zaustavlja Play uz gresku.
- Ready trazi pocetni A/V raspon. Pause cuva resurse i ogranicenu memoriju.
  Seek/promjena izvora ponistavaju oba kursora i odgovarajuce payloadove.
  Povratak izvan zadrzanog raspona stvarno cuea oba adaptera prije Ready;
  ne preimenuje vec dekodirani frame.
- `qnc-player-runtime/src/lib.rs`: pending native video izlaz vise ne
  preskace cijeli engine tick. Zauzet izlaz i dalje odbija submit na roku;
  to nije dozvola za preskakanje slika. HTTP monitor nema taj native gate,
  pa ova izmjena nije dokaz uzroka HTTP zastoja.
- Ugovorna dopuna je komentar neblokirajuceg audio adaptera u
  `engine_contract.rs`; nema novog wire formata ili projektnog polja.

Pocetni horizont H ostaje postojeci ograniceni preroll. Pri reprodukciji
zadrzava se najvise H + 2 video payloadova/audio paketa po redu (buduci
raspon, tekuci i prethodni frame). Nema ucitavanja cijelog klipa u RAM,
novog threada, poola, procesa ili drugog enginea u ovom zahvatu.

Dodano je sest regresija u `transport_engine/tests/readiness.rs`: odgodjena
slika uz neovisni PCM, ogranicenje oba reda kroz 1000 paused tickova,
odgodjeni audio bez izmisljanja PCM-a, reset oba reda pri seeku/promjeni
izvora, zauzet native izlaz i eksplicitni cue izvan zadrzanog raspona.
Prilagodjen je stari test koji je ocekivao da audio NotReady blokira sliku.

## 2. Granice i stvarni ulaz

Prije zahvata provjeren je put iz docs/72: javni registar aktivnog projekta,
read-only `public_project_settings`, javni player input i primjena u modulu.
Aktivni projekt `bbvvcx` u bazi ima dva izlazna audio kanala, 48000 Hz i
`playback.input = proxy_if_available`. Ti podaci nisu mijenjani.

Live koristi Sony karticu G: read-only, spremljeni katalog od 98 klipova,
proxy sliku i audio originala. Izvor ima cetiri odvojena mono kanala;
preview izlaz primjenjuje dva kanala iz projektne baze. Ovo nije dokaz
montaznog A1 OFF/izjava i A2 ambijent miksa.

Nema promjene Project koda/ugovora, Ingest forme/workflowa, keyboard kataloga,
Selecta, probea, baza ili kartice u ovom zahvatu. Select nije pokretan.
Postojeci vanjski keyboard katalog daje Space i Left/Right naredbe.
FFmpeg ostaje postojeci zamjenjivi decode adapter. V4 aktivni kod nije kopiran.

## 3. Build i automatska provjera

- `cargo test -p qnc-broadcast-player -p qnc-player-runtime -p qnc-audio-output -p qnc-pixel-convert --lib`:
  75 + 14 + 16 + 17 = 122 prolaza; dva device/card testa ostaju ignored.
- `cargo test -p qnc-player-input -p qnc-player-client -p qnc-player-runner --quiet`:
  18 + 4 + 4 = 26 prolaza. Ukupno 148 prolaza i dva ignored testa.
- `cargo build -p qnc-ingest -p qnc-player-runner`: uspjesno. Sibling
  `target/debug/qnc-broadcast-player.exe` izgraden 21:50:24; nepromijenjeni
  Ingest exe Cargo je ispravno ponovno upotrijebio (18:37:27).
- Conformance s apsolutnim QNC rootom: sve provjere prolaze, ukljucujuci
  keyboard v4-extension provjeru. `cargo fmt` i `git diff --check` prolaze.

Nije proveden cijeli `cargo test --workspace`. Nisu brisani target/cache ni
tudje izmjene. Nema commita/pusha u ovom zahvatu. Tijekom Playa nisu radili
build/test procesi; provjerene su putanje/PID-ovi novog Ingesta i playera.

## 4. Stvarni live kroz Ingest formu

Log: `target/player-independent-av-20260909.log` (lokalni dijagnosticki
artefakt, ne Git prilog). Sve naredbe dolaze kroz Ingest UI, ne testni helper
ili rucni HTTP pozivi. Start Ingesta: 21:52:17, PID 22416.

| Provjera | Rezultat |
| --- | --- |
| Mironik 2679, prvi neprekinuti Play | NE PROLAZI: zadnji carrier 2667, oko 53.34 s, `due AV frame is not prepared` |
| Mironik 2002, cijeli Play | PROLAZI do granice 10194, zadnja slika 10193, 203.88 s; bez playback greske u tom prolazu |
| 2002, Left/Right po jedan frame | Ready na 10192, zatim Ready na 10193; tocno trazeni carrier |
| 2679, drugi pokus, Pause/Play | Pause na 1194, Ready i 24000 audio sample-frameova; nastavak ide preko 1194 bez novih dekodera |
| Promjena klipa za vrijeme Playa | 2679 zaustavljen, 2002 thumbnail prikazan, Ready na 0, bez automatskog Playa |

Prvi 2679 session: `f79d809c-d820-48df-9fc6-a6034307e6c3`.
Na gresci: converted=2668, conversion_avg_us=12279, upload_avg_us=26,
tick_us=17199, max_gap_us=170505. Audio je tada jos imao 10560 sample-frameova
(220 ms pri 48 kHz). To nije ispraznjen audio red. Zapis broja konvertiranih
slika i zadnjeg carriera upucuje na nespremnu sljedecu sliku; genericka
greska ne biljezi posebno koji je red zakasnio. Precizan uzrok unutar
decode/convert/poll puta ovim zapisom nije izoliran. Nema tihog nastavka,
preskakanja slika ili podmetanja tisine nakon greske.

2002 session: `5df8942d-8052-4572-aa70-aab2b8cca9e4`. EOF dogadjaj je
`PlaybackBoundaryReached { frame: 10194 }`. To je dokaz dovrsenog engine
prolaza, ne dokaz da je monitor fizicki prezentirao svaki frame bez trzaja.

Drugi 2679 session: `a242d364-34ab-435b-9ede-53dfcae2be50`. Pause je zadrzao
carrier 1194 kroz vise ocitanja. Resume je potvrden u logu na 1776 i dalje.
Player PID 32868 i pet FFmpeg PID-ova 32372/20644/30680/15960/31948 ostali
su isti prije i poslije pauze. Pripremljeni dekoderi nisu otvarani na Play.
Promjena na 2002 prekinula je tu sesiju dok je jos bila Playing oko framea
3170. Stari player i svih pet dekodera vise nisu bili prisutni. Novi player
PID 18184 ostavljen je u Ready na 0, bez video submita ili starta audija;
u monitoru je thumbnail 2002. Ovo nije drugi puni prolaz klipa 2679.

## 5. Preostala uska grla i ogranicenja dokaza

Iz potpunih redaka prvih dvaju neprekinutih prolaza, po 100 uzoraka u retku:

| Klip | Redaka | Resize prosjek | Color prosjek | Total prosjek | Najveci total |
| --- | ---: | ---: | ---: | ---: | ---: |
| 2679 | 26 | 9.196 ms | 3.017 ms | 12.221 ms | 32.302 ms |
| 2002 | 99 | 8.700 ms | 2.545 ms | 11.251 ms | 49.434 ms |

To nisu p95 vrijednosti niti izolirani benchmark. Paralelni stderr redci
povremeno su isprepleteni; nepotpuni redci nisu brojani. CPU resize/color
put i 960x540 RGBA monitor u ovom koraku nisu mijenjani. Prosjek ispod
20 ms nije jamstvo za 50 fps rok.

HTTP `AV_F` transfer maxima iz potpunih zapisa: 321.635 ms (prvi 2679),
326.910 ms (2002). Ostaje ozbiljan problem isporuke slike i kad engine
dodje do kraja. Ne treba ga prikrivati fiksnim audio delayem.
Player working-set ocitanja oko 100 MB tijekom tih prolaza nisu formalni
vrh memorije ili dugotrajni leak test; granice pripreme potvrduju unit testovi.

Audio start-to-first-callback za prva dva Playa bio je 0.5184 i 4.0719 ms;
prvi driver delay 10 ms. To NIJE Play-command -> fizicki A/V izlaz mjerenje.
Nije potvrdena fizicka A/V sinkronizacija, presentation ack ni svaki prikazani
frame. Snimke Ingesta uzimane su u tockama testa, ne neprekidno. Povremeno
prekrivanje prozora i pauze radi korisnikova unosa ogranicavaju vizualni dokaz.

Windows/local je jedino live okruzenje. Linux/macOS, ARM, LAN/Intranet,
udaljeni output, SDI/NDI i montazni prijelazi nisu verificirani ovim testom.

## 6. Sljedeca odluka

Odvajanje pripreme sprjecava da buduca nespremna slika sama prekine
dopunjavanje PCM-a. Nije rijesilo sve video rokove i monitor transport.
Live kriterij ostaje otvoren; ne proglasavati player zavrsenim ili spremnim
za profesionalnu montazu na temelju jednog uspjesnog klipa.

Nastavak je odjeljak 5 iz docs/72: javni video/monitor izlazni put, uz
odvojene dokaze za decode/convert i isporuku/present. To nije implementirano
u ovom koraku. Nema odobrenja za novi engine, veci buffer kao zamjenu za
popravak, promjenu Projecta ili aktivni kod u formi.
