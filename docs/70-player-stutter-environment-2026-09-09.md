# Broadcast Player: zatajivanje Play-a i okruzenje

Datum: 2026-09-09. Radno stablo `C:\Users\miron\Projects\QNC`.
Ovo je analiza i prijedlog popravka. Kod, Project i baze nisu mijenjani.

## Opseg

Pregledan je cijeli playback put koji Ingest danas stvarno pokrece:
projektna baza (read-only) -> `qnc-work-settings` / `qnc-player-input` ->
`qnc-ingest-components` playback -> `qnc-player-client` -> child
`qnc-broadcast-player` (`qnc-player-runner`) -> `qnc-player-runtime` ->
decode / convert / audio / engine -> HTTP monitor -> egui paint.

Uzeti su zivji zapisi iz `docs/62`, `docs/63` i `docs/67`, trenutni kod
enginea, audija, klijenta i Ingest forme. Nije ponovljen live Play u ovom
zahvatu. MLT test (`docs/68`) nije predlozak za zamjenu enginea.

## Sto korisnik vidi kao "zatajkuje"

Tri razlicita kvara izgledaju slicno u monitoru. Moraju se razlikovati
prije popravka.

1. **Tvrdi prekid.** Reprodukcija staje. Log ima `audio queue underrun`
   ili `due AV frame is not prepared`. Runtime zatim pauzira i javlja
   gresku. To nije sitni drop; audio red ne smije izmisljati uzorke.
2. **Vizualni trzaj uz nastavak zvuka.** Slika kasni, ponavlja se ili
   skače, a engine i dalje svira. Tipicno HTTP monitor i Ingest paint.
3. **Stalni pomak slike iza zvuka.** Medijan oko 75 ms na 50 fps, s
   skokovima preko 400 ms. Zvuk ide, slika "kaska" ili se cini da
   proklizava.

Zivji dokazi nisu jedan uzrok. Isti klip je jednom stao na frameu 25,
drugi put na ~2000 / ~6000, treci put dosao do kraja nakon ubrzanja
pixel-converta.

## Cijeli lanac (sto stvarno tice Play)

```
projektna baza (settings_json + ingest snapshot)
        |  read-only, bez Project aplikacije
        v
qnc-player-input  (playback.input, source FPS, streamovi, audio kanali)
        v
Ingest komponenta  (ne forma)
        |  sibling exe: qnc-broadcast-player pored qnc-ingest
        v
player-client  HTTP /v1/player  +  HTTP /v1/player/frame (RGBA)
        v
owner thread u child procesu
  ffmpeg decode (video + N audio streamova)
  conversion worker (YUV -> RGBA, preview <= 960x540)
  FrameClock / TransportEngine
  WASAPI audio queue  (sat kada audio postoji)
        v
Ingest egui: thumbnail do potvrdenog Playing; zatim slika iz HTTP-a
  request_repaint_after(16 ms) dok je player spojen
```

Sto ovaj lanac **ne** radi, i ne smije raditi kao "popravak":

- nema player probe / ffprobe
- nema UI sata, FPS fallbacka ni lokalnog playheada u formi ili timelineu
- nema Project cratea ni novih projektnih postavki
- helper se ne gradi kao Cargo ovisnost Ingest cratea

## Dokazani uzroci (rangirani)

### P0. Pixel convert premasuje frame budzet (50 fps = 20 ms)

`docs/67`, prvi prolaz Mironik 2679: stop na frameu 25, underrun,
`conversion_avg_us=376379` (oko 376 ms po slici). To je ~19 puta sporije
od budzeta. Debug/neoptimiziran `qnc-pixel-convert` ubija i audio red
jer owner tick ceka pripremljeni AV paket.

Nakon `opt-level=3` samo za `qnc-pixel-convert`, `yuv` i
`fast_image_resize` u `Cargo.toml` `[profile.dev.package.*]`, isti klip
je dosao do zadnjeg framea bez `PlaybackError`. To potvrduje da je
spor convert bio dovoljan za tvrdi prekid, ne da je put sada stabilan.

Ako se Ingest ili helper ponovo pokrene bez tih profila, ili stari
`qnc-broadcast-player.exe` ostane pored novog Ingest exe-a, P0 se vraca.

### P0. Audio underrun je namjerno fatalan

`qnc-audio-output` pri praznom redu stavlja `underrun` i prestaje
izmicljati uzorke. Runtime `tick` vidi `Status::Failed` i pauzira
cijeli player. Korisnik cuje prekid / "zatajivanje", ne tihi click.

To je ispravna granica ugovora (nema laznog zvuka). Nije ispravan
dozivljaj ako convert ili HTTP/tick kasne. Popravak je drzati red
punim, ne uciti queue da izmisli PCM.

