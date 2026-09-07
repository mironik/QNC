# Javni ugovor media metapodataka

Datum: 2026-09-07. Korak odobren nakon audita docs/24.
Contract: `qnc.media.metadata`, verzija `0.2.0`.
Modul: `qnc-media-metadata`, capability `media.metadata.validate`.

## Opseg i granica

Ovaj korak definira serijalizirani zapis i cistu provjeru podataka u memoriji.
Nema scanner/parser/probe procesa, citanja medija, DB pisanja, UI promjene,
novog aplikacijskog workflowa ili migracije. Modul ne zna tko ga koristi.
Ne dodaje se u Ingest runtime manifest dok ga runtime stvarno ne koristi.

Zapis je namijenjen payloadu `public_probe_records.probe_json` postojeceg
Ingest content DB ugovora. Naziv stupca ne znaci da je ffprobe izvrsen:
izvor moze biti potpun zapis kamere. Owner ce u zasebnom koraku spojiti upis.
Sam payload ne ukljucuje Project, korisnika, aktivni projekt ni UI stanje.

## Struktura

- Jedan `clip_id`, obavezni `original`, najvise jedan opcionalni `proxy`.
- Svaka reprezentacija ima vlastiti `media_uri`, container, trajanje u
  racionalnim sekundama, potpunost popisa streamova i svoje streamove.
- Svaki stream ima stvarni container `index`, codec, opcionalni profile,
  racionalni `time_base`, `start_pts`, `duration_ts` i video/audio/other opis.
- Video: izvorni `FrameTimebase` iz javnog frame/timebase modula, frame count
  s oznakom exact/estimated, constant/variable/unknown rate mode, dimenzije,
  scan mode, pixel format, sample aspect ratio, rotacija i opis boja.
- Audio: sample rate, broj kanala tog streama, sample format, channel layout
  i opcionalna bit dubina. Nema zbrajanja kanala ni ogranicenja na 8 kanala.
- Ostali streamovi nisu odbaceni: cuva se njihova vrsta i zajednicki opis.
- `tags` cuva dodatne tekstualne podatke s porijeklom, npr. izvorni datum i
  vrijeme kreiranja s vremenskom zonom, timecode, UMID ili camera model.
  Tagovi nisu zamjena za obavezna tehnicka polja i ne cine nepotpun zapis potpunim.
  Opcionalni izvorni tag smije imati praznu vrijednost (npr. Sony mediaName="");
  cuvaju se naziv, porijeklo i izvorna praznina. To ne slabi provjeru obaveznih
  tehnickih polja i nije zamjenska/default vrijednost. Rubni slucaj potvrden
  stvarnim Sony dokumentom tijekom DB integracije, docs/31.

Nema trajnih OS pathova. I medij i dokument dokaza imaju QNC URI za
local/LAN/intranet. Modul validira identitet, ne radi resolve niti dokazuje
postojanje ili dostupnost datoteke. To ostaje posao transporta/producera.

## Porijeklo svakog podatka

Svaki `Fact<T>` sadrzi `value`, `evidence_id` i `locator` izvornog polja.
`evidence` katalog unutar zapisa sadrzi:

- `id`: referenca koju koriste factovi;
- `kind`: `camera_metadata` ili `ffprobe`;
- `document_uri`: QNC referenca na sacuvani izvorni XML/JSON zapis;
- `media_uri`: tocna reprezentacija na koju se dokaz odnosi.

Isti kamerin indeks moze opisivati original i proxy. Tada dva evidence
zapisa smiju pokazivati na isti dokument, ali su vezana uz razlicite medije.
Fact originala ne smije se automatski prihvatiti kao fact proxyja.
Locator je oznaka polja unutar dokumenta, npr. XML element/atribut ili JSON
pointer; nije putanja koju modul otvara. Producer mora prije izdavanja facta
provjeriti vezu dokumenta i medija. Ugovor nije dokaz autenticnosti dokumenta.

Izvorni dokumenti moraju se sacuvati kroz owner DB/artifact ugovor; nije
dovoljno zadrzati samo pokazivac na karticu koja ce biti izvadena. Ovaj korak
ne implementira njihovo spremanje. Nepotrebna ponovljena kopija dokumenta po
factu nije potrebna; factovi referenciraju evidence.

## Nedostaje, nije zapisano, nije primjenjivo

- `None` u tehnickom polju znaci da podatak jos nedostaje.
- `streams_complete = true` s praznim audio popisom znaci potvrdeno nema audija.
  Prazan popis bez potvrde potpunosti nije isto sto i nema audija.
- `Signal::Unspecified` cuva potvrdu da izvor ne deklarira vrijednost,
  npr. color primaries ili channel layout. To nije Rec.709/stereo default.
- Nepoznat codec, FPS, dimenzije ili scan mode ne smiju se prikriti defaultom.
- Codec je `Signal<String>`: audio/video zahtijeva Known; dodatni stream smije
  imati izriciti Unspecified uz sacuvanu vrstu, indeks i izvorni izvjestaj.
  To ne znaci da RTMD ima podrzan decoder niti da se stream smije odbaciti.
