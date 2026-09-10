# MLT: granica DB-first / bez dodatne analize

Datum: 2026-09-09. Nastavak docs/68, isto QNC radno stablo.

## Rezultat

Standardni MLT `avformat` producer nije prihvatljiv za QNC no-probe pravilo.
To vise nije samo nalaz iz izvornog koda: potvrdjeno je pracenjem stvarne
MLT 7.41.0 / FFmpeg biblioteke iz sluzbenog Shotcut 26.8.1 paketa.

`avformat-novalidate` izbjegava analizu u konstruktoru, ali je radi pri prvom
citanju. Predavanje spremljenog FPS-a, dimenzija i trajanja to ne iskljucuje.
Ne postoji dokaz da bi spremanje jos jednog projektnog polja ovo popravilo;
problem je implementacija media ulaza, ne Projects i ne nedostatak postavki.

Druga prepreka iz docs/68 ima provjereno rjesenje: postavljanje MLT `length`
i `out` iz DB video raspona zadrzava tocan broj frameova. Ovo je test javnih
MLT properties, nije promjena QNC playera ili baze.

## Stvarni test

Alat: `tools/diagnostics/mlt-probe-boundary.py`.
Ulazi se citaju pomocu postojeceg izoliranog testnog citaca iz docs/68.
Aktivni projekt: bbvvcx, proxy_if_available, 2 izlazna kanala / 48000 Hz.
Mironik 2002 i Mironik 2679: proxy slika, originalni zvuk, kartica read-only.

Pracenje ide kroz javni FFmpeg `av_log_set_callback`, AV_LOG_DEBUG. Broje se
izvorne Before/After oznake funkcije `avformat_find_stream_info`. Nema izmjena
DLL-a, hookanja izvrsnog koda, zamjene povratne vrijednosti, prikrivanja probea
ili zaobilazenja pravila. Logovi potvrduju pozive, ne njihov trosak u bajtovima.

Svaki sljedeci red ponovljen je za oba klipa i za video/originalni audio:

| Konfiguracija | Analize u konstruktoru | Pri prvom frameu | Bez probea |
| --- | ---: | ---: | --- |
| avformat | 3 | 3 | Ne |
| avformat-novalidate | 0 | 3 | Ne |
| avformat-novalidate + spremljeni timing/dimenzije | 0 | 3 | Ne |

Ukupno 12 slucajeva i 48 potvrdjenih parova ulaz/izlaz analize.
Standardni avformat je pozitivna kontrola: ako alat ne opazi njegovu analizu,
test pada umjesto da lazno potvrdi odsutnost probea.

Prvi video frame i prvi originalni PCM blok identicni su kroz sve tri
konfiguracije za isti klip. Pocetne slike i posljednje slike iz DB-bounded
slucajeva usporedjene su i s FFmpeg kontrolnim hashovima iz docs/68: podudaranje.

### Video granica

- Mironik 2679 baza: 11178 frameova, dozvoljeni indeksi 0..11177.
- Standardni MLT bez DB granice: 11179.
- Zadano prije prvog citanja: length=11178, in=0, out=11177.
- Nakon citanja MLT zadrzava 11178; slika na 11177 odgovara referenci.
- Mironik 2002 jednako zadrzava spremljenih 10194 frameova.

MLT `out` je ukljuciv, dok QNC raspon treba tumaciti kao [0, frame_count).
Ovo nije test ponasanja consumera na EOF-u. Adapter i dalje treba odbiti
zahtjev za frame >= frame_count, ne osloniti se na freeze/loop ponašanje.

## Provjera koda

Procitan je postojeci QNC put, bez izmjene:

1. qnc-work-settings cita postavke aktivne projektne baze read-only.
2. qnc-player-input / InputReader.load priprema i validira PreparedInput:
   odabir slike iz playback.input, audio format iz projekta, streamovi i
   source timebase iz finalnog spremljenog snapshot-a.
3. qnc-media-decode / Plan.command vec salje `-nofind_stream_info`, eksplicitni
   spremljeni kontejner i codec, tocnu stream mapu i `-copyts`. Media dolazi
   kroz qnc-media-stream HTTP endpoint/resolver, ne iz forme.

