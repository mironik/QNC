# MLT / Shotcut: izolirana provjera na stvarnoj kartici

Datum: 2026-09-09. QNC HEAD `875d981`, postojece necommitano radno stablo.

## Opseg i odobrenje

Korisnik je odobrio MLT test i njegovu dodatnu analizu streamova ISKLJUCIVO
u ovom izoliranom testu. To nije promjena AGENTS.md niti odobrenje za probe
u produkcijskom playeru. Nisu mijenjani Project, Ingest, shell, player runtime,
projektne postavke ni media zapisi. Nije uveden novi engine u aplikaciju.

Test koristi stvarne klipove s G: kartice, read-only, bez pisanja na karticu.
Nema ponovnog Selecta, scana, importa niti poziva ffprobe executablea.
Standardni MLT avformat i referentni FFmpeg otvaraju i analiziraju medij;
zato ovo izricito NIJE test rada bez probea.

Shotcut je gotova aplikacija izgradjena nad MLT-om, ne konkurentski engine.
Koristen je njegov sluzbeni portable paket kao izvor MLT/FFmpeg biblioteka.
Shotcut UI nije kopiran niti pokrenut. QNC egui UI ostaje nepromijenjen.

## Verzije i ulaz

- Shotcut portable: 26.8.1 Windows x64, bez instalacije i globalnih promjena.
- SHA256 ZIP-a, provjeren prema sluzbenoj datoteci:
  `b0148856de01b39add4bf4d6a813bfbc554b4663b65e3ca25cb2589f47555a6a`.
- Ukljuceni MLT prijavljuje **7.41.0**.
- Ukljuceni FFmpeg: `n8.1.2-34-g9b6c8969e0`.
- Aktivni projekt procitan iz postojece baze: `bbvvcx`.
- Projektna pravila: `playback.input=proxy_if_available`, audio 2 kanala,
  48000 Hz. Nisu uzeta iz lokalnog player JSON-a niti iz broja proxy kanala.
- Slika: spremljeni Sony proxy, 1920x1080, 50 fps, YUV420p.
- Zvuk: originalni MXF, 4 zasebna mono streama, svaki 48000 Hz.
- Source FPS i broj frameova dolaze iz spremljenog media zapisa.
- Mironik 2002 proxy ima spremljen `frame_rate_mode=unknown`, iako zapis
  sadrzi 50 fps i tocan broj frameova. Test koristi tu spremljenu vremensku
  bazu, ali NE pretvara unknown u potvrdeni CFR. Mironik 2679 je constant.

Izolirani dijagnosticki alat izravno cita SQLite u `mode=ro` i `query_only`,
koristeci javne prikaze za podatke i privatni lokalni binding za putanju.
To nije novi produkcijski DB adapter niti zamjena za qnc-work-settings i
qnc-player-input. Putanje medija razrjesavaju se iz postojecih URI bindinga
u ingest-transport.json, bez novih pretpostavljenih odredista.

## Izmjereno

Oba klipa su procitana od framea 0 do zadnjeg framea iz baze, bez preskakanja.
Za svaki frame dekodirana je stvarna slika i originalni audio. U memoriji se
racunaju SHA256 otisci; slike i PCM nisu pisani kao privremene datoteke.

| Mjerenje | Mironik 2002 | Mironik 2679 |
| --- | ---: | ---: |
| Frameova iz baze i potpuno dekodiranih | 10194 | 11178 |
| Trajanje medija | 203.88 s | 223.56 s |
| Vrijeme prolaza | 104.121 s | 97.914 s |
| Prosjecna obrada | 97.905 fps | 114.161 fps |
| Medijan rada po frameu | 6.302 ms | 5.953 ms |
| P95 rada po frameu | 28.940 ms | 25.185 ms |
| Maksimum, ukljucujuci pocetnu pripremu | 297.630 ms | 263.772 ms |
| Frameova s obradom duzom od 20 ms | 1095 | 857 |
| Audio uzoraka po originalnom kanalu | 9786240 | 10730880 |
| Seek usporedbe sa sekvencijalnim prolazom | 37/37 | 37/37 |
| Medijan seek + image/audio decode | 456.440 ms | 346.490 ms |
| Maksimalni seek + image/audio decode | 629.160 ms | 574.720 ms |

Ovo je throughput dekodiranja, kopiranja u memoriji i racunanja hashova,
ne tempo fizickog monitora. Nije usporedba jednakih QNC/MLT izlaznih pipelinea.
Prosjek iznad 50 fps NE dokazuje da playback nema zastajkivanja; P95 je iznad
20 ms i potrebni su priprema, buffer i provjera stvarnog consumera/izlaza.

Otvaranje video/audio producera, odvojeno od prvog dekodiranog framea:
2002: 1068.868 / 504.229 ms; 2679: 138.514 / 473.592 ms.
Nije kontroliran cold-cache benchmark. Nije mjeren Ready -> Play -> izlaz.

### Preciznost slike

Nakon sekvencijalnog prolaza otvoreni su novi produceri kako njihov prethodni
frame cache ne bi prikrio netocan seek. Test pokriva pocetak, kraj, susjedne
frameove oko sredine te 30 deterministicki nasumicnih pozicija po klipu.
Svih 74 usporedbe slike I svih 74 usporedbe cetverokanalnog PCM-a podudaraju
se sa spremljenim hashom odgovarajuceg sekvencijalno dekodiranog framea.

