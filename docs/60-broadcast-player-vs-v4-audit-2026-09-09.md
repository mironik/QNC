# QNC - dubinski audit Broadcast Playera vs qnc_v4

Datum: 2026-09-09.
Predmet: javni Broadcast Player u novom QNC-u, usporedba s aktivnim v4 putem.
Novi root: `C:\Users\miron\Projects\QNC` (HEAD `875d981` + necommitani player).
V4: `C:\Users\miron\Projects\qnc_v4` (HEAD `e130aa7`, aktivni put iz
`docs/qnc-v4-broadcast-player-port-map.md`).
Arhivirani `orphan-broadcast-2026-08-01` nije referenca.

Prethodni zapisi: docs/24 (v4 probe), docs/44-57 (novi player koraci),
docs/59 (sustav). Ovaj audit ne mijenja kod, AGENTS, Project ni baze.

## Zakljucak

Novi Broadcast Player je stvaran out-of-process modul, spojen na Ingest
monitor. To nije v4 kopija u jednom crateu. Jezgra je uska: ugovor, input iz
spremljenog snapshota, ffmpeg decode bez ffprobea, player-owned sat, HTTP
kontrola i RGBA monitor. Nije prenesen `qnc-media-ffmpeg` monolit s probeom
i generatorima.

Naspram v4, novi put je cistiji na granicama (probe zakon, original/proxy
opisi, mapa streamova, zabranjene ovisnosti). V4 je i dalje prednosti u
zrelosti izlaza: mmap frameovi, audio-device sat, program/playlist, hardware
decode politika, timeline kao daljinski i godine testova na stvarnom ritmu.

Novi player nije zavrsen proizvodni Broadcast Player u v4 smislu. Manifest
`runtime_available: true` pretjeruje naspram LAN AV, fizicke sinkronizacije
i timelinea. Ingest Play je spojen, ali helper bin nije Cargo ovisnost
Ingesta. Uvezi i Filmstrip/Wave ostaju izvan playera, sto je ispravno.

## Dva aktivna puta

```text
v4 (aktivno, ne arhiva)
  qnc-app/player_remote
    -> qnc-player-runner  (JSONL stdin/stdout)
    -> qnc-broadcast-player TransportEngine + FrameClock
    -> qnc-media-ffmpeg  (decode + filmstrip/wave/proxy/probe u istom paketu)
    -> mmap latest-frame  + opcionalni cpal

novi QNC
  Ingest forma (pasivni monitor)
    -> qnc-player-client  (prepare van UI threada)
    -> qnc-broadcast-player.exe  (stdin bootstrap JSON, ne JSONL)
         POST /v1/player
         POST /v1/player/frame  (binarni RGBA)
    -> qnc-media-decode (samo ffmpeg, -nofind_stream_info)
    -> audio/video adapteri u istom procesu
```

V4 `qnc-app/src/broadcast/**` je arhiva. Novi QNC tu arhivu nije vratio.

## Usporedba po temama