- Rotacija je `Signal<i32>`: Known cuva prijavljene stupnjeve, Unspecified
  potvrdu da puni stream izvjestaj ne deklarira rotaciju/display matrix.
  None znaci nerazrijesen podatak (ukljucujuci matricu bez citljive rotacije
  ili nepodrzani legacy rotate tag), nikad implicitnu nulu.
  Izvorni display matrix i ostali podaci ostaju u punom JSON-u za potrosaca.
  Verzija 0.1.0 nije podrzana niti se migrira; spremljeni izvorni XML/JSON
  moze se citati za provjeru novog parsera bez izvrsavanja novog probea.
- AAC codec i dekodirani sample format, npr. fltp, nisu PCM bit dubina.
  Neprimjenjiva PCM bit dubina ostaje prazna; ne izmislja se PCM24.
- `start_pts` smije biti nula ili negativan. `duration_ts`, dimenzije, rate,
  sample rate i broj kanala moraju biti pozitivni.
- `FrameCount::Estimated` i variable/unknown frame rate ostaju eksplicitni.
  `exact_frame_count()` ne vraca procjenu kao tocan broj frameova.

`inspect()` vraca `missing` i `invalid` probleme s putanjama polja. Ne mijenja
zapis i ne odlucuje treba li pokrenuti neki proces. `is_complete()` znaci da
je opis u skladu s ovim ugovorom, ne da svaki buduci decoder podrzava medij.
Ne jamci frame-precizan VFR seek ni tocnost procijenjenog frame counta.
Potrosac koji treba precizne frame granice mora zahtijevati exact count i
podrzan model vremena; ne smije pokrenuti probe da popravi taj nedostatak.

Kontejnersko trajanje nije zamjena za trajanje pojedinog streama. Story IN/OUT
i raspored segmenata nisu dio media metapodataka. Audio/video streamovi mogu
imati razlicite pocetke i trajanja; ne izjednacuju se automatski.

Za original/proxy tocne video frame count vrijednosti i poznati constant
FPS moraju se podudarati kad ih oba zapisa imaju. Dimenzije, codec, container
i audio format smiju se razlikovati. Sama podudarnost vremena nije dokaz
pairinga; pairing po kamerinu zapisu ostaje odgovornost scanner/parser modula.

## Jedini ingest prolaz

Buduci tijek: postojece vezano camera metadata -> provjera potpunosti ->
ffprobe samo za reprezentaciju kojoj potrebni podaci nedostaju -> trajni zapis.
Original/proxy ostaju jedan clip, bez probea proxyja kao zasebnog clipa.
Nema probea iz validatora, playera, Storyja, filmstripa, wavea ili exporta.
Ako ni taj jedini prolaz ne da potreban podatak, owner zapisuje nedostatak;
ne pokrece petlju ponavljanja niti naknadni repair. Ovaj korak ne implementira
merge, fallback odluku, ffprobe brojac niti runtime upis.

## Provjera ovog koraka

Provjereno: JSON round-trip, nepoznata verzija/polja, nedostajuci podaci, original
bez proxyja, odvojeni original/proxy codec i audio, vise audio streamova,
video bez audija, audio-only bez izmisljenog FPS-a, tocan racionalni FPS,
negativan PTS, neslaganje proxy vremena, evidence binding, raw path odbijanje
i iste serijalizacijske provjere za local/LAN/intranet.

Rezultati na Windows hostu:

- `cargo test -p qnc-media-metadata -p qnc-frame-timebase --locked --offline`:
  23 nova ugovorna testa i 6 postojecih frame/timebase testova prolaze.
- `cargo test --workspace --locked --offline --quiet`: 205 testova prolazi.
- `cargo clippy -p qnc-media-metadata -p qnc-frame-timebase --all-targets
  --locked --offline -- -D warnings`: prolazi bez upozorenja.
- `cargo fmt -p qnc-media-metadata -p qnc-frame-timebase --check`: prolazi.
- `qnc-conformance` nad apsolutnim QNC rootom: sve provjere prolaze,
  ukljucujuci v4 keyboard referencu, novi manifest i DB contract.
- Normalni Cargo dependency tree novog modula sadrzi samo neutralne
  qnc-contracts/frame-timebase i serde ovisnosti; nema aplikacijskog storea,
  UI-ja, filesystem/transport klijenta ni process executora.

Pocetni URI test otkrio je propust s `qnc://local/media/C:/clip`.
Popravljeno i ponovno provjereno. Clippy je otkrio nepotrebno velik enum;
video/audio payloadi sada su boxani, bez promjene JSON ugovora.
Prvi conformance poziv s relativnim `.` nije pronasao v4 keyboard referencu;
ponovljen je s apsolutnim rootom i zavrsio bez tog upozorenja.

Nije live ingest korak. Nema promjene vidljivog ponasanja koju bi se moglo
potvrditi pokretanjem aplikacije. Stvarna kartica i produkcijske baze ostaju
netaknute. Runtime povezivanje slijedi u zasebnom odobrenom koraku.

Nije provjeren stvarni LAN/intranet host, Linux/macOS/ARM izvrsavanje,
stvarni kamerin parser, ffprobe, DB round-trip ni player/Story reprodukcija.
Ovdje su provjereni transport-neutralni podaci, ne mrezni I/O. Sljedeci rizik
je ispravno preslikavanje stvarnih camera zapisa u ovaj ugovor: stvarni stream
indeks, timescale i potvrda potpunosti ne smiju se izmisliti iz samog FPS-a.
