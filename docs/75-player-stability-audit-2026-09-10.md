# Broadcast Player / engine: audit stabilnosti

Datum: 2026-09-10. Radni direktorij: `C:\Users\miron\Projects\QNC`.
Referenca: `C:\Users\miron\Projects\qnc_v4`.

## Zakljucak

Trenutni Ingest playback nije prihvatljiv kao stabilan frame-precizan izlaz.
Zavrsen decode/engine prolaz nije dokaz da je korisnik vidio sve frameove,
niti da je slika bila sinkronizirana sa zvukom. Korisnik je prijavio trzaje
i preskakanje i nakon GPU zahvata; live prihvat ostaje NEUSPJESAN.

Nisu sve osnove pogresne: DB-first input, racionalni source timebase,
audio-device clock, identitet/generacija sesije i odvojeni moduli postoje.
Glavna potvrdena rupa je izmedju pripremljenog framea i njegova prikaza.
Nije opravdano dalje popravljati samo resize ili zamijeniti cijeli engine
bez rjesavanja tog ugovora.

Ovo je audit radnog stabla, ne audit cistog HEAD-a. HEAD je `875d981`;
velik dio player stacka jos je untracked, uz ranije izmijenjene datoteke.
Ne moze se samo iz povijesti commita odrediti koja je izmjena novije verzije
agenta uzrokovala svaki simptom. Nije napravljen rollback, commit ni push.

## Stvarni put podataka

```text
public_app_settings.active_project_id
  -> javni read-only work-settings
  -> public_project_settings.settings_json + spremljeni clip/probe zapis
  -> player-input / Ingest komponenta
  -> zaseban qnc-broadcast-player proces
       -> zamjenjivi decoder adapter -> priprema slike i PCM-a
       -> audio uredjaj -> jedan media clock -> engine tick
       -> Presenter -> posljednja predana RGBA slika
  -> HTTP frame upit -> posljednji client View
  -> egui texture upload / paint
  -> zaslon (nema povratne potvrde u player)
```

Forma nije decoder ni sat. Problem nije potrebno rjesavati premjestanjem
aktivnog koda u formu, Timeline, Projects ili shell.

## Nalazi

### F1 / P1: kasnjenje moze neprimjetno izbaciti slike iz prikaza

`crates/qnc-broadcast-player/src/transport_engine.rs:568` sustize dospjele
frameove u petlji. Runtime postavlja burst na 2
(`crates/qnc-player-runtime/src/lib.rs:177`). Svaki submit zamjenjuje
`VideoSink.monitor` (`crates/qnc-player-runtime/src/output.rs:83`).
Runner objavi monitor tek nakon cijelog ticka
(`tools/qnc-player-runner/src/main.rs:104`).

Ako dva framea dospiju u jednom ticku, prvi moze biti zamijenjen prije nego
sto ga transport uopce dobije. Zatim postoji jos jedan latest-frame slot
u `tools/qnc-player-runner/src/control.rs:20`, koji se prepisuje na retku 35,
te zamjena client Viewa u `crates/qnc-player-client/src/lib.rs:215`.

Time je moguce imati neprekinut slijed engine submit dogadjaja, a prekinut
slijed slika na zaslonu. Nije potrebno da FFmpeg preskoci ijedan source frame.
Ne postoji end-to-end politika koja taj gubitak prijavi kao neuspjesan izlaz.

Test `delayed_tick_catches_up_without_skipping_frames_after_preroll`
(`transport_engine.rs:1611`) provjerava fake presenter dogadjaje, ne prolaz
kroz stvarni runner/client/monitor. Zato ne otkriva ovaj problem.

### F2 / P1: video se predaje tek kad dospije na audio satu, bez ugovora prikaza

`crates/qnc-player-runtime/src/lib.rs:206` uzima poziciju audio uredjaja.
Engine na temelju nje odredjuje dospjeli frame, a tek nakon toga pocinju
objava RGBA, preuzimanje u klijentu i UI upload. Njihovo kasnjenje nije
ukljuceno u raspored prikaza.

Za Ingest monitor nema native VideoOutput tokena. Tada
`Presenter::prepare_start_frame` vraca `true`
(`crates/qnc-player-runtime/src/output.rs:64`) bez potvrde UI izlaza.
`present_frame` vraca samo `VideoFrameSubmitted`, ne `FramePresented`.
Telemetrija u promatranim logovima ostaje `presented=None`.

`MonitorHeader` (`crates/qnc-player-contract/src/session.rs:18`) ima
identitet, generaciju, sequence i broj framea, ali nema ugovoreni rok
prezentacije i mapiranje tog roka na izlazni sat. Nema povratne potvrde
stvarnog prikaza. `AV_V` je samo CPU paint submission
(`crates/qnc-ui-kit/src/raster.rs:58`), ne GPU completion niti scanout.

