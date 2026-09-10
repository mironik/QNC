# Ponovni dubinski audit aktivnog v4 Broadcast Playera

Datum: 2026-09-09.
Vrsta: audit izvornog koda, bez implementacije i bez promjene pravila.

## 1. Opseg i identitet reference

- Stvarna referentna putanja: `C:\Users\miron\Projects\qnc_v4`.
- V4 HEAD: `e130aa749915718e78d5891aba22700aa4aa35bd`.
- V4 radno stablo NIJE cisto. Mijenjani su i player_remote, runner,
  media adapter te host resolver. Audit se odnosi na procitano radno stablo,
  ne samo na taj commit. Nisam mijenjao v4 niti njegove postojece izmjene.
- Novi QNC: `C:\Users\miron\Projects\QNC`, HEAD
  `875d981deceb4d2c5da6d1682a3a4a295523c30d` plus necommitani rad.
- Procitani su root AGENTS oba projekta i v4 player freeze pravilo.
- Predmet je aktivni playback put s njegovim Ingest ulazom, projektnim
  postavkama, resolverom, Source/Program adapterima, dekoderom, satom,
  audiom, transportom slike i monitorom. Ovo nije audit svakog nepovezanog
  Project/Story/export podsustava, niti sigurnosni audit cijelog v4 repozitorija.
- Arhiva `archive/orphan-broadcast-2026-08-01` nije produkcijska referenca.

Navodi `datoteka:linija` u odjeljcima 2-7 relativni su prema v4 rootu.
Poveznice vode na stvarne datoteke. Izvjestaj 60 ostavljen je neizmijenjen;
ovaj zapis daje dodatnu provjeru aktivnih funkcija i njihove granice.

## 2. Nalazi koje ne smijemo prenijeti iz v4

### V1 / P1: zaustavljen player nastavlja puniti playout memoriju

Potvrdjeno citanjem koda, bez mjerenja potrosnje RAM-a u ovoj fazi audita.

U `TransportEngine::tick`, kada nema sata, poziva se
`refill_from_current_position`. `refill_budget` vraca nulu za pun spremnik
samo kada je `playing == true`. Za Ready/Paused, cak i kada je `ahead >=
healthy`, izraz `healthy.saturating_sub(ahead).max(1).min(max_burst)` vraca 1.
`next_decode_frame` se nastavlja povecavati, a `PlayoutBuffer` su BTreeMap
kolekcije bez granice kapaciteta. U mirovanju nema prezentiranja koje bi
pozvalo `trim_before`. Put moze dekodirati i zadrzati ostatak klipa.

Granice FFmpeg cachea od 96 frameova / 256 MiB NE ogranicavaju ovaj drugi,
playerov spremnik. Tvrdnja da je cijeli v4 playback memorijski ogranicen
zato nije tocna. Novi kod vec ima zasebno pravilo ogranicene idle pripreme;
to se ne smije zamijeniti ovom v4 implementacijom.

Dokaz: [tick i refill_budget](C:/Users/miron/Projects/qnc_v4/qnc-broadcast-player/src/transport_engine.rs:350),
isti file: 596, 704, 837, 892, 902.

### V2 / P1: prvi audio stream nije cijela mapa kanala kamere

`FfmpegAudioStream::spawn` koristi `-map 0:a:0`, zatim `-ac` s brojem kanala
iz opisa klipa. Ako snimka ima vise zasebnih mono streamova, time se ne
ucitavaju svi streamovi. Promjena broja izlaznih kanala nije njihovo citanje.

`stereo_dual_mono_monitor_samples` potom zbraja neparne izvorne kanale lijevo,
parne desno i ogranicava amplitudu. To je workstation monitoring, ne dokaz
da je zapis na kartici stereo niti zamjena za izvornu mapu kanala.

Novi QNC mora zadrzati spremljene indekse i mapu svih potrebnih streamova.
Ne prenositi v4 `0:a:0` kao univerzalno pravilo.

Dokaz: [audio FFmpeg naredba](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:2296),
[monitor mapiranje](C:/Users/miron/Projects/qnc_v4/qnc-player-output/src/lib.rs:1100).

### V3 / P1: original i proxy dobivaju isti clip-level probe

Obje grane `resolve_playback_input_media` dolaze u `play_media_from_path`.
On uzima `playback_probe_meta(project_id, clip_id)`: jedan `probe_json` iz
`ingest_assets`, bez identiteta odabrane reprezentacije. Original i proxy
mogu imati razlicit raster, codec, kontejner i raspored streamova.

