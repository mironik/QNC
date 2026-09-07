# Deklaracije metapodataka i trajni pocetak probea

Datum: 2026-09-07. Odobren nastavak nakon docs/32.

## Granice zahvata

Prvo se zatvara semantika metapodataka, a zatim trajna rezervacija jedinog
probe pokusaja prije njegova izvrsavanja. Parser, DB adapter i executor
ostaju odvojeni javni moduli; nijedan ne smije postati Ingest workflow.
UI, Project i shell nisu mjesto za taj kod.

`qnc.media.metadata` 0.2.0 razlikuje Known/Unspecified codec i rotaciju.
Nepoznat codec za audio/video ostaje nedostatak. Za ancillary stream je
izricito neprepoznat codec valjan opis, ne dokaz podrzanog dekodiranja.
Parser cuva FourCC/codec tag kao fact uz tocni stream pointer i cijeli
izvorni JSON. RTMD se ne brise i ne pretvara u audio/video.

Rotacija Unspecified znaci samo odsutnost deklaracije u punom stream
izvjestaju. Nije 0 stupnjeva niti dokaz da je proizvoljna matrica identitet.
Prisutna matrica bez citljive rotacije ili nepodrzani rotate tag ostaju
nerazrijeseni. Poznata rotacija ne zamjenjuje izvorni display matrix.
Izvor semantike: [FFmpeg ffprobe source](https://ffmpeg.org/doxygen/trunk/ffprobe_8c_source.html),
print_displaymatrix i show_stream (rotation i odvojeni codec_tag_string).

Nema migracije starog derived zapisa. Replay test ponovo parsira samo
arhivirane izvorne XML/JSON dokaze u novu privremenu provjernu bazu.
Ne izvrsava ffprobe niti cita ili mijenja karticu.

## Trajni pokusaj (prije implementacije)

Rezervacija je kratka atomska DB transakcija. Klijent smije izvrsiti probe
samo ako je upravo dobio novu rezervaciju; ponovljen request nikad ne
smije ponovno dati dozvolu za izvrsavanje. Ishod se zapisuje odvojeno.
Poceti pokusaj bez ishoda nakon pada/timeouta ostaje neizvjestan i ne smije
se automatski ponoviti. To je at-most-once, ne lazno exactly-once jamstvo.
DB nije zakljucan dok se cita medij. Identitet ne smije ovisiti o UI-ju,
aktivnom projektu ili jednom procesu. Udaljeni pristup koristi isti
ugovor i resolver, bez direktnog dijeljenja SQLite datoteke preko mreze.

Ugovor `qnc.media.records`/DB je 0.2.0 (SQLite user_version 2), bez migracije.
Pokusaj je jedinstven po stabilnom javnom media URI-ju unutar owner baze,
ne po projektu/clip_id-ju. Original i proxy imaju zasebne fizicke URI-je,
ali oba se provjeravaju prema istom postojecem camera snapshotu.
Final snapshot nikad ne dobiva novi pokusaj. Novi attempt_id ili document_uri
ne zaobilazi prethodni pokusaj za isti medij.

`BeginAcquisition` commit-a pocetak uz synchronous=FULL prije `granted=true`.
Ponavljanje Begin uvijek vraca `granted=false`. `FinishAcquisition` atomski
pohranjuje izvorni JSON i ishod (stored/failed/uncertain); smije se ponoviti
samo isti ishod i identicni dokument, bez ponavljanja izvrsvanja.
Begin bez Finish nema rok isteka niti reset/retry operaciju. Citanje statusa
ne moze dodijeliti pravo izvrsavanja. Nema DB write locka izmedu Begin i Finish.

Granica jamstva: jedna vlasnicka baza i jedan stabilni javni media identitet.
Duplicirana offline baza ili novi alias istog fizickog medija nisu automatski
globalno koordinirani. Owner mora zadrzati isti identitet i bazu pri promjeni
transport endpointa. Ovaj zahvat ne uvodi offline sync niti tvrdi globalni
exactly-once preko nepovezanih kopija. Stare testne arhive se ne probaju opet.

Runtime Select i live integracija nisu zavrseni samim dodavanjem ovih
ugovora.

## Izvrsena verifikacija

- `cargo test --workspace --locked --offline --quiet`: 367/367 prolazi.
  U ovom koraku dodano je 17 testova (5 metadata/parser + 12 acquisition).
- `qnc-media-record-db`: 26/26, ukljucujuci konkurentne konekcije, child
  exit bez destructora, potvrden DB commit uz izgubljeni odgovor, ponovljeni
  Begin/Finish, partial/final granicu, jedinstveni original/proxy pokusaj,
  atomicki zapis dokaza, read-only ovlasti i nepostojanje dugog write locka.
- `qnc-ffprobe-metadata`: 14/14; `qnc-media-metadata`: 25/25.
- Clippy svih sedam dotaknutih producer/contract/DB crateova, all-targets
  i `-D warnings`: prolazi. Ciljani cargo fmt --check: prolazi.
- QNC conformance nad apsolutnim rootom: sve provjere prolaze.
- Git diff provjera potvrdjuje da nema promjena aplikacija, Ingest forme i
  komponente, Project scopea, UI/keyboard ugovora ili seeda u ovom koraku.

Replay svih pet ranijih arhiva: 103 original/proxy clipa, 206 sacuvanih
JSON izvjestaja, 0 novih ffprobe poziva, 0 konflikata. Novi parser i DB
readback daju 102 Final/Complete zapisa po ugovoru 0.2.0. Prvi clip iz
starog profila 1 MiB/0.1 s ostaje Final/Partial: proxy pixel_format i
sample_aspect_ratio nedostaju i nisu izmisljeni niti ponovno probani.
Arhivirani JSON tekst provjeren je identicnim pri readbacku nakon zatvaranja
writera. Izvorne arhive i kartica nisu mijenjane; nema migracije.

Complete je potpunost ovog ugovora, ne tvrdnja da su svi buduci potrosaci
implementirani ili da je procijenjeni frame count postao exact.

## Otvoreno za sljedeci korak

Select jos nije povezan s ovim modulima. DB API provodi trajno odbijanje
ponovljenog pocetka, ali trenutni stateless Media Probe helper ne posjeduje
ledger i to ne radi sam. Ingest komponenta mora najprije dobiti novi claim,
zatim izvrsiti modul i trajno spremiti ishod prije objave rezultata iz baze.
Nijedna greska transporta ne daje dozvolu za probe retry.

Preostaje povezati taj slijed i scanner/camera/source-index module u
pozadinskom poslu Ingest komponente, bez poslovne logike u formi. UI treba
primati procitane DB zapise/progress bez cekanja svih clipova. Puni live
Select test nije pokrenut niti se ovaj korak predstavlja kao gotov Ingest.

LAN/intranet provjereni su javnim ugovorom preko loopback HTTP testnog
hosta. Stvarni udaljeni TLS deploy, nepovezane offline kopije baze,
Linux/macOS/ARM izvrsavanje i UI prikaz nisu provjereni ovim korakom.
