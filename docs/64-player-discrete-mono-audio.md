# Player: izvorni mono kanali

Korisnicki zahtjev 2026-09-09: audio mora zadrzati 2 ili 4 odvojena mono
kanala izvornog zapisa, ne automatski stereo izlaz.

## Ugovor prije implementacije

- `playback.input` iz projektne baze i dalje bira original/proxy SLIKU.
- Javni Player Input opisuje audio iz originala: URI, spremljene stream
  indekse, kanale i timing. Proxy audio nije zamjena za originalne kanale.
- Kada slika dolazi iz proxyja, spremljeni original i proxy moraju imati
  podudaran frame rate i broj frameova. Audio PTS racuna se prema pocetku
  originalnog videa, video PTS prema pocetku odabranog videa.
- Nema dodatnog probea, izmjene baze ni novih projektnih postavki. Sve dolazi
  iz vec spremljenog original/proxy zapisa kroz javni DB/transport ugovor.
- Player otvara odvojene video/audio ulaze kroz isti javni media transport.
  Lokalna putanja nije dio javnog identiteta; Local/LAN/Intranet koriste
  isti opis i resolver. Ne mijenja se forma, keyboard ni Project kod.
- Audio Output dobiva sve izvorne kanale redom, 1:1, bez miksanja i bez
  odbacivanja CH3/CH4. Broj kanala nije uvijek cetiri: odredjuje ga zapis.
- Izricita mapa fizickih izlaza ostaje javna mogucnost audio modula.
  Ingest ne namece vlastiti par kanala. Bez mape vrijedi puni 1:1 izlaz.
- Ako fizicki uredaj ne podrzava taj broj kanala/sample rate, player javlja
  trazeni format i ogranicenje uredaja. Ne prelazi potiho na stereo.
  Cetiri diskretna fizicka izlaza nisu moguca na dvokanalnom uredaju bez
  zasebno dogovorenog izbora kanala za slusanje.

## Verifikacija

Potrebno provjeriti original 2/4 mono, proxy slika + original 4 mono,
podudaran timing, odbijanje nepotpunih zapisa bez probea, 1:1 audio mapu,
Local/LAN/Intranet descriptor i stvarni Ingest. Live rezultat i hardverska
ogranicenja zapisati nakon provjere; unit test nije dokaz fizickog izlaza.

## Implementirano i provjereno

- Player Input ugovor/crate je 0.2.0: odvojeni izbor slike i originalnog
  audija, puna native mapa streamova/kanala i zasebni spremljeni PTS origin.
- Ingest vise nema `.min(2)` izbor kanala. Runner bez izricite mape koristi
  javni `ChannelMap::identity`; dekoderi otvaraju originalni audio URI.
- `cargo build --release -p qnc-ingest -p qnc-player-runner`: uspjesno.
- Ciljani `cargo test --release` nad audio-output, broadcast-player,
  ingest-components, pixel-convert, player-client, player-input,
  player-runner, player-runtime i ui-kit: 173 prosla, 0 pala, 2 hardverska
  testa ignored. Ti ignored testovi nisu zamjena za Ingest live test.
- Testovi obuhvacaju dvije/cetiri odvojene mono adrese, impuls bez
  preslusavanja kanala, proxy sliku uz originalni audio, zasebne pocetne
  timestampove, nepoznat proxy frame-rate-mode bez izmjene zapisa i isti
  javni descriptor kroz Local/LAN/Intranet.
- Project freeze scope nema promjena. Nema novog probea ni DB upisa u
  playback putu.
- `cargo run --release -p qnc-conformance -- C:\Users\miron\Projects\QNC`:
  sve provjere prolaze, ukljucujuci javne player granice i verzije ugovora.

## Stvarni Ingest test i ogranicenje

Ponovno pokrenut novi `target/release/qnc-ingest.exe`; odabran Mironik 2002
iz ucitanih 98 klipova, bez Select scana/probea. Thumbnail se prikazao.
Spremljeni original ima cetiri mono PCM24 streama na 48000 Hz; spremljeni
proxy ima jedan dvokanalni AAC stream. Oba imaju 10194 video framea, 50 fps.

Default fizicki audio uredaj `Headphones` odbio je cetiri kanala. Stvarna
prijavljena podrska za f32 na 48000 Hz: `[2]`. Player je prijavio gresku i
nije pokrenuo lazni Ready/Playing niti automatski presao na stereo.
Log: `target/ingest-discrete-mono.stderr.log`.

Cetverokanalni fizicki izlaz i novi A/V pomak zato NISU live-verificirani.
Za nastavak treba cetverokanalni uredaj ili korisnikov izricit izbor para
za slusanje na dvokanalnom uredaju, uz ocuvanje sva cetiri originalna kanala
unutar playera. Izbor para nije pretpostavljen niti zapisan u Project bazu.
Stvarni LAN/Intranet i Linux/macOS audio uredaji nisu testirani u ovom prolazu.

## Odobren dvokanalni monitoring

Korisnik je zatim odobrio dva izlazna kanala. Za ovo racunalo bira se
CH1 -> izlaz 1, CH2 -> izlaz 2, bez miksanja; source ostaje cetiri mono.
Izbor je deployment postavka `data/player-output.json`, ne Project postavka
i ne promjena izvornog zapisa. Javni player-client cita i validira taj mali
zapis tijekom pripreme te salje postojeci `device_channels` bootstrap.
Bez datoteke ostaje native 1:1; nevaljan zapis nije stereo fallback.
Forma nema novi kod, tipke ni UI. Isti javni citac mogu koristiti drugi
klijenti za svoj player/audio uredaj neovisno o lokaciji medija.

Prvi live prolaz s parom CH1/CH2 otvorio je pravi audio uredaj i sve cetiri
originalne mono dekoderske trake, ali stao je na frameu 847 zbog audio
underruna. Zapis: `target/ingest-mono-pair.stderr.log`.

Sljedeca ogranicena izmjena javnog decode adaptera: samo za spremljeni MXF
container `short_seek_size` jednak je vec postojecem HTTP read prozoru
(1 MiB). Time mali preskoci izmedju interleaved audio/video paketa koriste
read-ahead umjesto novih Range zahtjeva. MP4 workaround iz docs/48 ostaje.
Nema promjene kanala, probea, memorijskih granica ili source identiteta.
Ovo je hipoteza za live provjeru, ne unaprijed potvrdeno uklanjanje zastoja.
Primarni izvor: [FFmpeg HTTP protocol](https://ffmpeg.org/ffmpeg-protocols.html#http).