To je konkretan rizik pogresnog opisa odabrane datoteke, ne poziv novog
ffprobea. Ne dokazuje da je Mironik2002 u v4 pogresno opisan; za taj zakljucak
treba usporediti njegov stvarni zapis i odabranu reprezentaciju.

Dokaz: [izbor i opis medija](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/play.rs:92),
isti file: 153, 364. Novi zasebni original/proxy snapshot ne treba ukidati.

### V4 / P1, uvjetno: video bez audija ostaje bez tekuceg device sata

`build_process_source_session` bira audio sink i sat samo prema
`args.audio_device`, ne prema tome ima li izvor audio. Klijent po defaultu
trazi audio uredjaj. CPAL sat napreduje callbackovima, a stream se pokrece
tek kada postoje predani audio paketi. Video-only Source nema takve pakete.

Po ovom kodnom putu video-only ulaz uz `--audio-device` moze ostati na
pocetnom frameu: service radi, ali referentni tick ne napreduje. Ovo nije
reprodukcija na kartici niti objasnjenje zasto Mironik2002 s audiom staje.
Pri izdvajanju audio-master clock adaptera obvezno zadrzati valjan sat i za
video-only ulaz, bez izmisljanja audio streama u metapodacima.

Dokaz: `qnc-player-runner/src/main.rs:577,600,2181`,
`qnc-player-runtime/src/process_client.rs` (audio_device default true),
`qnc-player-output/src/lib.rs:321,378,650,676`.

### V5 / P2: Ready u v4 ne znaci danasnji strogi Ready ugovor

`activate_source_handle` objavljuje Ready nakon pripreme adaptera, ne nakon
potvrdjenog pocetnog AV spremnika. `play` odmah objavljuje Playing i postavlja
`play_pending_preroll`; stvarno punjenje i pokretanje sata dolaze u
`advance_play_preroll`. Postoji idle priprema, ali Play i dalje moze zateci
nespreman spremnik. Sam poziv Play ne dekodira, no prvi stvarni izlaz moze cekati.

Za novi QNC vrijedi AGENTS 8.2: priprema prije Ready, a Play pokrece spremni
izlaz. Stara semantika nije razlog za oslabljivanje tog pravila.

Dokaz: `qnc-broadcast-player/src/transport_engine.rs:251,485,642`.

### V6 / P2: nastavak nakon underruna nije dokaz neprekinutog audija

CPAL callback pri praznom redu upisuje tisinu. `accept_audio_packet` to
prijavljuje kao DecodeWarning i nastavlja; device error i puna queue su
greske. Engine pri nedostupnom frameu pokusava refill, a adapter moze cekati
trazeni payload. Zato v4 i novi player ne staju pod istim uvjetima.

Ne uklanjati prijavu greske u novom playeru samo da bi izgledao kao da radi.
Treba postici dovoljan stvarni dotok AV podataka i definirano ponasanje pri
gubitku izvora, ne skrivati praznine tisinom ili ponavljanjem slike.

Dokaz: `qnc-player-output/src/lib.rs:415,586`,
`qnc-broadcast-player/src/transport_engine.rs:377`,
`qnc-media-ffmpeg/src/lib.rs:1586,2231`.

### V7 / P2: monitor nije univerzalni broadcast izlaz

Workstation put koristi 8-bitni YUV420p i Rec.709. `video_format_from_probe`
postavlja Rec709, a shader pretpostavlja limited/broadcast range. Ne prenosi
cijeli izvorni color/depth ugovor za druge materijale. FramePresented se
generira pri prihvatu framea u runnerovom presenteru, prije stvarnog GPU
prikaza u app procesu. Latest-frame monitor moze preskociti medjuframeove.

To je pasivni preview, ne dokaz svakog fizicki prikazanog framea, SDI izlaza,
genlocka ili mjerene fizicke AV sinkronizacije. Za novi javni monitor
primijeniti spremljeni format/range/color opis, ne bezuvjetno Rec709/8-bit.

Dokaz: `qnc-app/src/player_remote.rs:1697`,
`qnc-player-output/src/lib.rs:30`, `qnc-player-workstation-monitor/src/lib.rs:41`,
`qnc-player-workstation-monitor/src/yuv420.wgsl:35`.