Posljedica: Ready govori o pripremi na producer strani, ne o spremnosti
cijelog A/V izlaza. Engine moze uredno zavrsiti klip dok slika kasni.
Postojeci logovi potvrduju pozitivan softverski A/V pomak; tocna fizicka
sinkronizacija zvucnika i zaslona nije ovim auditom izmjerena.

### F3 / P1: slika i kontrola dijele serijski blokirajuci put

`crates/qnc-player-client/src/connection.rs:190` u istom pollu obavlja
state/command zahtjev pa blokirajuci `post_binary` za frame (redak 243).
State se osvjezava najvise svakih 50 ms bez dodatne akcije. Jedan client
worker to zatim objavljuje UI-ju.

Na serveru jedan worker obrazuje frame odgovor ili ceka odgovor na kontrolu
(`tools/qnc-player-runner/src/control.rs:62`). Sporo slanje frame bodyja
moze odgoditi prihvat kontrole. Na klijentu akcija koja stigne za vrijeme
preuzimanja slike mora cekati da se taj poll zavrsi.

U logu Mironik 2002 jedno preuzimanje framea traje do 334.425 ms, a
preuzimanje s obradom do 335.952 ms. To je izmjereno trajanje poziva, ne
dokaz da sama mreza ili sam HTTP parser trosi toliko. U njemu mogu biti
scheduling, cekanje servera, prijenos i kopiranje. Strukturno blokiranje
kontrole postoji neovisno o tome koji od tih dijelova dominira.

Za 960 x 540 RGBA pri 50 fps payload je 103,680,000 B/s, prije zaglavlja
i dodatnih kopija. To nije dokaz da HTTP ne moze raditi, nego da ovaj
lokalni request/response put nije verificiran kao vremenski odredjen izlaz.
Zamjena protokola bez F1/F2 ne bi zatvorila problem.

### F4 / P2: priprema je ubrzana, ali nema dokaza dovoljne rezerve za stabilan rad

GPU pretvorba ukljucuje upload, compute, readback i CPU kopiju
(`crates/qnc-gpu-raster/src/lib.rs:367`, `:380`, `:408`). Slijede transport
i novi upload u egui (`crates/qnc-ui-kit/src/raster.rs:30`). To nije
direktan GPU izlaz u monitor. Konverzijski worker ima jedan posao u letu
(`crates/qnc-player-runtime/src/conversion.rs:58`).

Prosjek pretvorbe je bolji, ali maksimumi i dalje prelaze 20 ms interval
50 fps framea. Jedan takav maksimum sam po sebi NE dokazuje underrun:
prebuffer moze pokriti pojedinacni zastoj. Potrebna je mjera kontinuiteta
pripreme i preostale rezerve, ne samo prosjek pretvorbe.

Audio otvara decoder za svaki spremljeni stream
(`crates/qnc-player-runtime/src/output.rs:141`) i ceka sve trackove prije
interleavea (`:222`, `:240`). Za ove originale to je cetiri mono dekodera,
uz zaseban video decoder, iako projekt koristi dva izlazna kanala. Mapiranje
na projektni izlaz dogadja se poslije. To je dodatni trosak i ovisnost o
spremnosti neizlaznih kanala; nije dokaz da je to uzrok svakog aktualnog trzaja.

Tvrdi prekid je zasebna klasa kvara. Kada dospjeli A/V frame nije spreman,
`transport_engine.rs:582` vraca gresku i zaustavlja transport. Audio underrun
takodjer zaustavlja engine (`qnc-player-runtime/src/lib.rs:223`). To su
namjerne zastite, ne nesto sto treba sakriti izbacivanjem frameova ili
ponavljanjem zvuka. Raniji CPU log biljezi takav prekid kod framea 2667,
uz jos 10560 sample frameova u audio redu. Novi GPU log Mironik 2002
zavrsava normalnom OUT granicom, ali vizualni prihvat nije prosao.

### F5 / P2: nevaljano ili preklopljeno ocitanje audio sata vraca pocetnu poziciju

`crates/qnc-audio-output/src/queue.rs:302` vraca `anchor` ako se timing
snapshot preklopi s callback upisom ili nema driver timestamp. Tijekom
Playa to moze biti povratak na pocetak trenutnog raspona, a ne posljednja
valjana clock pozicija. Engine tada privremeno nema novi dospjeli frame.

Ready/Start ne zahtijeva raspoloziv driver timing (`queue.rs:215`). Ako ga
backend ne daje, audio moze krenuti, dok video sat ostaje na anchoru.
To je konkretan multiOS/backend rizik, ali nije dokazan uzrok ovih Windows
sesija: one imaju zabiljezen driver delay od 10 ms. Treba odvojeno testirati
monotonost i valjanost clock snapshota, bez uvodjenja drugog UI sata.