Dodatno: zasebni FFmpeg CLI sekvencijalno dekodira proxy od pocetka i daje
SHA256 sirovog YUV420p na 10 odabranih pozicija po klipu. MLT na iste
pozicije dolazi obrnutim redom seekom. **20/20 slika je identicno**, svaki
klip ima 10 razlicitih kontrolnih slika. Ukljuceni su prvi i zadnji frame te
susjedne pozicije. Referenca dijeli FFmpeg decoder obitelj s MLT-om, ali ne
koristi njegov seek, producer poziciju ili cache.

To je dokaz za testirane formate/pozicije, ne garancija za svaki codec,
VFR, interlace, promjenu brzine ili montaznu sekvencu.

### Audio kanali

MLT je od originala procitao sva 4 mono streama kao odvojene kanale. Prva dva
kanala, prema broju iz projekta, usporedjena su sa zasebnim citanjem streama
1 i 2. **20/20 usporedbi uzoraka je identicno**, ukljucujuci kraj klipa.
Nema zbrajanja kanala, proxy zvuka ni zakljucivanja da su CH1/CH2 stereo.
Ovo provjerava sadrzaj/mapiranje, ne fizicki audio uredaj ili uloge montaze.

## Otvorene zapreke

1. Standardni MLT avformat radi vlastitu analizu streamova. Samo iskljucivanje
   ffprobe executablea ne zadovoljava QNC pravilo. `avformat-novalidate`
   odgadja otvaranje, nije dokaz uklanjanja analize. Prije integracije treba
   dokazati DB-first media adapter bez te analize; nije implementiran ovdje.
2. Mironik 2679: MLT proxy producer prijavljuje **11179**, a baza i original
   **11178** frameova. Spremljeni proxy video traje 223.56 s, cijeli kontejner
   223.573313 s, a proxy audio je dulji od slike. To je u skladu s razlikom
   zbog trajanja kontejnera/zaokruzivanja, ne dokaz dodatne video slike.
   Test nije prepisao bazu niti dekodirao izvan njenog video raspona.
   Buduci adapter mora dosljedno koristiti DB video granicu [0, 11178).
3. Nije mjeren stvarni A/V pomak na uredaju, dropout, Play/Pause/seek izlaz,
   Ready-priprema, zamjena klipa ni dugotrajni consumer playback.
4. Nije testiran MLT u Ingest formi, LAN/Intranet, Linux/macOS, druga CPU
   arhitektura, profesionalna sekvenca, prijelazi, SDI/NDI ili audio resampling.
5. Bundled FFmpeg tijekom otvaranja proxyja prijavljuje `infe version < 2` upozorenje.
   Dekodiranje/hash provjere su prosle; podrsku svim container metapodacima
   time nismo dokazali. Upozorenje ostaje u logu, nije razlog za probe retry.

## Artefakti i ponavljanje

- Test: `tools/diagnostics/mlt-evaluate.py`, nije dio Cargo runtimea.
- `target/mlt-eval/full.json`: puni decode i seek rezultati.
- `target/mlt-eval/audio-map.json`: provjera mono mapiranja.
- `target/mlt-eval/reference.json`: FFmpeg/MLT kontrolni hashovi.
- Odgovarajuci `*.stderr.log`: izvorna upozorenja.
- SHA256 datoteka registryja, aktivne projektne baze i media-records baze
  ostao je isti prije i nakon svakog od tri prolaza.

Paket i rezultati su lokalni testni artefakti u ignoriranom targetu.
Pokretanje zahtijeva Python sa sqlite3/ctypes, raspakiran sluzbeni Shotcut
paket i izricito odobrenje analize. Primjer iz QNC root direktorija:

```powershell
$mlt = (Resolve-Path -LiteralPath target/mlt-eval/Shotcut).Path
$env:MLT_DATA = Join-Path $mlt 'share/mlt'
$env:MLT_PROFILES_PATH = Join-Path $mlt 'share/mlt/profiles'
$env:MLT_REPOSITORY = Join-Path $mlt 'lib/mlt'
$env:PATH = "$mlt;$env:PATH"
python -I tools/diagnostics/mlt-evaluate.py --root . --mlt-dir $mlt --output target/mlt-eval/full.json --allow-stream-analysis
```

Za dodatne provjere dodati `--audio-map-only` ili `--reference-only` i zadati
drugo ime izlaznog JSON-a. Bez `--allow-stream-analysis` alat odbija rad prije
citanja baze/otvaranja medija. Ova zastavica nije trajna korisnicka dozvola.

## Zakljucak i sljedeci korak

MLT je smislen kandidat za daljnju kvalifikaciju dekodiranja i frame-based
montaznog enginea. Nije odobren kao neposredna zamjena niti je ovim testom
zatvoren Broadcast Player ili live Ingest korak prema AGENTS.md.

Prva odluka nije promjena UI-ja nego moze li MLT adapter zadovoljiti strogi
DB-first/no-probe ulaz i tocan video raspon. Tek nakon toga ide ogranicena
integracija u postojeci javni out-of-process player, bez drugog sata u UI-ju,
te stvarni Ingest test spremnog Playa, A/V sinkronizacije, seeka i zamjene
klipa. Ako no-probe granica ne moze biti odrzana, standardni MLT nije
prihvatljiv za ovaj projekt bez izricite promjene korisnickog pravila.

## Sluzbeni izvori

- [Shotcut download i checksum](https://www.shotcut.org/download/)
- [Shotcut ovisnosti](https://github.com/mltframework/shotcut)
- [MLT framework](https://www.mltframework.org/docs/framework/)
- [MLT avformat producer](https://github.com/mltframework/mlt/blob/master/src/modules/avformat/producer_avformat.c)
- [MLT producer API](https://www.mltframework.org/doxygen/structmlt__producer__s.html)
- [MLT consumer API](https://www.mltframework.org/doxygen/structmlt__consumer__s.html)
