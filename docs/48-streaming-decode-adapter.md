# Streaming decode adapter

2026-09-08, opseg prije implementacije. Nastavak docs/47 i AGENTS 8.2.

Sljedeci korak je javni qnc-media-decode, odvojen od player sata, baze,
workflowa i UI-ja. Ulaz je vec odabrani MediaRepresentation iz javnog
spremljenog zapisa i otvoreni QNC MediaStream. Nema original/proxy odluke,
novih postavki ni probea. Project freeze ostaje zatvoren.

Jedna instanca dekodira jedan eksplicitni video ili audio stream kontinuirano
jednim FFmpeg procesom. Native video pixel format i bit depth cuvaju se;
audio izlaz je PCM f32le s izvornim sample rateom i svim kanalima tog streama.
Vise source audio streamova ne postaju prebrisani ili lazni stereo kanali:
caller otvara svaku zeljenu native stream instancu zasebno.

Izlaz je niz paketa s izvornim media/stream identitetom, ordinalom,
cjelobrojnim PTS-om i racionalnim timebaseom te ogranicenim byte payloadom.
PTS pripada stvarnom dekodiranom izlazu, ne UI satu ili pretpostavljenom FPS-u.
Unknown/variable FPS ne pretvara se u CFR. Ovaj rez ne tvrdi da je paket
prezentiran, da je cujan niti da ima potvrden source frame broj nakon seeka.

FFmpeg koristi -nofind_stream_info, spremljeni demuxer/decoder i eksplicitni
stream indeks. Jedan proces po streamu, ne po frameu. Bounded queue, bounded
packet/line size, timeout, cancel i Drop kill/wait/join moraju biti provjereni.
Na kraju se provjerava izlazni status; prazan stdout nije automatski uspjeh.

Novi uski loopback byte bridge u qnc-media-stream omogucuje da FFmpeg cita
vec otvoreni MediaStream. Za remote ulaz QNC klijent provjerava svaki raspon
prije predaje bajtova dekoderu. FFmpeg ne dobiva udaljeni endpoint/credential
izravno, pa ne zaobilazi QNC provjeru verzije/URI-ja/stamp-a. Bridge nije
poslovni servis niti veza izmedu aplikacija; pripada jednoj decode sesiji.

V4 referenca: qnc-media-ffmpeg/src/lib.rs, FfmpegVideoStream i kontinuirani
pipe reader, te aktivni qnc-player-runtime. Preuzima se nacelo kontinuiranog
procesa, ne stari paket/probe/generatori, FPS fallback ili lokalne media putanje.

