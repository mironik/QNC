# Player media byte transport

Datum: 2026-09-08. Nastavak docs/46. Opseg prije implementacije.

Postojeci source-reader ima stat/list i ogranicene whole-file text/binary
read operacije. Nema seek/range pristup velikom klipu. Ne podizati limit
da cijeli klip stane u RAM niti kopirati klip u temp za player.

Dodaje se javni qnc-media-stream: Read + Seek za lokalni ili udaljeni QNC
izvor. LocalSource ostaje jedina capability granica direktorija; novi
modul ne duplicira pravila OS putanja, ne skenira i ne cita media format.
LocalSource dobiva uski read-only file-handle ulaz po QNC referenci.

Mrezni ugovor: autentificirani GET/HEAD /v1/media/bytes?uri=<QNC URI>,
jedan RFC byte range, Content-Length/Content-Range, QNC verzija, URI i
storage stamp. Klijent odbija promijenjenu velicinu/mtime, pogresan URI,
verziju ili raspon, pogresnu deklariranu duljinu, nepotpuno tijelo i redirect. Nema automatskog
retrya ni lokalnog fallbacka. HTTPS osim loopbacka. Stamp nije content
hash i ne dokazuje da datoteka nije prepisana uz sacuvanu velicinu/mtime.

Projekt i izbor original/proxy ostaju u prethodnom javnom DB ulazu.
Stream dobiva vec izabrani QNC media URI, ne zna za Project, Ingest,
postavke, timeline ili player sat. Byte cursor nije playback playhead.

V4 je referenca za streaming decode i razdvojeni clock/output: aktivni
qnc-media-ffmpeg, qnc-player-runtime, qnc-player-runner. Ne kopira se puni
paket, probe, generatori, monolitni runner niti app-specific aktivni kod.
Ovaj transport implementira se zasebno, uz postojece javne QNC adaptere.

Nema UI izmjena. Plan provjere: Read/Seek, granice i izmjena datoteke,
auth/range/URI greske, Local/LAN/Intranet preko loopback test endpointa,
zatim stvarni saved DB -> source URI -> HTTP range -> FFmpeg decode u
memoriju, bez ffprobea ili pisanja na karticu. Ovo nije zavrsen player,
potvrda prikaza/sound-device izlaza niti live test A/V sinkronizacije.

## Implementirano

- `qnc-media-stream` je javna biblioteka s vlastitim manifestom, ne aplikacija
  niti playback engine. `MediaStream` ima ograniceni Read/Seek, a
  `server::respond` je storage-side HTTP handler. Ne pokrece proces, ne bira
  projekt/medij, ne poznaje UI, ne cita DB i ne dekodira.
- Normalne ovisnosti koriste postojeci source-reader, resolver i provjeru
  credentials iz JSON transporta. Read-only file handle otvara samo
  LocalSource unutar svoje capability granice. Postojeci source JSON wire
  ostaje nepromijenjen; nova Rust capability je `source.file.open_read_only`.
- Jedan Read vraca najvise 1 MiB; server struji zadani raspon. Nema cijelog
  klipa u memoriji, privremenog media fajla ni dodatnog kataloga.
- HEAD vraca duljinu bez tijela; GET podrzava cijeli tok ili jedan zatvoreni,
  otvoreni ili suffix byte range. Nevaljan range daje 416; promjena stamp-a
  daje 412; pogresan credential 401, drugi source 403, nepostojeci file 404.
  Izvan GET/HEAD nema operacija; nijedan credential ne daje pravo na pisanje.
- Klijent zahtijeva tocnu verziju, URI, stamp, Content-Length i Content-Range.
  Odbija kompresiju/chunked odgovor i nepotpuno tijelo prije predaje bajtova
  pozivatelju. Nema pomicanja remote cursora nakon pogresnog odgovora.
  Iskljucen je i ureq pool koji bi inace ponovno pokusao stale-connection GET.
- Runtime ovisnosti provjeravaju se Cargo grafom u conformanceu; stream ne
  moze ovisiti o aplikaciji, DB owneru, playeru, UI-ju, scanneru ili probeu.
  Source scanner dodatno odbija proces/DB/write operacije u aktivnom modulu;
  to nije potpuni semanticki dokaz nepostojanja svih mogucih zaobilaznica.

## Provjera 2026-09-08

Zavrsni ciljani prolaz: 172 testa u player/core/input, Ingest component/store,
work-settings, source-reader, JSON transportu, media-streamu i conformanceu.
Jos jedan test opt-in dijagnostickog primjera: ukupno 173, bez padova.
Novi modul: 14 unit testova + 1 example test. Clippy s `-D warnings`,
conformance i `git diff --check` prolaze.

