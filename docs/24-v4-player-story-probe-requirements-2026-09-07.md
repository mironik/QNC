# qnc_v4: probe podaci za Broadcast Player i Story Segment

Datum: 2026-09-07. Read-only audit stvarnog koda, ne testnih fixturea.
Referenca: `C:/Users/miron/Projects/qnc_v4`.
HEAD reference: `e130aa7`, ali stablo ima postojece necommitane promjene,
ukljucujuci player, probe i Story. Nalazi vrijede za procitano radno stablo,
ne za cisti commit. Postojece promjene nisu dirane.

Novi QNC root: `C:/Users/miron/Projects/QNC`. Nema izmjene runtime koda,
AGENTS, kataloga, poslovnih baza niti kartice. Nije pokrenut probe, aplikacija,
import ili test. Ovaj dokument nastavlja docs/22 i docs/23.

## Glavni odgovor

V4 Broadcast Player i Story Segment vec koriste spremljene probe podatke.
Potrebni ulaz nije samo codec i FPS: potreban je frame-precizan opis trajanja,
video field mode i potpuni audio format. Story ne treba zaseban Story probe.

To je postojece ponasanje koje treba sacuvati. Ipak, v4 nije potpuni referentni
media ugovor za sve kamere: neke podatke ne sprema, neke pretpostavlja, a
original i proxy nisu pravilno razdvojeni u playback probe lookupu.

## Stvarni put podataka

1. Workerov Media Probe job poziva `media_probe::probe_media_fast` i vraca
   `MediaProbeJobResult { probe }`.
   [Worker](C:/Users/miron/Projects/qnc_v4/qnc-worker/src/lib.rs:1784).
2. Ingest sprema serijalizirani **tipizirani MediaProbe**, ne sirovi puni
   ffprobe JSON, u `ingest_assets.probe_json` i root virtualni kadar.
   [Zapis rezultata](C:/Users/miron/Projects/qnc_v4/qnc-host/src/ingest/store.rs:638).
3. Media resolver cita taj zapis iz baze, validira ga i vraca `source_probe`.
   Nedostajuci snapshot je greska, ne playerov ffprobe.
   [DB lookup](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/play.rs:364).
4. Source player bridge i kanonski Story program builder iz njega stvaraju
   runtime video/audio/timebase opis. FFmpeg player priprema sesiju iz tog
   opisa, bez novog ffprobea.
   [Source bridge](C:/Users/miron/Projects/qnc_v4/qnc-app/src/player_bridge.rs:29),
   [Story program resolver](C:/Users/miron/Projects/qnc_v4/qnc-host/src/program_playlist.rs:167),
   [Video prepare](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:1565),
   [Audio prepare](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:2689).

FFmpeg pri samom dekodiranju mora otvoriti i demultipleksirati kontejner.
To nije novi Media Probe posao niti zapis novog probe rezultata. Nije
moguce izjednaciti zabranu dodatnog probea sa zabranom citanja medija za play.

## Polja koja postoje i stvarno se koriste

Kanonski v4 tip:
[MediaProbe](C:/Users/miron/Projects/qnc_v4/qnc-service-contracts/src/lib.rs:250).

