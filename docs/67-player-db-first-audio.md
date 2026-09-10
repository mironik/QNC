# Broadcast Player: Ingest cita postavke iz baze

Datum: 2026-09-09. Radno stablo `C:\Users\miron\Projects\QNC`.

## Granica

Projects i shell ne prenose postavke Ingestu. Ingest ih sam cita iz baze
aktivnog projekta kroz javni read-only `qnc-work-settings` / `qnc-player-input`
put. Player dobiva pripremljeni modulni ulaz, ne poslovnu poruku druge
aplikacije. Forma ne cita bazu, ne odlucuje o formatu i ne izvrsava playback.

## Izmjena

- `PreparedInput` 0.3.0 zahtijeva `project_audio` procitan iz postojecih
  `audio.channels` i `audio.sample_rate`. Nema defaulta za nedostajuci zapis.
- Source FPS, PTS, trajanje i inventar svih izvornih kanala ostaju iz
  spremljenog medijskog zapisa. Projektni FPS nije source sat.
- Source preview cuva redoslijed kanala do projektnog broja izlaza. Nema
  zbrajanja kanala ni izmjene zapisa o originalu. A1/A2 uloge montaze ne
  zakljucuju se iz broja kanala snimke.
- Uklonjeni su `data/player-output.json`, njegov loader, capability i
  odvojeni `device_channels` bootstrap/Launch override. Runtime koristi
  audio konfiguraciju izvedenu iz DB-read ulaza.
- Postojeci lokalni i mrezni read ugovori ostaju isti. Nema Project cratea,
  nove baze, novog probea, scana ili UI/layout promjene u ovom zahvatu.

## Provjera

- 85 ciljanih testova proslo; 2 eksplicitna device testa preskocena.
- Nakon optimizacije proslo je i svih 15 `qnc-pixel-convert` testova.
- Testovi koriste stvarne SQLite javne prikaze: promjena owner's zapisa u
  testnoj bazi mijenja sljedece citanje; citac ne mijenja DB datoteku.
- Local, LAN i Intranet reader testovi potvrduju ista audio polja. Mrezni
  testovi su loopback provjera protokola, ne potvrda stvarnog LAN rasporeda.
- `qnc-conformance`: all checks passed.
- `cargo check` za player client/runtime/runner primjere prolazi. Primjeri
  nisu koristeni kao zamjena za live Ingest test.
- Standalone Ingest i Broadcast Player izgradjeni su iz ovog stabla.

## Live Ingest

Korisnik je odabrao duzi klip Mironik 2679 i odobrio preuzimanje Ingest prozora.
Klip je otvoren iz vec spremljenog kataloga, bez ponovnog Selecta ili probea.
Player log potvrdjuje `native_channels=4 project_channels=2 project_rate=48000`.
Thumbnail je ostao u monitoru do Playa; Space iz postojeceg kataloga pokrenuo
je video. Spremljeno trajanje je 11178 frameova na 50 fps (223.56 sekundi).

Prvi prolaz NIJE prosao: zaustavio se na frameu 25 uz audio underrun.
Log pokazuje `conversion_avg_us=376379`, odnosno obradu slike sporiju od
20 ms frame budzeta. Prvi log je `target/ingest-db-audio-live.stderr.log`.
To nije dokaz da DB citanje ne radi niti dokaz ispravne duge reprodukcije.

Ukljucena je dev opt-level=3 samo za postojeci `qnc-pixel-convert`, `yuv`
i `fast_image_resize`. Nema promjene algoritma, dekodera ili arhitekture.
Oba stvarna executablea ponovo su izgradjena prije ponovljenog testa.

Ponovljeni prolaz kroz Ingest stigao je do `carrier=11177`, zadnjeg framea,
nakon cijelih 223.56 sekundi. Status je zatim `Paused`; u optimiziranom logu
nema `PlaybackError`. Tijekom reprodukcije audio red ostaje popunjen.
Dokaz je `target/ingest-db-audio-optimized.stderr.log` i promjena stvarne
slike u Ingest monitoru. To potvrduje zavrsetak playera, ne prikaz svakog
pojedinog framea na fizickom monitoru.

Na Mironik 2676 provjereni su Play, Pause (carrier ostaje 925), nastavak
Playa i odabir Mironik 2679 dok stari klip svira. Novi klip ostaje na
thumbnailu i `Ready`, carrier 0, bez prenesenog Playa. Koristena je postojeca
Space akcija iz keyboard kataloga. Nije ponovljen Select, scan ili probe.

## Izmjereno kasnjenje prikaza

Prva puna optimizirana sesija Mironik 2679:
`322e40e7-9f5f-473d-8e4f-8dd2488f7264`.

Prvi `AV_V` zapis po sequenceu povezan je s frameom iz `AV_F` i najblizim
`AV_A` driver timestampom iste sesije. Prva sekunda izostavljena je iz
statistike. Delta je `paint_time - audio_time - (frame/50 - sample/48000)`.
Ponavljanja istog framea ne racunaju se kao nova prezentacija. Koristene su
samo potpune parsirane linije; stderr vise procesa povremeno ispreplete zapis.

- 3157 uparenih uzoraka: medijan +76.52 ms, P95 +105.26 ms.
- Raspon +39.78 do +441.98 ms. Pozitivno znaci da CPU slanje UI slike kasni
  za odgovarajucim medijskim vremenom audio drivera.
- Ovo NIJE GPU/display presentation timestamp niti fizicki izmjeren pomak
  zvuka iz zvucnika i slike na ekranu. Ne predstavlja potvrdu A/V sinkronizacije.
- Puni prolaz vise ne staje zbog prethodnog sporog pixel-convert puta, ali
  latencija UI monitor puta ostaje otvoren playback nalaz. Sljedeci uski
  zahvat treba mjeriti prijenos, upload i prezentaciju, bez izmjene izvornog
  FPS-a, dodavanja UI sata, nasumicnog audio delaya ili novog probea.

## Preostalo

- Ovaj audio adapter radi na native sample rateu. Ako se projektni rate
  razlikuje, priprema vraca eksplicitnu gresku potrebne pretvorbe. Ne mijenja
  oznaku source sample ratea niti tiho zanemaruje projektnu postavku.
- Manjak trazenih izvornih kanala ili nepodrzan fizicki izlaz daje gresku,
  ne automatski downmix ili dupliciranje kanala.
- Fizicki A/V pomak, drugi OS-ovi i stvarni LAN/Intranet live rad nisu
  potvrdjeni ovim testovima. Broadcast Player se ne smatra zavrsenim.