Time nije potvrdena kvaliteta postojeceg player clock/output puta. Nije
opravdanje za ponovno uvodjenje njegovih problema preko MLT omotaca.

MLT izvor provjeren na commitu
`0f8244a125872544fb34b88125ee18a88b4d0b85`:

- producer_avformat_init razlikuje obicni ulaz i novalidate samo pri otvaranju.
- producer_open nakon otvaranja poziva avformat_find_stream_info; nema grane
  koja to preskace na osnovu prethodno spremljenih MLT metapodataka.
- Postoje dodatni pozivi kod drugih format contexta i posebnih decode putova.
- `_probe_complete` pripada drugoj metadata/probe funkciji i ne uklanja
  obveznu analizu u producer_open. Nije podrzani no-probe prekidac.
- force_fps i meta.media.* ne rekonstruiraju AVFormatContext/AVCodecParameters
  iz QNC baze i ne uklanjaju pozive analize.

Izvorni commit sluzi za provjeru pristupa. Tocan build commit bundled DLL-a
nije utvrdjen; gore navedeni brojevi su mjereni na samoj bundled biblioteci.

## Odluka za nastavak

Ne uvoditi standardni MLT avformat u produkciju niti slabiti AGENTS.md.
Ne forkati cijeli Shotcut i ne preuzimati njegov UI.

Najmanji smisleni sljedeci pokus jest javni **QNC media producer adapter**
za MLT: ulaz je postojeci PreparedInput, media pristup ide kroz transport,
decode nema discovery/probe, a MLT dobiva frameove i originalne audio uzorke
za trazenu poziciju. Za pocetnu kvalifikaciju moze se koristiti postojeci
javni decoder, ali NE postojeci player/runtime/sat. Time se izbjegava kopiranje
aktivnog QNC ili MLT enginea. Adapter nije nova aplikacija ni owner baze.

Ovaj adapter jos NIJE implementiran ili potvrden. Prije ugradnje mora dokazati:

- Nula poziva dodatne analize pri pripremi, Playu, seeku i promjeni klipa.
- Tocne PTS/frame/audio sample granice i izostanak dodatnog zadnjeg framea.
- Dovoljnu decode/buffer rezervu, ne samo prolaz jednoga kratkog klipa.
- Jednog vlasnika playback sata; MLT i stari QNC runtime ne smiju paralelno
  voziti vlastite satove.
- Stvarni Ingest live test, ne zamjenu takvog testa helper rezultatima.

Ako taj uski adapter zahtijeva prepisivanje velikog dijela enginea ili
naslijedi dosadasnju nestabilnost, MLT nije automatski bolji izbor. Potrebna
je ponovna tehnicka odluka, ne novi slojevi radi zadrzavanja odabranog imena.

## Artefakti i ogranicenja

- Rezultat: `target/mlt-eval/probe-boundary.json`.
- Poziv zahtijeva `--allow-stream-analysis`; to nije trajna dozvola.
- Koristiti isto MLT okruzenje i Python -I kao u docs/68, zatim:

```powershell
python -I tools/diagnostics/mlt-probe-boundary.py --root . --mlt-dir target/mlt-eval/Shotcut --output target/mlt-eval/probe-boundary.json --allow-stream-analysis
```

SHA256 registryja, aktivne projektne baze i media-records baze ostao je isti.
Produkcijski kod i AGENTS.md nisu mijenjani u ovom koraku. Nije pokretan cargo
build/test jer nisu mijenjani Rust kod ili ugovori. Ovaj test ne zatvara live
Ingest, stvarni A/V izlaz, LAN/Intranet ili druge OS/CPU konfiguracije.

## Izvori

- [MLT producer, pinani commit](https://github.com/mltframework/mlt/blob/0f8244a125872544fb34b88125ee18a88b4d0b85/src/modules/avformat/producer_avformat.c)
- [MLT avformat properties](https://www.mltframework.org/plugins/ProducerAvformat/)
- [FFmpeg debug oznake unutar stream analize](https://github.com/FFmpeg/FFmpeg/blob/n8.1/libavformat/demux.c)