| Polje | Uloga u Broadcast Player / Story putu | Obaveznost |
| --- | --- | --- |
| timebase.fps_num, fps_den | Frame seek, ritam playa, IN/OUT, granice, audio sample/frame racun | Valjan racionalni source rate, oba > 0 |
| duration_frames | Puna duljina izvora i granica zadnjeg framea; kontrola source rangea | > 0 za playable source |
| has_video | Odreduje postoji li video grana izvora | Eksplicitno, ne zakljuciti samo iz ekstenzije |
| width, height | Video buffer/format i skaliranje | > 0 ako has_video |
| scan_mode | Progressive, interlaced TFF ili BFF; field-aware decode | Ne smije Unknown ako has_video |
| has_audio | Odreduje postoji li audio grana | Eksplicitno |
| audio_channels | Broj izvornih kanala i provjera audio routinga | > 0 ako has_audio; 0 bez audija |
| audio_format.sample_rate_hz | Pretvorba frame pozicije u audio sampleove | > 0 ako has_audio |
| audio_format.channel_count | Diskretni audio kanali | Mora odgovarati audio_channels |
| audio_format.sample_format | Spremljeni PCM/source sample opis | Tipizirani audio ugovor; ne isto sto i codec kontejnera |
| duration_sec | Snapshot/prikaz i pomocno izvodjenje | Option; nije autoritet za montazni IN/OUT |
| frame_count | Broj frameova iz nb_frames, kad je poznat | Option; parser ga preferira za duration_frames |
| codec | Video codec, ili audio codec ako nema videa | Sprema se, ali minimalni player open ga ne validira niti prenosi u video format |
| field_order | Izvorni tekstualni field opis i dodatna DB projekcija | Cuvati; runtime prvenstveno koristi scan_mode |

Hostov `validate_playback_probe` provjerava timebase, duration_frames, video
format i dosljednost audio kanala. Daljnji `ProbedAudioFormat::validate`
provjerava i sample rate. Zato prolaz samo prvog host validatora nije dokaz
da ce cijeli player prihvatiti zapis.
[Host provjera](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/play.rs:408),
[Audio tip i validacija](C:/Users/miron/Projects/qnc_v4/qnc-service-contracts/src/audio.rs:38).

V4 audio sample tipovi su PcmS16, PcmS24, PcmS32 i PcmF32. Konacni runtime
AudioFormat prenosi sample rate i broj kanala; trenutni FFmpeg izlaz ide u
s16le. To su razlicite stvari od izvornog AAC/PCM codeca. Izlazni format
playera i projektni audio preset nisu dokaz stvarnog source formata.
[Runtime pretvorba](C:/Users/miron/Projects/qnc_v4/qnc-app/src/player_remote.rs:1708),
[Audio decode izlaz](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:2317).

## Sto Story Segment dodaje, a nije probe

Story cita source FPS/timebase iz spremljenih ingest podataka ili iz
virtualnog kadra. Kreiranje segmenta prenosi clip identitet, source range i
timebase; ne pribavlja nove podatke iz videa.
[Segment iz klipa](C:/Users/miron/Projects/qnc_v4/qnc-host/src/story/db.rs:1719),
[Segment iz virtualnog kadra](C:/Users/miron/Projects/qnc_v4/qnc-host/src/story/db.rs:1365),
[DB-only source timebase/FPS](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media_pool/ingest_db.rs:304).

Za reprodukciju segmenta i pokrivalice koriste se isti video/audio podaci
kao za izvorni clip. `FlatProgramSource` zahtijeva source_duration_frames,
source_range s timebaseom, opcionalni video format te audio format i
kanale za aktivne audio rute.
[Program source ugovor](C:/Users/miron/Projects/qnc_v4/qnc-service-contracts/src/program_playlist.rs:95),
[Sync pokrivalica](C:/Users/miron/Projects/qnc_v4/qnc-app/src/components/sync_cover_capture.rs:321).

Sljedeci podaci jesu potrebni Storyju, ali **ne nastaju probeom**:

- clip_id i lokacijski neovisna media referenca;
- virtual_shot_id / segment_id;
- korisnicki source_in i source_out frameovi;
- trajanje segmenta = OUT - IN, odvojeno od pune duljine originala;
- polozaj segmenta na programu i redoslijed;
- M markeri, marker-slotovi, pokrivalice i njihove veze;
- izbor/routing audio kanala, mute i druge montazne odluke.

Timecode je izvedeni prikaz frame pozicije u ovom putu. Kamera-zapisani
CreationDate, originalni LTC i camera identitet korisni su ingest metapodaci,
ali navedeni v4 player/segment open ne traze ih kao obavezne ulaze.
Ne pokretati ffprobe za svaki novi virtualni kadar ili rez.