### F6 / P2: provjere ne zatvaraju kvalitetu stvarnog izlaza

Conformance provjerava granice i deklaracije, ne cadence ili sinkronizaciju.
Postojeci testovi pripreme, PTS-a i submit slijeda nisu zamjena za dokaz
prezentacije u Ingestu. I sam raster test provjerava ponovno koristenje
teksture, ne koji je frame zaslon prikazao u kojem trenutku.

Dijagnostika koristi sinkrone `eprintln!` pozive i zajednicki stderr; dio
zapisa je isprepleten. Iz razlika izmedju parsiranih sequenceova nije
ispravno izracunati tocan broj izgubljenih zaslonskih frameova. Treba
provjeriti i trosak ukljucene dijagnostike. To nije dokaz da je upravo ona
glavni uzrok korisnikovih trzaja.

## Ponovno analizirani logovi

Postojeci alat, bez pokretanja decoder/probe procesa:

```text
node tools/diagnostics/av-offset.mjs target/player-gpu-resume-20260910.log 50 1
node tools/diagnostics/av-offset.mjs target/player-gpu-preparation-20260909.log 50 1
```

FPS 50/1 potvrden je u stvarnom DB clip zapisu, ne izveden iz Project FPS-a.

| Mjera | Mironik 2002, 10.9. | Mironik 2679, 9.9. |
| --- | ---: | ---: |
| Spremljeno trajanje | 10194 framea | 11178 frameova |
| CPU paint / audio timing uparivanja | 3255 | 4820 |
| Audio ispred CPU painta, medijan | 71.852 ms | 58.439 ms |
| Isti pomak, P95 | 185.212 ms | 193.500 ms |
| Isti pomak, maksimum | 415.487 ms | 418.020 ms |
| Frame HTTP poziv, medijan | 8.085 ms | 7.813 ms |
| Frame HTTP poziv, maksimum | 334.425 ms | 324.729 ms |
| Engine kraj | frame 10193 / OUT 10194 | frame 11177 / OUT 11178 |
| Potvrda fizickog prikaza | nema | nema |

Ovo su softverske opservacije iz prethodnih live sesija, ponovno analizirane
u ovom auditu. Alat uparuje CPU paint s najblizim driver timing zapisom unutar
75 ms i izbjegava prelazak izmedju razlicitih audio generacija. Moze brojati
vise crtanja iste slike. Broj opservacija nije broj prikazanih frameova.
Ove brojke nisu mjerenje fizickog lip-synca, niti dokaz da je sav pomak
uzrokovan samo HTTP-om. Nisu ni dokaz stalnog drifta source timestampova.
Prekinuta nocna 2002 sesija pri zatvaranju poklopca nije test stabilnosti.

## DB-first i ono sto vec radi

Read-only provjera `data/project_store.db` daje aktivni projekt:
`bbvvcx_14934338a30a4378a64903801dcc849a`, naziv `bbvvcx`.
Njegova baza je:
`C:\Users\miron\Test projekt\bbvvcx_14934338a30a4378a64903801dcc849a\qnc_project.db`.

Stvarni `settings_json`: `audio.channels=2`, `audio.sample_rate=48000`,
`transcribe_channel=CH1`, `atmosphere_channel=CH2`,
`playback.input=proxy_if_available`, `playback.cache.mode=off`.
Projektni video zapis je 1920x1080, progressive, Rec.709, 50 fps.

`qnc-work-settings/src/local.rs:85` cita aktivaciju i javne postavke uz
read-only/query_only konekciju. `qnc-player-input/src/lib.rs:199` ponovno
cita postavke, provjerava workspace i ucitava spremljeni clip/snapshot.
`ProjectAudio::read` koristi broj kanala i sample rate iz baze, a
`qnc-player-runtime/src/input.rs:187` primjenjuje projektni broj kanala.
Izbor slike je proxy ako postoji; audio inventory ostaje iz originala.

U oba promatrana DB zapisa original ima cetiri mono PCM streama po 48 kHz;
originalni video i audio imaju start_pts=0. Proxy video takodjer ima
start_pts=0. Spremljeni proxy time_base 1/50000 i originalni 1/50 nisu
razliciti FPS-ovi: oba videa su spremljena kao 50 fps. Nije nadjena
DB vrijednost koja bi sama objasnila izmjerenih 58-72 ms kasnjenja.
To ne dokazuje da su izvorni audio i video fizicki savrseno sinkroni.

Source playback koristi spremljeni source timebase, ne projektni/export
FPS, sto je i pravilo v4. FFmpeg adapter koristi saved container/codec,
`-nofind_stream_info`, `-copyts` i izlazne PTS zapise. Audit nije pokretao
ffprobe, Select ni novi scan. Ne predlaze nove Project postavke.