### V8 / P2: stare aplikacijske i lokalne veze nisu novi javni ugovor

- V4 app ima jedan PlaybackStack za vise radnih prikaza; to nije model
  DB-only veze izmedju novih samostalnih aplikacija.
- `QncBroadcastPlayer::pump` izravno dispatcha `PlayerRemote::open`.
  Source open grana sinhrono gradi procesnu sesiju i ceka odgovore; procesna
  izolacija dekodiranja sama po sebi ne jamci neblokirajuci UI pri otvaranju.
- Mmap frame transport je lokalni IPC. Podrska za URL medija nije isto sto i
  udaljeni player s prijenosom slike/zvuka na drugu radnu stanicu.
- Source/Program timeline adapteri imaju pending/static fallback projekcije.
  To nije dozvola da novi timeline posjeduje paralelni playback polozaj.
- V4 monitor prelazi s postera cim odgovara otvorena source sesija, ne nuzno
  tek nakon Play. Novo eksplicitno pravilo thumbnail-do-Playa ima prednost.

Dokaz: `qnc-app/src/qnc_broadcast_player.rs:299`,
`qnc-app/src/player_remote.rs:660,1375`,
`qnc-app/src/playback_stack.rs:213,251,268`, `qnc-app/src/ingest/mod.rs:688`,
`qnc-player-runtime/src/process_client.rs`, `qnc-player-frame-transport/src/lib.rs`.

## 3. Stvarni aktivni lanac

```text
Projektna baza: playback.input
Ingest baza: media putanje + spremljeni probe_json
             |
     host media gateway / PlaybackMediaResolverComponent
             |
     opis odabranog izvora + racionalni source timebase
             |
     PlaybackStack / QncBroadcastPlayer / PlayerRemote (klijent)
             |
     PlayerProcessClient -- JSONL naredbe i dogadjaji
             |
     qnc-player-runner --player-service-jsonl
             |
     PlayerRuntimeServiceHandle (vlastita nit + ReferenceClock)
             |
     BroadcastPlayerRuntime -> TransportEngine -> FrameClock
             |
       +-----+-----------------------+
       |                             |
 FFmpeg video pipe              FFmpeg audio pipe
 reader + cache                 reader + cache
       |                             |
       +---- player AV spremnik -----+
       |                             |
 presenter / monitor           CPAL audio queue -> uredjaj
       |                             |
 YUV latest-frame mmap         callback -> referentni sat
       |
 app: WorkstationMonitor -> WGPU Y/U/V teksture -> shader
```

Nije jedan ffmpeg poziv po prikazanom frameu. Video i audio imaju odvojene,
dugotrajnije FFmpeg procese/pipeove za kontinuirano citanje. Promjena izvora
ili udaljeni seek moze ih ponovno otvoriti; normalni slijed koristi iste.
Monitor/UI ne otkucava source frameove i ne pokrece decode.

Aktivnu kompoziciju potvrduju `qnc-app/Cargo.toml`,
`qnc-app/src/player_remote.rs:1375,1725`,
`qnc-player-runner/src/main.rs:347,577,786`.
`run_app.ps1:15` gradi runner s `audio-device`; sam default Cargo build
runnera tu opciju nema. Time je razdvojena predvidjena desktop konfiguracija
od headless/testne konfiguracije. Binarna podudarnost trenutno otvorenog v4
prozora s ovim stablom nije provjerena.

## 4. Projektne postavke i probe: procedura koju treba zadrzati

1. Odabir klipa mijenja preview identitet. Novi izvor prekida staru source
   reprodukciju; kasni resolver odgovor drugog klipa/projekta odbacuje se.
2. Komponenta salje `PlaybackInput` zahtjev media gatewayu. Ne radi novi probe.
3. Host ucitava `project_effective_settings`, zatim `playback.input`.
   `proxy` zahtijeva proxy; `original` bira original; `proxy_if_available`
   eksplicitno dopusta original kada proxy nije dostupan. To nije export FPS
   ili odluka UI-ja.
4. Medijski opis dolazi iz `ingest_assets.probe_json`. Timebase i trajanje
   moraju biti valjani; player ne smije izmisljati FPS.
5. Klijent prosljedjuje pripremljeni SourceRuntime i konkretan media input
   runneru. Player jezgra ne otvara projektne baze niti zna za formu.

