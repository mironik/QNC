# Javni audio output adapter

2026-09-08, opseg prije implementacije. Nastavak docs/49, AGENTS 8.2.

Prvi stvarni izlaz je uski `qnc-audio-output`. To je javni device adapter,
ne player, aplikacija, dekoder niti UI komponenta. Worker posjeduje CPAL
stream; callback trosi samo unaprijed pripremljene PCM f32 uzorke iz
ogranicenog SPSC reda. Nema baze, source I/O, procesa dekodera ni probea.

Ugovor: tocni sample rate i broj kanala, eksplicitni uredaj ili sistemski
default, kapacitet i pocetni prag reda. Nepodrzan format daje gresku; nema
downmixa, izmisljene stereo mape ili resamplinga. Sample pozicije su brojevi
audio frameova, ne video FPS. Vise streamova spaja zaseban adapter prije
ovog izlaza, prema spremljenoj mapi, nikad ovaj modul.

Open pokrece driver s tisinom i ceka prvi callback. Begin ponistava staru
generaciju i ceka potvrdu praznjenja. Queue/commit ostaju necujni; Ready
zahtijeva pripremljeni red. Start je samo atomic gate bez opena, alokacije,
decodea, DB pristupa ili cekanja prvog buffera. Pause zatvara gate i potvrdom
callbacka odbacuje stari red, ali ne zatvara uredaj. Hardware vec predani
uzorci imaju ogranicen izlazni rep; pause nije obecanje nulte fizicke latencije.

Svaka instanca izolira red, generaciju i lifecycle. Callback nema mutex,
cekajuci kanal, alokaciju ni logiranje. Potpuni blok ulazi u red atomicki.
Overflow odbija cijeli blok. Underrun zaustavlja gate i javlja gresku bez
automatskog ponovnog starta. Eksplicitno najavljen EOF je Drained, ne greska.

Local/LAN/Intranet koriste isti PCM/generation ugovor na output rubu:
odabir medija i prijenos do tog ruba pripadaju zasebnim modulima. Ovaj rez
NE implementira mrezni audio sender/receiver, video output niti cijeli
out-of-process player worker. Ne uvodi lokalni raw path kao javni izlaz.
Te granice i A/V sinkronizacija ostaju uvjet prije spajanja aplikacijskog UI-ja.

V4 referenca: qnc-player-output (CPAL callback i device telemetry).
Ne kopiraju se stari raw-file sinkovi, Null sink, event-only presenter,
downmix ni ovisnost o cijelom qnc-media-ffmpeg. Novi kod slijedi uski ugovor.

Dokumentacija ovisnosti:
[CPAL 0.17.1](https://docs.rs/cpal/0.17.1/cpal/),
[rtrb SPSC queue](https://docs.rs/rtrb/0.3.2/rtrb/).
CPAL stream se izricito pokrece u Open fazi s tisinom zbog razlika OS backendova.

Verifikacija: callback s poznatim uzorcima (tihi preroll, bez dupliranja,
pause/generacija, overflow, underrun/EOF, kanali), device format validacija,
stvarni lokalni audio driver s kratkim tihim test signalom i lifecycleom.
Callback predaja i driver playback timestamp NISU mikrofon/loopback potvrda
fizicki cujnog zvuka. Ne predstavljati ih kao zavrsen A/V Play live test.

## Implementirano

- `crates/qnc-audio-output`: odvojeni javni modul 0.1.0 s CPAL workerom,
  bounded rtrb PCM redom, generacijom i eksplicitnim Open/Begin/Queue/Commit/
  Start/Pause/Finish lifecycleom. Uspjesni Start radi samo provjeru i atomic
  gate; priprema je sinkrona API operacija za player worker, ne UI thread.
- Driver mora podrzati tocni f32 sample rate i broj kanala. Uredaj se bira
  CPAL identitetom ili eksplicitnim None za OS default, bez skrivenog
  prebacivanja na drugi uredaj nakon greske. Pocetni prag ne moze biti manji
  od vec opazenog callback buffera osim kod kratkog, unaprijed potvrdenog EOF-a.
- Queue prima cijele interleaved blokove i zahtijeva kontinuitet broja
  sample framea; greska nikad ne prihvaca samo dio bloka. F32 ulaz mora biti
  konacan u [-1,1]; vrijednosti izvan raspona se odbijaju, ne potajno rezu.
- Conformance provjerava Cargo/manifest verziju i tranzitivnu granicu:
  audio-output ne smije povuci player jezgru, decoder, input/DB, UI ili app.
  Tehnicki device modul ne smije preuzeti njihovu orkestraciju.

## Verifikacija i preostalo

- `cargo test -p qnc-audio-output -p qnc-player-contract -p qnc-broadcast-player -p qnc-conformance --all-targets`:
  97 prolaza; zasebni device test ostaje opt-in.
- `cargo test -p qnc-audio-output --lib real_device -- --ignored --nocapture --test-threads=1`:
  prolaz na Windows WASAPI Speakers, 48 kHz, 2 kanala; kratki signal amplitude
  0.015. Potvrdjeni tihi preroll (0 submitted), Start, Pause/flush, ponovna
  priprema bez zatvaranja uredaja i Drained s tocno 9600 sample frameova.
- Prvi Start: API 2600 ns, predaja prvom callbacku nakon 9.2874 ms.
  Resume: API 2800 ns, predaja prvom callbacku nakon 7.2215 ms.
  Driver u oba slucaja prijavljuje jos 10 ms do playback vremena.
  To su dva mjerenja jednog drivera, ne SLA i ne akusticki loopback test.
- Ciljani Clippy `--all-targets --no-deps -- -D warnings`, format i puni
  conformance prolaze. Nisu mijenjani aplikacijski UI, postavke ni baze;
  nije citana/pisana kartica niti pozvan FFmpeg/probe u ovom koraku.

Ovaj modul jos NIJE spojen na Broadcast Player jezgru/decode ili Ingest.
Nema zavrsenog out-of-process player procesa, mreze za A/V izlaz ni video
prezentacije. Linux/macOS, ostali CPU-i i fizicki LAN/Intranet nisu live
verificirani. CPAL backend moze blokirati tijekom OS otvaranja/zatvaranja;
neki backendovi ne postuju timeout otvaranja. To je izvan Start puta i mora
ostati na workeru. Gubitak uredaja zahtijeva eksplicitni novi Open.

Sljedeci korak: zasebni video output/presentation ugovor s potvrdom stvarne
prezentacije, zatim uski player worker za spajanje vec razvijenih modula.
A/V mapiranje, sample granice, seek i zajednicki ritam treba verificirati
prije UI integracije. Ne siriti ovaj audio modul dekoderom ili workflowom.
