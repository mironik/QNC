# Javni Media Probe i objedinjavanje metapodataka

Datum: 2026-09-07. Odobren nastavak docs/31, bez UI/Select integracije.

Nastavak u docs/33 razrjesava RTMD/odsutnu rotaciju iz ovog nalaza i dodaje
trajnu DB rezervaciju pokusaja. Ovaj dokument cuva rezultate prvog prolaza;
ne opisuje naknadno ponovno izvrsavanje ffprobea.

## Granice prije implementacije

- `qnc-media-probe`: jedan ffprobe child proces za jedan eksplicitno vezan
  medij; lokalni owner binding ili HTTPS/JSON zahtjev pomocniku uz medij.
  Nema skeniranja, app ovisnosti, DB pisanja, automatskog retrya niti mergea.
- `qnc-ffprobe-metadata`: cista pretvorba punog JSON izvjestaja u tehnicke
  factove. Nema I/O, izvrsvanja procesa, default FPS/boja/audio formata.
- `qnc-media-metadata-compose`: cista odluka o nepotpunim reprezentacijama
  i objedinjavanje camera/probe factova. Nema I/O ni pozivanja probea.
  Original i opcionalni proxy ostaju isti logicki clip.
- Postojeci `qnc-media-record-db` ostaje jedini writer ovog zapisa. Ne dodaje
  se scanner, parser ili probe u DB adapter. Kamera snapshot ostaje u povijesti.

Javni input sadrzi QNC URI, ne OS path. Pomocnik dobiva privatne tocne
URI->datoteka bindinge od storage ownera; nema implicitnog path fallbacka.
LAN/Intranet pomocnik nije Ingest aplikacija ni aplikacijski servis: isti
javni modul moze raditi u bilo kojem hostu, bez popisa dozvoljenih aplikacija.
Pristup se autorizira transport credentialom i eksplicitnim bindingom medija.
Nema prijenosa cijelog videa u JSON ili kopiranja u privremeni direktorij.

Puni format/stream/program/chapter JSON dolazi iz jednog poziva. Zavrsni CLI
ukljucuje i codec extradata (`show_data`) te verzije programa/biblioteka.
Limite unaprijed postavlja privatna owner konfiguracija, ne javni zahtjev.
Pocetno je provjeren v4 profil: probesize 1048576, analyzeduration 100000 us.
On je na prvom stvarnom Sony proxyju ostavio pixel format i SAR nepoznatim.
Nema automatskog drugog poziva. Na DRUGIM klipovima provjeren je veci prozor
od 1000000 us, najprije 8 MiB pa 1 MiB; zavrsni live profil je 1 MiB/1 s.
Vrijednosti su konfiguracija unaprijed, ne retry/fallback pravilo po rezultatu.
Nema count_frames, show_frames, show_packets niti drugog poziva s vecim
limitom. Puni JSON nije jamstvo da kamera/demuxer deklarira svaki podatak.
Nedostaci se cuvaju kao nedostaci, ne kao dozvola za kasniji probe.
Izlaz je ogranicen u RAM-u, child ima timeout i mora biti reaped.
`format.filename` normalizira se u javni media URI prije objave/spremanja;
ostala JSON polja cuvaju se. To je transport-normalizirani izvjestaj,
ne tvrdnja o byte-identicnoj kopiji privatnog stdouta.

Samo owner Select komponenta smije pokrenuti jedini prolaz i trajno
zabiljeziti zavrsetak. Statelesni javni pomocnik sam ne posjeduje DB ledger
niti jamci exactly-once nakon pada hosta. Mrezni timeout je neizvjestan
ishod i ne smije izazvati ponavljanje. Runtime integracija i trajno
upravljanje pocetkom/zavrsetkom prolaza nisu dio ovog koraka.

Parser cuva nb_frames kao deklarirani exact count; trajanje*FPS samo kao
estimated. Jednak avg_frame_rate/r_frame_rate ne dokazuje CFR; bez
kamerina dokaza frame_rate_mode ostaje Unknown. Ne pretvara procjenu u exact.
Nedeklarirana boja/channel layout moze biti Signal::Unspecified samo kad
izvjestaj eksplicitno nosi takvu vrijednost. Nepostojeca rotacija nije nula.
Streamovi se spajaju po indeksu, a nepoznati indeks samo kad postoji tocno
jedan kandidat iste vrste na obje strane. Dvosmislenost se odbija.

Provjera: ciljani testovi, workspace/conformance i read-only Sony kartica.
Loopback s LAN/Intranet URI-jima nije dokaz stvarne udaljene mreze, TLS
deploya ili Linux/macOS/ARM izvrsavanja. UI nije mijenjan.