Dokaz: `qnc-app/src/ingest_player.rs:154,197,225,243`,
`qnc-app/src/components/playback_media_resolver.rs:28,89`,
[projektna playback politika](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/play.rs:60),
isti file: 364,408; `qnc-media-ffmpeg/src/lib.rs:283`.

Otvaranje/demux medija unutar FFmpega radi dekodiranja nije zaseban poziv
ffprobea. To ipak nije opravdanje da se playbackom nadomjesta nedostajuci
DB zapis. Cijeli v4 `qnc-media-ffmpeg` sadrzi i druge media funkcije te se ne
smije prenijeti kao nova player ovisnost u cijelosti.

U novom QNC-u treba zadrzati javni read-only settings/input/resolver put,
bez povezivanja Ingesta s Projects aplikacijom ili njezinim privatnim storeom.

## 5. Buffere, sat i prikaz treba gledati odvojeno

| Sloj | V4 produkcijski put / default | Sto stvarno ogranicava |
| --- | --- | --- |
| Video reader prefetch | 24 framea | Citac pipea / priprema unaprijed |
| Video decode cache | 96 frameova i 256 MiB | Samo cache decode adaptera |
| Audio prefetch/cache | 32 / 192 paketa, cache 16 MiB | Audio decode adapter |
| Engine decode burst | 4 | Rad po ticku i catch-up |
| Engine healthy target | burst x 4 = 16 | Cilj, NE tvrdi idle limit; V1 |
| CPAL queue | 64 paketa | Predani audio paketi |
| CPAL callback | cilj 10 ms, prilagodjen uredjaju | Device callback |
| Clock lookahead | 2 callback buffera | Referentni callback tick |
| Service timeout | 2 ms | Budjenje service niti, nije FPS |
| Runner publish loop | timeout 5 ms | Objava posljednje monitor slike |
| Monitor transport | 2 mmap slota | Posljednja slika, ne playlist/cache |

Prefetch/cache vrijednosti mogu se promijeniti preko `hardware_profile`
vrijednosti koje `player_process_config` stvarno prosljedjuje runneru.
Broj 6 za source decode burst i neke playlist opcije iz app pomocnika su pod
`#[cfg(test)]`; nisu stvarna postavka procesa. `build_runtime` ne postavlja
drugi burst i koristi engine default 4.

Dokaz: `qnc-media-ffmpeg/src/lib.rs:25,380,1915`,
`qnc-app/src/player_remote.rs:1725,1765`,
`qnc-player-runner/src/main.rs:678,703,786`,
`qnc-player-runtime/src/service.rs:11,177`.

### Audio kao sat

Desktop s ukljucenim audio uredjajem injektira `AudioDeviceClockHandle` u
service. Tocan izvor `now_tick` je callback elapsed timestamp plus callback
lookahead. `consumed_sample_frames` postoji kao telemetrija, ali nije vrijednost
iz koje ova implementacija racuna `now_tick`. Bez audio-device grane koristi
se monotoni sat. Za video-only granu vrijedi nalaz V4.

FrameClock racuna slotove iz racionalnog timebasea. Ne uzima project/export
FPS i ne broji egui repaintove. Transport catch-up isporucuje dospjele
frameove redom, do burst granice po ticku.

Dokaz: `qnc-player-output/src/lib.rs:321,378`,
`qnc-player-runner/src/main.rs:65,2181`,
`qnc-broadcast-player/src/frame_clock.rs:77,174`.

### Pasivni GPU monitor

FFmpeg daje YUV420p. Runner objavljuje tu sliku, a javni workstation monitor
u app procesu prenosi Y/U/V ravnine u GPU teksture i izvodi konverziju u
shaderu. Teksture se ponovno koriste dok se raster ne promijeni.

Nema CPU YUV->RGBA konverzije cijelog monitora u playerovu ticku. Nema nove
JPEG datoteke za svaki playback frame. Mmap ima backing datoteku, ali
`publish` ne radi per-frame flush. Ovo NIJE zero-copy: postoje kopije payloadova.

Za 1920x1080: YUV420p je 3,110,400 B, RGBA8 je 8,294,400 B po slici.
Razlika je 2.67x samo u velicini payloada, bez racunanja dodatnih kopija.
To nije izmjeren mrezni bandwidth ili FPS rezultat.

Dokaz: [frame transport](C:/Users/miron/Projects/qnc_v4/qnc-player-frame-transport/src/lib.rs:115),
[GPU upload](C:/Users/miron/Projects/qnc_v4/qnc-player-workstation-monitor/src/lib.rs:254),
`qnc-player-workstation-monitor/src/yuv420.wgsl`.