Za strogo formatiran timestamp/payload opis koristi se FFmpeg stats_mux_pre
sa zadanim formatom, ne parsing slobodnog debug/probe izvjestaja:
[FFmpeg advanced options](https://ffmpeg.org/ffmpeg.html#Advanced-options).
Taj opis prati pakete koji se upravo dekodiraju, nije dodatni media probe.

Plan verifikacije: zahtjevi/format/granice, poredak i PTS, backpressure,
cancel/EOF/exit failure, Local i remote bridge, stvarni DB/card video/audio.
Bez UI izmjena. Fizicki LAN/TLS i A/V device sync nisu potvrdeni ovim korakom.
Nakon dekodera i dalje slijede output adapteri i samostalni player runtime.

## Provjera implementiranog dekodera

- qnc-media-decode: 6 unit testova i 3 eksplicitna FFmpeg testa (kontinuirani
  izlaz/EOF, cancel s punim redom i izolacijom sesija, nevaljan medij).
- qnc-media-stream: Local i LAN/Intranet ugovor kroz loopback HTTP testove,
  provjera source promjene, bounded Read/Seek i autorizacija. To nije fizicki
  LAN ili TLS deployment test.
- Stvarna kartica G: samo read-only. Javni DB/input ugovor odabrao je proxy
  klipa Mironik 1522 iz projekta novi-cjeloviti-1; baza nije mijenjana.
- Cijeli proxy: 482 video framea, yuv420p 1920x1080, PTS 0..481000 uz
  timebase 1/50000; 452 stereo PCM f32le paketa na 48 kHz. Jedan proces po
  streamu. Seek na 4.82 s vraca PTS 241000 i isti hash kao sekvencijski frame.
- Zasebna dijagnostika originala ne mijenja playback.input: 8 native
  yuv422p10le video frameova te 8 paketa svakog od 4 mono audio streama.
  Nema downmixa niti zamjene original/proxy politike. Svi procesi ugaseni.
- Izvorne datoteke hashirane prije i poslije; sadrzaj nepromijenjen.

Pocetni serijski codec byte bridge imao je zastoj oko 5 s pri MP4 seeku.
FFmpeg 8.1.1 moze drzati prethodnu vezu otvorenom dok ceka novu; dva bounded
worker threada sada koriste odvojene pozicije s kratkim lockom samo za
media read/seek, bez locka tijekom socket writea. FFmpeg short_seek_size=1
izbjegava zateceno pogresno drain racunanje stare veze. Dijagnosticki logovi
uklonjeni su; credentiali se ne ispisuju.

Release read-only mjerenje nakon izmjene: prvi proxy video paket 126 ms,
cijeli video s hashiranjem svakog framea 1636 ms; prvi audio paket 109 ms,
cijeli audio 162 ms; prvi seek paket 132 ms. Ovo su mjerenja jednog klipa na
jednom racunalu s vec citanim izvorom, NE Play latency niti A/V playback test.

## Obvezni sljedeci zahvat: priprema prije Playa

Korisnik je precizirao da Play mora biti trenutan. U sadasnjoj cistoj jezgri
transport_engine::play jos postavlja play_pending_preroll, a tick poziva
advance_play_preroll. SourceReady se objavljuje prije pocetnog buffera.
To NIJE zavrsen playback put i mora se promijeniti prije runtime/UI spajanja.

Odabir klipa mora pripremiti decoder, pocetni bounded video/audio buffer i
izlaz u player procesu. Tek tada je Ready. Play na spremnom klipu samo
pokrece pripremljeni izlaz i sat, bez DB/media opena, spawna i decode cekanja.
Pause zadrzava spremne resurse. Seek/promjena izvora ponistavaju spremnost
ako se trazeni izlaz vise ne nalazi u bufferu. Forma ostaje pasivna.

Nije implementirano/potvrdjeno: A/V device output, command-to-output Play
latency, sinkronizirani stvarni izlaz, out-of-process player runtime i UI.
Nisu testirani svi codeci, VFR/interlace/rotation izlazi, Linux/macOS/ARM ni
fizicki LAN/TLS. Nepodrzani native pixel format/container daju gresku.
Decoder limit pokriva vlastite packet redove, ne cijeli FFmpeg proces niti
pakete koje caller zadrzi. Byte bridge je privatan codec adapter; codec mora
zatvoriti sockete prije njegova Dropa. Nije opci javni mrezni server.
AAC padding ostaje u dekodiranom izlazu; player mora primijeniti spremljene
vremenske granice, ne mijenjati DB niti pokrenuti dodatni probe.

Zavrsna provjera ovog reza: 71 ciljani test (core 48, decode 6, media-stream
16 + example 1) te dodatno sva 3 eksplicitna FFmpeg testa prolaze. Conformance
i git diff --check prolaze. Clippy --all-targets --no-deps -D warnings za
dva dirana modula prolazi. S ukljucenim dependency lintovima Clippy nalazi
postojeci permissions_set_readonly_false u qnc-ingest-store/content/database.rs;
taj nepovezani kod nije mijenjan. Nije pokrenut cargo test --workspace.
Hash globalne i aktivne projektne baze ostao je isti; zamrznuti Project scope
nema izmjena. Nema preostalih FFmpeg/inspect_decode procesa. To ne zatvara
gore navedeni kriterij trenutnog Playa.