Kanonski v4 playlist i Sync ogranicavaju source/program na jednaki racionalni
timebase. To je ogranicenje postojece montazne implementacije, ne dozvola
Ingestu da mijenja stvarni source FPS prema project/export FPS-u.
[Provjera timebasea](C:/Users/miron/Projects/qnc_v4/qnc-service-contracts/src/program_playlist.rs:168),
[Sync provjera](C:/Users/miron/Projects/qnc_v4/qnc-app/src/components/sync_cover_capture.rs:279).

## Sto v4 ne treba slijepo prenijeti

### 1. Isti probe za original i proxy

`play_media_from_path` dobiva odabrani path i kind, ali poziva
`playback_probe_meta` samo s clip_id. Query vraca jedan `probe_json`, bez
odabira original/proxy tehnickog opisa. Stoga opis ne mora odgovarati
konkretnom mediju koji se otvara.
[Resolver](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/play.rs:152).

Novi ugovor treba jedan logicki clip s posebnim original/proxy opisima,
vezanima uz pripadajuce media reference. Proxy nije drugi clip. Ovo je
bitno i za Sony primjer: original moze imati LPCM/4 kanala, proxy AAC/2.

### 2. Zbrajanje kanala nije dovoljan opis audio streamova

Aktivni MediaProbe worker zbraja kanale svih audio streamova uz isti rate i
sample format, najvise 8 kanala. Ali rezultat zadrzava samo zbroj, ne
stream index / mapu kanala. Player zatim koristi `-map 0:a:0`.
Cetiri mono streama zato nisu isto sto i jedan cetverokanalni stream; samo
`audio_channels=4` ne omogucuje playeru pristup svim snimljenim kanalima.
[Agregacija](C:/Users/miron/Projects/qnc_v4/qnc-worker/src/media_probe.rs:134),
[Odabir samo prvog streama](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:2321).

Postoji i drugi parser u `qnc-media-ffmpeg/proxy.rs`: uzima format prvog
audio streama i maksimalan broj kanala pojedinog streama. Nije isto sto i
aktivni worker parser. Ne odabrati ga kao zamjenu bez analize.
[Drugi parser](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/proxy.rs:500).

### 3. AAC proxy nije ispravno opisan PCM-only parserom

Worker trazi `sample_fmt`, ali `stream_sample_format` ga ne koristi; tumaci
samo PCM codec nazive i 16/24/32 bit polja. AAC s neprimjenjivom/nultom
PCM bit dubinom zavrsava bez podrzanog sample formata. To je ogranicenje
parsera, ne dokaz da AAC proxy nema audio niti razlog za ponavljanje probea.
[Audio parser](C:/Users/miron/Projects/qnc_v4/qnc-worker/src/media_probe.rs:201).

Treba razlikovati komprimirani codec, format dekodiranih sampleova i
eventualnu valjanu PCM bit dubinu. Ne izmisljati PCM24 za AAC.

### 4. Nedostajuca polja i implicitne pretpostavke

V4 MediaProbe nema container/format_name, pix_fmt, codec profile, color
primaries/transfer/matrix/range, sample aspect ratio, stream indekse,
per-stream time_base/start_time niti rotaciju. To se ne smije prikazati kao
popis polja koja v4 vec cita iz baze.

Player konverzija postavlja Rec709, a VideoFormat konstruktor square pixel
aspect. Audio prepare postavlja decode offset na nulu. Decode-profile kod
u runtimeu izvlaci container iz ekstenzije; mapiranje codec/pix_fmt/profile
iz probe vrijednosti postoji samo pod cfg(test).
[Rec709](C:/Users/miron/Projects/qnc_v4/qnc-app/src/player_remote.rs:1697),
[Pixel aspect](C:/Users/miron/Projects/qnc_v4/qnc-broadcast-player/src/model/av.rs:48),
[Nulti audio offset](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:2689),
[Runtime/test decode profile](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/lib.rs:527).