## 6. Pause, seek, kraj i novi klip

- Pause uklanja engine clock/playout i zaustavlja audio izlaz kroz adapter,
  ali moze zadrzati decode cache/pipeove za nastavak.
- Seek/cue je frame zahtjev. Cache hit ne otvara novi FFmpeg. Mali pomak
  naprijed moze iscijediti postojeci pipe; veci skok/natrag ponovno otvara
  decode stream s input seekom i preciznim trim/select korakom.
- Video seek racuna preroll u frameovima, audio granice u uzorcima iz
  racionalnog source timebasea. UI ne upravlja GOP-om ili sample offsetom.
- Stop/promjena izvora prekida izlaz i decode. Audio koristi generation
  identitet pa callback ne smije pustiti stare pakete nakon promjene.
- V4 dopusta carrier na OUT vremenskoj granici. Video adapter taj zahtjev
  ogranicava na zadnji dekodirljivi frame i preoznacava payload. Ne prenositi
  to kao dokaz da postoji dodatni frame N u klipu od N frameova; novi
  ekskluzivni OUT ugovor treba ostati konzistentan.
- Lokalni process client pri normalnom zatvaranju salje Shutdown, ceka do
  roka i po potrebi prekida proces. To ne dokazuje da su sve greske tijekom
  samog spawna/handshakea pokrivene istom cleanup putanjom.

Dokaz: `qnc-broadcast-player/src/transport_engine.rs:280,431,442`,
`qnc-media-ffmpeg/src/lib.rs:634,728,854,2296`,
`qnc-player-output/src/lib.rs:692`, `qnc-player-runtime/src/process_client.rs`.

## 7. Story/Program nije drugi player

V4 `OpenProgram` ulazi u isti runner/service/TransportEngine, ali preko
`program_playlist` adaptera. On pretvara program frame u odgovarajuci source
frame, priprema naredni izvor i sastavlja audio busove. Timeline crta dobiveno
stanje i salje zahtjeve. Filmstrip i Wave nisu dio ovog tick/decode puta.

Adapter ima ogranicen lookahead izvora (default 120 frameova, jedan naredni
izvor; isti prijelaz moze traziti vise audio izvora). Aktualni
`program_timebase_from_items` odbija mijesane source timebaseove. Ne postoji
osnova tvrditi da stari player vec podrzava svaku mjesovitu playlistu.

Dokaz: `qnc-player-runner/src/main.rs:627`,
`qnc-player-runtime/src/program_playlist.rs:29,181,369,448,586,1025`.
Pri novoj organizaciji playlist builder mora ostati zaseban neutralan modul;
Ingest ne treba Story workflow da bi reproducirao jedan klip.

## 8. Usporedba s novim QNC-om i stvarnim zastojem

| Bitna tocka | V4 | Novi QNC, procitano stablo |
| --- | --- | --- |
| Playback sat | Audio callback clock u desktop audio grani | `epoch: Instant` u Runtime |
| Monitor konverzija | YUV ravnine -> app GPU shader | CPU Converter u decode/tick putu -> RGBA |
| Monitor payload | Lokalni latest-frame mmap | Binarni RGBA HTTP response |
| Engine burst/target | 4 / cilj 16 | 1 / minimum 8 |
| Nedostaje dospjeli AV | Refill koji moze cekati payload | Nonblocking refill; ako jos nije spremno, NotReady/pause |
| Audio underrun | Tisina + warning | Failed/error zaustavlja reprodukciju |
| Idle priprema | Neispravno nastavlja; V1 | Ogranicena priprema prije Ready |
| Original/proxy opis | Jedan clip probe | Odvojeni spremljeni opisi |
| Audio odabir | Prvi stream | Eksplicitni spremljeni stream indeksi |

Dokaz u NOVOM rootu: `crates/qnc-player-runtime/src/lib.rs:97,139,146,163,176,319`,
`crates/qnc-player-runtime/src/input.rs` (PREBUFFER_FRAMES),
`crates/qnc-broadcast-player/src/transport_engine.rs:558`,
`tools/qnc-player-runner/src/control.rs:147`.

Prethodni dijagnosticki pokus ove radne sesije nad stvarnim Mironik2002:

- odabran Proxy iz postojeceg javnog DB/input puta;
- 1920x1080, 50/1 fps, 10,194 frameova, dakle oko 203.88 sekundi;
- jedan 10-sekundni pokus je prosao, ali dulji 60-sekundni pokus NIJE;
- u jednom duljem pokusaju zaustavljanje je bilo kod framea 823, oko
  16.46 sekundi, s `due AV frame is not prepared; output paused`;
- tada je audio telemetrija imala `queued_frames=0`; 824 konvertirane slike,
  prosjecna CPU konverzija oko 8.46 ms. To nije mjerenje cijelog ticka niti
  fizicke sinkronizacije zvuka i monitora.

Klip tada nije dosegnuo kraj. Stvarni simptom je iscrpljen dotok pripremljenih
AV podataka, a ne dokaz neispravne kartice. Nije jos izolirano koliki udio
imaju decode, CPU konverzija/kopije, raspored rada i odvojeni satovi.
Promjene buffera ili kratki uspjesni test to ne dokazuju.

Eksperimentalne izmjene novog playera nastale prije ovog zahtjeva za audit
ostaju necommitane i nepotvrdjene. Tijekom ovog audita nisu nastavljene,
revertirane niti proglasene popravkom. Nisu prenesene u v4.

## 9. Predlozeni nastavak, tek nakon potvrde smjera

Ne graditi novi monolit niti novi playback sustav od pocetka. Zadrzati
postojece javne input/settings/transport/decode granice i jasan lifecycle.

1. Kao prvi ograniceni zahvat razdvojiti pripremu native video framea od
   pasivnog monitor prikaza: izbaci punu CPU YUV->RGBA obradu monitora iz
   playerova kriticnog puta. To pripada javnom output/monitor adapteru,
   uz opis stvarnog formata iz baze; ne Ingest formi. Local transport moze
   koristiti latest-frame memorijski adapter, ali to samo po sebi ne
   zatvara LAN/Intranet ugovor.
2. Audio referentni sat i kontinuirani bounded prefetch provjeriti na istom
   izvoru kao zasebne promjene. Cuvati video-only sat, stvarnu mapu mono
   streamova, ogranicen RAM i kontroliranu gresku, bez v4 idle buga i bez
   skrivanja underruna. Ne lijeciti prosjecni manjak propusnosti sve vecim
   bufferom.
3. Nakon svakog zahvata mjeriti pripremu odvojeno od Play->prvog AV izlaza,
   dotok/queue minimum kroz cijeli Mironik2002, zatim Pause/nastavak,
   seek/korak, kraj klipa i prekid prethodnog klipa na novu selekciju.
4. Tek nakon neprekinutog playera nastaviti pasivni timeline, Filmstrip i
   Wave. Generatori artefakata citaju bazu i ostaju odvojeni od playera.

Nema novih projektnih postavki, izmjena Project koda, novog probea, scana
ili pisanja po kartici. Prije prenosenja aktivnog v4 koda treba potvrditi
tocan modul i opseg, prema AGENTS 2/8.2/10.

## 10. Sto je i sto nije verificirano

Provjereno: aktivne Cargo veze; desktop build feature; source resolver i
projektna playback politika; persisted probe lookup; stvarni runner
argumenti; Source i Program kompozicija; service clock; engine refill i
preroll; kontinuirani FFmpeg pipeovi; audio mapiranje/device callback;
latest-frame transport; GPU monitor; client/UI i timeline granice; relevantne
razlike u novom playeru. Nalazi su izvedeni iz koda, ne iz broja unit testova.

Nije provjereno u ovom audit prolazu: nova live A/B reprodukcija v4 i QNC-a,
podudarnost otvorenog v4 bina s dirty stablom, native frame/audio capture,
fizicki AV offset, svi camera formati, LAN/Intranet player AV prijenos,
Linux/macOS ili ARM, cjeloviti cargo test/build oba stabla. Raniji pokus
Mironik2002 iz odjeljka 8 nije zamjena za te provjere.

Zakljucak: v4 pokazuje korisno razdvajanje playera i pasivnog monitora,
audio-device clock te kontinuirano citanje unaprijed. Ne dokazuje da je
cijeli stari kod bez gresaka ili sukladan novim pravilima. Sljedeci rizik
je proglasiti kratki Play uspjehom, a prije stabilnog dotoka frameova
nastaviti timeline/filmstrip ili prenijeti cijeli stari media paket.