`docs/62`: prvi stop ~frame 2024, tick 93 ms, convert na owner threadu
~11.5 ms; nakon workera i dalje prazan red ~frame 6188.
`docs/63`: nakon audio-sata i half-second prerolla jedan prolaz je
dosao do kraja, ali s 2-kanalnim proxy AAC, ne s 4 mono originala.

### P1. HTTP monitor nema presentation ack

Klijent svaki `poll` radi `POST` slike. State se osvjezava najvise
svakih 50 ms. Ingest trazi repaint svakih 16 ms. Na 50 fps to nije
isti ritam kao audio sat. Header `presented` ostaje prazan. Engine
objavljuje submit; monitor ne potvrduje da je slika stvarno nacrtana.

`docs/67` na uspjesnom punom prolazu: 3157 uzoraka, medijan
**+76.52 ms**, P95 **+105.26 ms**, raspon do **+442 ms**. Pozitivno =
UI slanje kasni za audio driver vremenom. To nije GPU vsync niti
zvucnik-ekran mjerenje, ali objasnjava zasto Play "trza" i kada
engine ne javlja gresku.

v4 salje frameove mmapom, ne punim HTTP RGBA tijelom. Novi put kopira
buffer kroz localhost HTTP u UI proces. To je predvidljiv izvor
kasnjenja i izostavljenih frameova pri opterecenju.

### P1. Sat je audio uredjaj; slika je pratilac

`Runtime::playback_tick` koristi `playback_position_ns()` s WASAPI
procjene kada audio postoji, inace `Instant` od otvaranja. To je
usklađeno s AGENTS 8.2. Problem je sto monitor i egui ne prate taj
sat: slika se vuce kad UI stigne, ne kad je frame due.

Fiksni 75 ms delay u UI-ju nije opravdan (`docs/62`). Ne dodavati ga.

### P2. Okruzenje pokretanja, ne samo engine

| Uvjet | Ucinak na Play |
| --- | --- |
| Sibling `qnc-broadcast-player.exe` pored Ingest exe | Stari helper = stari convert/audio put |
| `cargo run` / debug bez package opt-level | P0 se vraca |
| `data/ingest-transport.json` ili `QNC_INGEST_TRANSPORT_CONFIG` | Bez bindinga nema medija; to nije stutter, to je fail open |
| Windows ReadOnly na `qnc_project.db` / WAL | Select/upis; Play cita snapshot vec u memoriji/bootstrapu |
| 4 zasebna ffmpeg audio procesa + 1 video | CPU i I/O; 2 projektna izlaza ne smanjuju broj decode streamova |
| Preview 960x540 samo na monitor putu | Smanjuje convert; GPU blit u Ingestu i dalje nema |
| Ingest `request_repaint_after(16ms)` | UI radi i kad nema novog framea; trosi CPU s child procesom |

Project freeze i DB-first ovdje nisu uzrok zatajivanja. `playback.input`,
`audio.channels` i `audio.sample_rate` vec dolaze iz baze (`docs/67`).
Nedostajuci zapis mora ostati kontrolirana greska, ne lokalni default.

### P2. Play prije Ready i krivi helper

Klijent `send()` gleda `reply`, ne `ready()`. Rani Space daje engine
`NotReady`. To izgleda kao "ne krene" ili kratki fail, ne kao underrun
usred klipa. Ipak, UI mora blokirati Play dok `PlaybackReadinessChanged
{ ready: true }` nije stigao.

## Sto nije uzrok (ne trositi zahvat)

- Novi probe ili MLT u produkcijskom playeru.
- Project UI, nove projektne postavke, `player-output.json` (uklonjen).
- Forma kao vlasnik vremena ili timeline kao sat.
- Tihi stereo mix ili preimenovanje uzoraka kad projektni rate/kanali
  ne odgovaraju uredjaju.
- Kopiranje v4 `qnc-media-ffmpeg` monolita (probe + generatori).
- "Popravak" underruna izmicljanjem tisina ili restartom uredjaja bez
  nove pripreme.

## Prijedlog popravka (redoslijed)

Svaki korak je uski. Ne otvarati Project. Ne dirati UI layout. Nakon
svakog koraka: ciljani test + jedan live klip 50 fps, original audio
4 mono, proxy slika, mjera Play -> prvi stvarni izlaz odvojeno od
pripreme.

### Korak 0. Dijagnoza prije koda (obavezno)

Na sljedecem zatajivanju zabiljeziti jednu od tri klase iz pocetka
dokumenta i ove brojke iz child stderr-a:

- `conversion_avg_us`, `upload_avg_us`, `converted`
- audio `underrun` / `device failed`
- `AV_F` `transfer_us` i `control_us` ako je diagnostics ukljucen
- je li Ingest i helper iz istog `target/...` i istog vremena builda
- debug vs release / postoji li `[profile.dev.package.qnc-pixel-convert]`