Za buduci potpuni ugovor treba zasebno dogovoriti cuvanje tih podataka,
posebno po streamu i po original/proxy reprezentaciji. To je prijedlog na
osnovu vidljivih rupa, ne tvrdnja da su svi ti podaci danas obavezni za v4.

### 5. Nezamjenjivi FPS i pouzdanost broja frameova

Aktivni parser bira avg_frame_rate, zatim r_frame_rate, a ako oba nedostaju
stavlja 1/1. Takav fallback ne smije glumiti probed video FPS. Audio-only
medij treba eksplicitnu semantiku, ne izmisljeni video FPS.

`duration_frames` dolazi iz nb_frames ili round(duration_sec * fps). Drugi
put je procjena, nije opce jamstvo frame-preciznosti, posebno za VFR i
razlicita trajanja audio/video streamova. Izvorni frame count iz kamerina
zapisa treba sacuvati i provjeriti njegovu semantiku/timebase; ne odbaciti ga
pa ga ponovno procjenjivati. Nedovoljno potvrden podatak nije spreman zapis.
[Parser](C:/Users/miron/Projects/qnc_v4/qnc-worker/src/media_probe.rs:66).

### 6. Postoji naknadni probe, ali nije u player/Story read putu

Host nakon pokretanja pokrece metadata maintenance. On prolazi uvezene
clipove i poziva `resolve_clip_fps`, koji moze izvrsiti probe prije nego
upotrijebi postojece FPS podatke. To je odvojeno od DB-only
`resolve_stored_clip_fps` koji koristi Story.
[Boot maintenance](C:/Users/miron/Projects/qnc_v4/qnc-host/src/main.rs:132),
[Ponavljanje probea](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media_pool/ingest_db.rs:397).

Novi QNC to ne smije prenijeti: ffprobe samo gdje nedostaju potrebni podaci
tijekom jedinog Select/Ingest prolaza; nikad boot repair, player ili Story.

### 7. Nedostatak audija nije razlog za izmisljanje audio metapodataka

Source `build_open_request` koristi eager `then_some` oko izraza koji vec
vraca gresku ako nema audio_format. Zato i has_audio=false/None moze zavrsiti
greskom. Kanonski program resolver koristi ispravan uvjetni if.
To je kodni kvar Source bridgea, ne zahtjev da klip bez audija mora imati
audio probe. Auditom nije mijenjan niti live reproduciran.
[Source bridge](C:/Users/miron/Projects/qnc_v4/qnc-app/src/player_bridge.rs:55),
[Program grana](C:/Users/miron/Projects/qnc_v4/qnc-host/src/program_playlist.rs:206).

## Zakljucak za novi Ingest

Prvo preuzeti potpuni upotrebljivi zapis kamere ili ranije spremljeni
odgovarajuci zapis. Kriterij potpunosti mora biti konkretni media ugovor iz
gornje tablice, ne samo postojanje XML-a ili polja FPS/codec. Ako nedostaje
potreban podatak, ffprobe se jednom izvrsava za taj medij u Select/Ingest
fazi i rezultat se trajno sprema. Ne probeati sve zato sto neki zapisi nisu
potpuni, niti ponavljati probe u kasnijim aplikacijama.

Zadrzati postojece dobre rutine/ugovore gdje odgovaraju novoj granici.
Ne kopirati cijeli v4 host, ProjectDbBroker, repair workflow ili ogranicene
parsere kao novi monolit. Public media opis i njegova provjera moraju biti
u uskom modulu, izvan pasivne forme.

Provjeren je staticki tok od worker parsera do DB zapisa, media resolvera,
Source playa, Story segmenta/virtualnog kadra, kanonskog programa, Sync
pokrivalice i FFmpeg video/audio prepare/decode ulaza. Opcionalni test-only
builderi nisu tretirani kao produkcijski put. Nije provjeren cijeli export,
svaki Story UI element, dekodiranje stvarnog medija, mrezni rad ili drugi OS.
Nije utvrdjeno izvrsava li se svaki opisani kvar na konkretnom korisnickom
klipu; navedene slabosti potkrijepljene su kodnim putovima.