Testovi pokrivaju Read/Seek/EOF/velicinu, promjenu izvora, read-only handle,
zabranu root/folder/traversal/drugog sourcea, auth i HTTP metode, pogresne
verzije/identitete/stamp/range/status/encoding/duljine, redirect i prekinuto
tijelo bez predaje nepotpunih bajtova. Windows junction test source-readera
prosiren je na novi file-handle API i prolazi. Unix symlink test je dodan,
ali nije izvrsen na ovom Windows racunalu.

Stvarni read-only test:

```text
cargo run -p qnc-media-stream --example decode_saved -- C:/Users/miron/Projects/QNC G:/ clip-0b16e5ca-0030-4ddb-9916-0dd3872c276b
```

`G:/` je izricita privatna owner lokacija za dijagnostiku, provjerena prema
postojecem source bindingu. Nije novi javni path niti consumer default.
Primjer preko SettingsReader/InputReader cita aktivni DB i postojecu
`proxy_if_available` odluku. Aktivni projekt: `novi-cjeloviti-1`.
Izabrani medij: `Mironik 1522S03.MP4`, proxy originalnog `Mironik 1522.MXF`.

Poslije citanja baze, FFmpeg dobiva samo autentificirani loopback media URL.
Container, codec, stream indeksi, dimenzije i vremenski podaci dolaze iz
spremljenog ulaza. Demuxer i decoder su eksplicitno zadani. Zavrsni poziv
ima `-nofind_stream_info`: nema heuristickog dopunjavanja stream podataka,
ffprobea, metadata zapisa ni probe fallbacka. Citanje demux zaglavlja i
dekodiranje odabranih paketa ostaju nuzni za izradu trazenih video/audio uzoraka.

Rezultat: 3 razlicita RGB framea (dva s pocetka, jedan nakon timestamp seeka
na 4.82 s), 38 400 PCM f32 bajtova za 0.1 s oba audio kanala, 48 kHz,
peak 0.0044737174. Ukupno 15 HTTP range zahtjeva. RGB frame SHA-256:

```text
9ea668dd54fd89d0e6ad938fa3c68b760991a6d91b5b242688f665d10a9dd3fb
43701f7ef8886105823f8f782cd975f30385edcb458f55840144b2ce049ffd99
e592094935cda127ed2e386fe111f36f26c827199f7396861530f2da3517e7b1
```

Hash procitanog medija jednak je prije/poslije. Za tu dijagnosticku provjeru
primjer dvaput sekvencijski cita medij u malim blokovima; to nije runtime
player korak. Globalni registar i projektna baza takodjer imaju iste SHA-256
prije i poslije. Nema zapisa na karticu ili u DB. FFmpeg procesi zavrsavaju,
stdout/stderr imaju memorijske granice i timeout; test host gasi svoje workere.

Zateceno i ispravljeno tijekom provjere: tiny_http je za veliki odgovor
automatski birao chunked prijenos, sto ne odgovara nasem ugovoru; prag je
postavljen tako da ostane Content-Length uz streaming. FFmpeg 8.1.1 s
ogranicenim `request_size` trazi `multiple_requests=1` za soft-seek preko
trajne veze; bez toga pokusava ponovno koristiti vezu uz `Connection: close`.
Primjer sada eksplicitno zadaje taj nacin, iskljucuje redirect/reconnect i
dodatni stream-info prolaz. Zavrsni rezultat gore vrijedi za taj ispravljeni poziv.

## Granice I Nastavak

Local i oba mreza URI okruzenja provjerena su preko stvarnih loopback HTTP
test endpointova. Nisu provjereni fizicki LAN/Intranet, TLS deployment,
Linux/macOS/ARM ni mrezne performanse. TLS termination, connection limiti,
binding izvora i lifecycle servera odgovornost su storage hosta. Ovaj korak
ne isporucuje zaseban trajni media-server executable niti mijenja Ingest manifest.

Dekodiranje je opt-in example, nije codec adapter u aktivnom modulu ni
produkcijski player. Diagnosticni FFmpeg direktni HTTP ulaz ne provjerava
sve QNC response headere kao MediaStream klijent; produkcijski adapter mora
zadrzati tu provjeru i ne moze example proglasiti gotovim transportnim playerom.
Stamp velicina/mtime nije hash i ne otkriva prepisivanje uz ocuvanje oba podatka.

Jos nema A/V clock/output sinkronizacije, zvucnog uredjaja, prikaza u Monitoru,
potvrde frame-preciznog seeka za unknown/variable timing ni live playbacka.
Originalna cetiri mono streama nisu dekodirana u ovom live dijagnostickom
prolazu; postojeci input testovi cuvaju njihovu mapu. Nema UI izmjena.

Sljedeci korak ostaje odvojeni codec/output adapter i samostalni Broadcast
Player proces nad spremljenim ulazom i ovim media pristupom. UI/timeline ne
preuzimaju runtime; Filmstrip i Wave ostaju zasebni generatori za kasniji korak.