Bez toga sljedeci korak gadja krivi sloj.

### Korak 1. Jedan build + jedan helper

Napraviti da se `qnc-broadcast-player` uvijek izgradi i kopira pored
Ingest executablea istim profilom (dev+opt convert ili release).
Ingest i dalje trazi sibling po imenu, bez Cargo ovisnosti na runner
crate. Stari exe odbaciti. To zatvara lazne "regresije" zbog krivog
binarija.

### Korak 2. Convert mora stati u budzet i na debug putu

Zadrzati package opt-level. Mjeriti `conversion_avg_us` kao gate:
na 50 fps prosjek mora biti << 20 ms, P95 ispod ~12 ms na preview
velicini. Ako debug i dalje puca, convert crate ostaje u release-like
profilu; ne vracati convert na owner thread.

Worker burst (sada 2) i `min_prebuffer` (monitor: pola sekunde, 25
frameova na 50 fps) ostaju. Povecanje buffere nije zamjena za spor
convert; smije se dirati samo ako mjerenje pokaze jitter isporuke,
ne CPU convert.

### Korak 3. Audio red ostaje pun; fatal ostaje fatal

Cilj: underrun se ne dogodi na zdravom lokalnom disku. Sredstva:

- refill vezan uz audio sat, ne uz HTTP poll
- predaja PCM-a ne smije cekati zavrsetak RGBA converta iste slike
  ako je audio paket vec spreman
- 4 mono decode smiju ostati odvojeni streamovi (ugovor); I/O i CPU
  rasporediti tako da audio worker ne stoji iza video converta

Ako underrun ipak nastupi: jasna greska i nova priprema, ne lazni
Playing. Opcionalno kasnije: jednokratni bounded recover samo nakon
eksplicitnog re-prerolla; to zahtijeva novi ugovor i test. Nije prvi
korak.

### Korak 4. Monitor delivery po audio satu, ne po UI timeru

Zamijeniti slijepi `request_repaint_after(16)` s okidacem kad stigne
novi potvrdeni frame ili transport event (vec postoji
`notify_on_player_change`). UI ne interpolira vrijeme.

Uvesti presentation ack u monitor ugovor: klijent javlja zadnji
nacrtani `sequence`. Engine ne trosi submit slot kao da je prikazan
ako ack kasni. Fiksni delay zabranjen.

Srednji rez: zadrzati HTTP, ali vuci frame samo kad se `sequence`
promijenio; ne kopirati isti RGBA svakih 16 ms.

Ciljani rez (paritet s v4 iskustvom, ne kopija koda): shared-memory
ili mmap slotovi za RGBA/preview, HTTP ostaje samo kontrola. Isti
ugovor Local/LAN/Intranet: lokalni mmap nije mrezni servis; mreza
ostaje eksplicitni transport.

### Korak 5. Play samo nad Ready; mjera Play -> prvi izlaz

Klijent/Ingest ne salju Play dok `ready` nije true. Logirati nanosekunde
od naredbe do prvog submitanog videa i prvog audibilnog callbacka,
odvojeno od vremena `open`. To je korisnicki kriterij 2026-09-08.

### Korak 6. Tek nakon stabilnog lokalnog puta

LAN/Intranet isti ugovor, drugi adapter. Timeline remote na vec
provjereni engine. GPU video output u Ingest monitoru nije prvi
popravak zatajivanja; HTTP/mmap jeste.

## Predlozeni redoslijed zahvata (kratko)

1. Dijagnoza klase kvara + build/helper usklađenost.
2. Helper uvijek iz istog profila pored Ingesta.
3. Convert budzet kao test gate; audio refill ne ceka sliku.
4. Monitor ack + vuci samo novi sequence; zatim mmap umjesto HTTP tijela.
5. Play gated na Ready; mjera prvog izlaza.

Ne implementirati sve odjednom. Prvi kodni zahvat nakon koraka 0 treba
biti 1+2 ako log pokaze spor convert ili stari exe; 3+4 ako convert
vec stane u budzet a slika i dalje trza.

## Nije provjereno u ovom zahvatu

- Novi live Play na ovom stroju (koriste se stari logovi).
- Fizicki pomak zvucnik vs panel.
- Drugi OS, stvarni LAN/Intranet medij.
- Sve camera sheme i VFR (`frame_rate_mode=unknown` na nekim klipovima).
- Import, filmstrip, wave, Story.

Broadcast Player se ne smatra zavrsenim dok Play na 50 fps proxy +
4-mono original ne prolazi bez underruna i bez vizualnog proklizavanja
iznad izmjerenog budzeta, uz potvrdeni Ready prije Playa.