| Tema | qnc_v4 | Novi QNC | Ocjena |
| --- | --- | --- | --- |
| Proces | Child `qnc-player-runner`, JSONL | Child `qnc-broadcast-player`, HTTP JSON | Namjerna zamjena, ne regresija ugovora |
| Frameovi do UI | Lokalni mmap, UI ne koci sat | HTTP RGBA; spor UI moze kasniti | v4 bolji za ritam monitora |
| Sat | TransportEngine u runneru; audio samples kad je cpal | FrameClock u engineu; client 8 ms poll nije sat | Isti princip |
| Probe | DB `MediaProbe`; decode otvara kontejner | Spremljeni Final snapshot; ffmpeg `-nofind_stream_info` | Novi strozi; v4 crate i dalje sadrzi ffprobe helper |
| Original/proxy | `playback.input`; jedan `probe_json` po clip_id | `playback.input`; zaseban original i proxy u snapshotu | Novi ispravlja docs/24 rupu |
| Audio mapa | ffmpeg `-map 0:a:0` | `-map 0:{spremljeni index}` | Novi tocniji |
| Monitor audio | Dual-mono fold u `qnc-player-output` | Prva 1-2 nativna kanala, bez downmixa | Drukciji ugovor; v4 zreliji uredjaj |
| Play na Ready | Engine + app resolve | Engine `NotReady`; klijent salje Play cim ima hello | P2 u novom klijentu |
| Klik / Play slika | Poster dok `source_monitor_ready`; zatim video i bez Playa ako je cue spreman | Thumbnail do potvrdjenog Playing; Pause drzi video | Novi doslovnije prati AGENTS 8.2 |
| Promjena klipa | Novi Open nakon resolve | `stop_player` uvijek, i za nevaljan klip | Novi strozi |
| Timeline | Prikaz CarrierSync + CueFrame | Placeholder, bez intent | v4; novi namjerno kasnije (8.2) |
| Program/Story | `OpenProgram`, playlist | Ugovor postoji; proces odbija | v4 |
| Hardware decode | `hardware_profile` | Nema | v4 |
| LAN AV | ffmpeg HTTP URI sposobnost; IPC lokalni | `MediaBinding::Network`; kontrola loopback | Oba nepotpuna |
| Modulna cistoca | Jedan ffmpeg paket = decode+probe+generatori | Odvojeni decode/stream/output crateovi | Novi |
| Ingest spoj | Monolitna app | Javni klijent, bez Ingest workflow u playeru | Novi |

## Sto novi QNC radi bolje od v4

1. **Granica probea.** Player crateovi ne zovu ffprobe. Contract
   `forbidden_calls` to zabranjuje. Decode ne radi `find_stream_info`.
2. **Original i proxy nisu isti opis.** v4 docs/24: jedan `probe_json` za
   clip, pa proxy MXF moze lagati original. Novi `layout()` bira
   `snapshot.metadata.original` ili `.proxy` prema `playback.input`.
3. **Stream index iz baze**, ne pretpostavka `0:a:0`.
4. **Nema ffmpeg monolita.** AGENTS 8.2 izricito zabranjuje prijenos
   cijelog v4 `qnc-media-ffmpeg`.
5. **Sesija.** Novi klip gasi stari proces prije pripreme, cak i ako novi
   zapis nije spreman. Nova selekcija ne nasljedjuje Play.
6. **UI pravilo slike.** Thumbnail ostaje dok Play nije potvrden. v4 moze
   pokazati video cim je sesija ready nakon CueFrame, prije korisnickog
   Playa, ako je `source_monitor_ready`.

## Sto v4 i dalje ima, a novi nema

1. **mmap latest-frame.** Kontrolni protokol odvojen od piksela tako da spor
   UI ne koci sat. Novi HTTP frame je jednostavniji za LAN ugovor, ali slabiji
   kao lokalni monitor transport.
2. **Program/playlist i Story.** v4 `OpenProgram` je zivi put. Novi proces
   prima jedan immutable source descriptor.
3. **Timeline kao daljinski.** v4 crta playhead iz playera i salje CueFrame.
   Novi Ingest timeline je prazan pravokutnik.
4. **Audio master / uredjajska zrelost.** v4 cpal, 48 kHz stereo fold,
   sat od consumed samples. Novi: audio na hostu player procesa; remote
   audio uredjaj nije implementiran (docs/57).
5. **Hardware decode politika.**
6. **Isporuka bina uz app.** v4 `run_app.ps1` stavlja runner pored
   `qnc-app.exe`. Novi Ingest trazi sibling `qnc-broadcast-player`, ali
   `qnc-ingest` ne ovisi o `qnc-player-runner`.
7. **Godine boundary testova** (OUT prije pause, delayed tick, dual-mono).
   Novi ima ciljane testove i docs/55 native seek na jednom klipu; to nije
   v4 test istina.

## Ingest spoj (novi) vs v4 ingest monitor

Novi lanac: klik -> `INGEST_PREVIEW_FOCUS` -> `prepare_preview` (DB read +
spawn) -> poster. `PLAY_PAUSE` / Space ne ucitava bazu ponovo. Korak ±1 je
`CueFrame` od potvrdenog carriera. Proizvoljni cue s timelinea ne postoji.