Izvor CLI semantike: https://ffmpeg.org/ffprobe.html (provjeren 2026-09-07).

## Otkrivena semanticka greska Sony parsera

`dur` i `Duration`/edit-unit count opisuju VIDEO, ne trajanje kontejnera.
Prvi arhivirani primjer: proxy video 9260/50 = 185.2 s, MP4 kontejner
185.216 s zbog audio paddinga. Prijasnji Sony parser pogresno je promovirao
video trajanje u `MediaRepresentation.duration_seconds`.
Parser sada cuva exact video frame count, rate i izvorne XML factove; ne
popunjava trajanje kontejnera iz video broja frameova. Kontejnersko trajanje
uzima se iz jedinog ffprobe izvjestaja. Nema tolerancije koja skriva konflikt.
Popravak je potvrden ponovnim parsiranjem arhiviranog XML/JSON, bez probea.

## Jedan prolaz i paralelizam

`execute_batch`: 1-16 radnika, najvise 256 zahtjeva, isti URI/request/document
ne smije se ponoviti u batchu. Svaki valjan zahtjev izvrsava se jednom;
ishod ostaje u njegovoj poziciji. Greska jednog ne ponavlja ostale.
DB transakcije nisu otvorene tijekom izvrsavanja. Primjer koristi 8 radnika.
Server host je odgovoran za ukupni broj konkurentnih klijenata i request-body
timeout na gatewayu; bibliotecni handler ne stvara rezidentni app servis.
`required_probes` prima provjereni DB Snapshot i odbija `Final`, ukljucujuci
`Final/Partial`. To nije globalni crash-safe ledger niti dozvola za retry.

## Live nalaz i otvorena granica

Kartica G: koristena samo read-only. Ukupno 103 originala + 103 proxyja,
svaka fizicka reprezentacija pozvana jednom u ovom koraku. Testni skupovi
su nepoklapajuci rasponi MEDIAPRO indeksa; nisu razliciti snimljeni materijali
za usporedbu brzine, pa sljedeca vremena nisu strogi A/B benchmark.

| Skup | Profil | Broj clipova / poziva | Probe batch | Cijeli primjer |
| --- | --- | --- | --- | --- |
| local, indeks 0 | 1 MiB / 0.1 s, serijski | 1 / 2 | zbroj 461 ms | 781 ms |
| local, indeks 1 | 8 MiB / 1 s, serijski | 1 / 2 | zbroj 652 ms | 1015 ms |
| local, indeksi 2-34 | 8 MiB / 1 s, 8 radnika | 33 / 66 | 10053 ms | 14254 ms |
| LAN loopback, 35-68 | 1 MiB / 1 s, 8 radnika | 34 / 68 | 3803 ms | 10721 ms |
| intranet loopback, 69-102 | 1 MiB / 1 s, 8 radnika | 34 / 68 | 6120 ms | 14847 ms |

Cijeli primjer ukljucuje source-index zapis, XML, camera/final DB upise,
idempotency provjeru, JSON arhivu i readback. Intranet skup prvi ukljucuje
`show_data` i verzije. Nije dokaz prijasnje brzine ~3 s za cijelu karticu.
Svih 103 zapisa zadrzava original, vezani proxy i datum kreiranja.
Prvi zapis finaliziran je u NOVOJ testnoj bazi iz arhive, bez novog probea.
Ostali skupovi finalizirani su i procitani nakon gasenja testnog hosta.

Zavrsni zapisi jos su `Final/Partial`, ne lazno `Complete`:

- original/proxy nema deklariranu rotaciju u izvjestaju;
- proxy ima dodatni Sony `data` stream s oznakom `rtmd`, a ffprobe ne daje
  poznati `codec_name`. Izvorni tag i ostali podaci su u arhiviranom JSON-u;
- samo prvi, stari brzi profil ima i neocitan pixel format/SAR proxyja.

Parser ne stavlja rotaciju 0 niti izmislja rtmd codec radi zelenog testa.
Prije UI integracije treba dogovoriti semantiku odsutnog transform podatka i
nepoznatog ancillary codeca u media ugovoru. To nije razlog za novi ffprobe.
Ne tvrdi se da su sada svi zapisi spremni za player/filmstrip.

`sony_probe` je testni caller, ne produkcijski Select. `replay_saved` cita
samo spremljeni XML/JSON u novu privremenu provjernu bazu; ne izvrsava probe,
ne mijenja postojecu bazu, ne migrira i ne cita karticu.
Arhive su u ispisanim privatnim privremenim direktorijima, ne na kartici.
UI, Ingest runtime, Project, shell i application manifesti nisu mijenjani.