## Sto stvarno pokazuje v4

V4 `AGENTS.md:272` propisuje racionalni source timebase, jedan vlasnik
Playa i pasivne forme. Ta pravila treba zadrzati.

V4 `qnc-player-runtime/src/process_client.rs:82` otvara lokalni frame map;
kontrola ide zasebnim JSONL kanalom. Frame transport koristi memmap2
(`qnc-player-frame-transport/src/lib.rs:11`) i `read_latest` (`:180`).
To uklanja ovaj HTTP RGBA roundtrip, ali je i dalje latest-frame model:
sama kopija mmap koda ne garantira da monitor prikaze svaki frame na vrijeme.

Ni v4 naziv `FramePresented` nije svugdje fizicki dokaz. Primjer je
`qnc-player-output/src/lib.rs:35`: event-only presenter vraca ga nakon
validacije payloada. Zato postojece v4 testove/evente treba tumaciti prema
stvarnom izlazu, ne samo prema nazivu dogadjaja. V4 nije mijenjan niti je
u ovom auditu ponovljen usporedni live test.

## Predlozeni nastavak, bez implementacije

1. Prvo definirati i odobriti jedan javni ugovor video izlaza: identitet,
   generacija, frame/timebase, ugovoreni trenutak prikaza i mapiranje na
   audio clock, ogranicen red, spremnost izlaza, potvrda predaje/prikaza te
   jasan prijavljen prekid kada izlaz ne moze odrzati dogovoreni ritam.
   Ne glumiti scanout potvrdu preko UI repainta ili samog GPU fencea.
2. Unutar player/output modula zamijeniti best-effort latest-frame put
   vremenski upravljanim izlazom. Lokalni adapter moze koristiti dijeljenu
   memoriju/GPU resurse; udaljeni adapter mora imati isti vremenski ugovor
   i vlastitu provjeru izvedivosti. Transport URI i DB-first ostaju.
   UI samo prikazuje povrsinu, bez sata i playback matematike.
3. Kontrolni kanal odvojiti od velikog frame prijenosa. Play/Pause/Cue ne
   smiju cekati preuzimanje slike. Uskladiti engine catch-up s izlaznim
   ugovorom tako da ne izbacuje medjuframeove neprimjetno u mailbox.
4. Zatim mjeriti stvarnu pripremnu rezervu; suziti nepotreban audio decode
   i GPU readback samo gdje mjerenje dokaze korist. Provjeriti clock
   snapshot monotonost i odbijanje nevaljanog audio timing backenda.
5. Prihvat iskljucivo kroz stvarni Ingest: Mironik 2002 i 2679 do kraja,
   Play/Pause, frame-step, seek i promjena klipa. Mjeriti razmak stvarnih
   prezentacija i A/V pomak u vise tocaka klipa; provjeriti display refresh
   i mogucnost prikaza 50 fps. Potom isti ugovor provjeriti na LAN/Intranet
   i drugim OS-ovima. Bez toga nema oznake "stabilan" ili "1-frame tocan".

Za taj nastavak treba potvrda smjera; ovaj audit nije odobrenje novog
enginea, UI redizajna, novog probea ni izmjena zamrznutog Project koda.

## Verifikacija i ogranicenja

- Procitan aktualni izvorni kod navedenih putova i pravila QNC/v4; provjereni
  stvarni Project i clip/probe DB zapisi iskljucivo read-only.
- Ponovno analizirana oba navedena loga stvarnog Ingesta.
- Ponovno pokrenut postojeci `target/debug/qnc-conformance.exe` nad QNC rootom:
  all checks passed. Audit nije rebuildao taj binary.
- Test/build rezultati prethodnog koraka navedeni su u docs/74; nisu novi
  testovi ovog audita. `cargo test --workspace` nije ovdje izvrsen.
- Trenutno nadjeni Ingest PID 21292 i njegov player PID 11920 oba imaju
  executable path pod `QNC/target/debug`. Nije nadjen paralelni qnc-app,
  ffmpeg ili ffprobe u tom process snapshotu. Nista nije ubijeno/restartano.
- Nije izvrsen novi live Play, fizicko mjerenje A/V, display scanout/cadence,
  LAN/Intranet test, Linux/macOS test ni test slozene montaze.
- Jedina nova datoteka ovog audita je ovaj izvjestaj. Kod, baze, kartica,
  postavke i postojeci radni diff nisu mijenjani.

Sljedeci rizik: razvijati Filmstrip, Wave ili slozenu montazu iznad izlaza
koji jos ne zna razlikovati "predano" od "prikazano na vrijeme".