v4: klik radi media resolve i default **CueFrame**; Play zove
`ensure_preview_playback_ready`. Monitor prelazi s postera na player sliku
kad je sesija spremna, ne nuzno tek na Play.

Oba citaju spremljene probe/snapshot podatke. Nijedan ne smije novi Media
Probe. Oba otvaraju medijsku datoteku za decode.

## Ulazni podaci (nastavak docs/24)

Novi `InputReader` zahtijeva Final fazu, `exact_frame_count`, spremljene
streamove i `playback.input` iz work-settings. Nedostatak je greska, ne
novi probe. Importirani URI mora odgovarati original ili proxy bindingu;
import worker i dalje ne postoji, pa se play veze na izvorni transport.

v4 `validate_playback_probe` trazi timebase, `duration_frames`, scan_mode,
audio format. Novi snapshot ugovor je siri (docs/25), ali player input
odbija nepotpun original ili proxy prefix. To je stroze od v4 lookupa
istog `probe_json` za oba filea.

## AGENTS 8.2 naspram koda

| Kriterij | Stanje u novom QNC |
| --- | --- |
| OOP javni modul, bez allowliste | Da |
| Sat i Ready u playeru | Da, u engineu |
| Play ne otvara medij/DB | Da u engineu; prepare vec otvorio proces |
| Thumbnail do Playa | Da |
| Prekid sesije na drugi klip | Da |
| Bez probea | Da |
| Isti ugovor Local/LAN/Intranet | Media URI da; kontrola/monitor loopback; remote audio ne |
| Timeline nakon playera | Jos nije; placeholder |
| `runtime_available` | Manifest `true`; LAN/fizicki sync nisu |

## Nalazi

### P1

- **Uvezi** i dalje stub. Player radi s kartice preko bindinga. To nije
  v4 import/queue.
- **Isporuka helper bina** uz Ingest/shell nije u Cargo grafu aplikacije.

### P2

- Klijent salje Play cim postoji `reply`, ne ceka `play_ready`. Engine
  odbije; to nije "Play odmah na spremnom" iz UI-ja ako korisnik pritisne
  rano.
- HTTP monitor vs v4 mmap: rizik kasnjenja slike, nije izmjeren Play->prvi
  frame za Ingest put.
- `runtime_available: true` ispred LAN AV i fizicke sinkronizacije.
- `qnc-player-input` ovisi o `qnc-ingest-store` (content ugovor). Nije app
  workflow, ali vezuje player input uz Ingest shemu.
- Seek u Ingest UI samo korak; timeline nije daljinski. Cue moze maknuti
  zadnju sliku pa se vidi thumbnail usred seeka.

### P3

- Nema program/playlist/Story.
- Nema hardware decode.
- Filmstrip/Wave nisu player posao i nisu implementirani.
- §16 i dalje kaze da playback nije implementiran.

## Granice koje drze

- Player ne pise poslovnu bazu.
- Nema Ingest/Player -> Project cratea.
- Nema `allowed_applications`.
- Decode nije Media Probe.
- Forma ne dekodira.

## Provjereno

- v4 port map i aktivni crateovi (ne arhiva).
- Novi playback.rs, player-input, media-decode plan, runner, ingest widgets.
- AGENTS 8.1/8.2 i docs/24, 55, 57.
- Original/proxy i stream map u novom inputu.
- Freeze: ovaj audit nije dirnuo Project.

## Nije provjereno

- Live A/B Play na istoj kartici v4 vs novi QNC u ovom prolazu.
- docs/55 native mjerenja nisu ponovljena.
- Fizicki LAN, Linux/macOS, cjeloviti workspace testovi.
- Pixel-doslovnost Ingest monitora naspram v4 `qnc_broadcast_player`.

## Sljedeci rizik

Tretirati HTTP monitor kao zamjenu za v4 mmap prije mjerenja Play->frame
vraca lag u Ingestu. Copirati v4 `qnc-media-ffmpeg` zbog "pariteta"
vraca probe/generatore u player. Ispravan red: isporuka bina, uskladiti
klijentski Ready s engineom, onda timeline kao pasivni remote, onda
Filmstrip/Wave iz baze. Program/Story tek kad postoji citljiv katalog i
isti player ugovor.
)
