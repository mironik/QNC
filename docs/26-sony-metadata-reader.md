# Sony XML reader - prvi izvrsni adapter

Datum: 2026-09-07. Nastavak odobren nakon docs/25.
Modul `qnc-sony-metadata`, ugovor `0.1.0`. Nije aplikacija, scanner ni probe.

## Granica prije implementacije

- Ulaz: XML tekst vec procitan kroz transport, QNC URI sacuvanog dokumenta,
  caller-resolved original/proxy reference i opcionalni sidecar.
- Izlaz: izricite index veze, dostupni `qnc.media.metadata` factovi i upozorenja.
- Nema filesystema, mreze, DB-a, FFmpeg/ffprobe, UI-ja ili Project/Shell znanja.
- `read_index` vraca recording-root-relative reference, ne izmisljene globalne
  media identitete. Caller ih provjerava kroz transport i daje QNC binding.
- `read_metadata` provjerava da binding odgovara tocnoj relativnoj referenci.
- UI i Ingest runtime manifest ostaju nepromijenjeni: parser jos nije scanner
  niti puni Select postupak.

## Podrzani zapisi i semantika

Izvor je docs/23 i novi read-only uvid u dostupni FX6 XML. Namespace-aware
`MediaProfile` reader prihvaca `http://xmlns.sony.net/pro/metadata/mediaprofile`.
Sidecar je `NonRealTimeMeta` u
`urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20`.
Nepoznata verzija/namespace nije automatski isti format.

Original dolazi iz `Contents/Material`, proxy samo iz njegova `Proxy`, a
XML/JPG veze iz `RelevantInfo`. Razlicit UMID proxyja je normalan; sidecarov
TargetMaterial mora se podudarati s UMID-om originala. Vise proxyja, dupli
identiteti ili dijeljene media reference nisu tiho svedeni na jedan rezultat.
Prazan indeks nije dokaz da je kartica prazna. Nepovezani fileovi su izvan
ovog readera i ostaju zadatak buduceg scannera.

Sva izvorna atributna polja odabranih media/sidecar zapisa zadrzavaju se kao
tag factovi s dokumentom i selectorom. Strani namespaceovi imaju zasebnu
`{namespace}ime` oznaku da ne prepisu istoimena Sony polja.
Ne izvodi se codec iz ekstenzije.
Za poznate AVC50/AVC_Proxy oznake normalizira se video codec h264; ostale
oznake ostaju raw dok adapter nema dokazano mapiranje.

U prvom rezu promoviraju se izricite dimenzije iz VideoLayout i progresivni
cjelobrojni FPS iz formatFps/index fps. Decimalni NTSC, interlaced, slow/quick
i ostale semantike cuvaju se bez izmisljanja FPS-a ili field ordera. Capture
FPS i LTC tcFps nisu zamjena za format FPS.
Duration se promovira u exact frame count/racionalne sekunde samo za
potvrdeni normalni progressive zapis s jednakim capture/format FPS-om,
istim index/sidecar trajanjem i nultim originalnim offsetom.
Proxy koristi vlastiti fps/dur; prazna polja ne nasljeduje od originala.

AudioFormat/numOfChannel i CH1-CH4 su camera kanalni opis, ne dokaz broja
streamova u kontejneru. Zadrzavaju se kao factovi; ne kreira se jedan lazni
zbrojeni audio stream niti se brojevi CH pretvaraju u container indekse.
Container stream index, time_base, start_pts, audio sample rate/format,
pixel format i drugi nepoznati podaci ostaju prazni. `streams_complete`
se ne postavlja na true iz ovih XML-a.

CreationDate se cuva doslovno s UTC pomakom; camera serial, mediaId i UMID
ostaju razlicite vrste podataka. Capture gamma/color nisu bez dodatnog dokaza
automatski prepisani u kodirani video color opis.

## Sigurnost i katalog

XML obradu radi postojeci xml-rs parser, ne regularni izrazi. Ulaz je
ogranicen velicinom, dubinom i brojem elemenata/atributa. DTD/ENTITY su
zabranjeni prije parsiranja; nema vanjskog resolvera. Prihvaca se UTF-8.
Relativne reference cuvaju case i razmake, dekodiraju URI escape jednom i
odbijaju absolute path, parent traversal, backslash, URL, query/fragment i
double-encoding. Ne smiju izaci iz recording roota. To nije provjera
symlinka/junctiona ni postojanja filea: to mora provjeriti transport.

Postojeci katalog kamera ostaje jedini katalog. Dopuna Sony selectors ide
u postojece podatke, ne u drugi katalog ili izvrsljive skripte. Ranija
objavljena SQLite verzija se ne prepisuje. Katalog ostaje research-only za
cjelokupan detector; ovaj uski parser ne daje podrsku svim kamera-obrascima.

Dopuna je objavljena postojecim `qnc-camera-catalog revise` alatom:

- Osnova: `catalogs/camera-patterns/camera-patterns-v1.sqlite`, dataset
  `2026.09.06.1`. Seed i objavljena osnova nisu promijenjeni.
- Revizija: `catalogs/camera-patterns/revisions/2026.09.07.1.json`.
- Izlaz: `catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite`.
- Samo `sony-xdroot-sd` dobiva dodatne selectors (ukupno 28) i dokaz opazanja.
  Promjena ima before/after zapis i razlog u istom katalog ugovoru.
- Test usporeduje novo izdanje s deklariranom revizijom te provjerava da je
  osnova ostala nepromijenjena. Nema aplikacijske DB migracije.

## Verifikacijski plan

Anonimizirani fixtures prate opazeni format, ne stvarne osobne nazive/seriale.
Testirati namespace, veze, nedostajuci proxy, zasebne proxy podatke, UMID,
duplikate, nepodrzane semantike, konfliktne factove, DTD/XXE, path escape,
neispravan XML i local/LAN/intranet QNC bindinge. Zatim isti Rust reader
provjeriti nad stvarnim indexom i sidecarima, read-only, bez media probea.
JSON stdin primjer sluzi provjeri parsera; nije nova aplikacija ili runtime
filesystem zaobilaznica. Ne ukljucuje podatke korisnika u spremljene fixtures.

Mrezni pristup, stvarni media fileovi, import i UI live ponasanje nisu ovaj
korak. Sljedeci je povezivanje readera sa source/transport postupkom.

## Izvrsena provjera

2026-09-07, Windows x86-64, Rust debug build:

- `cargo test --workspace --locked --offline --quiet`: 229/229 testova.
- `qnc-sony-metadata`: 23 testa, ukljucujuci neispravan XML, identitete,
  namespace, konflikte, DTD/XXE, granice ulaza i URI/binding provjere.
- `qnc-camera-catalog`: 17 testova, ukljucujuci nepromjenjivu osnovu i reviziju.
- Clippy za oba navedena cratea, `--all-targets -- -D warnings`: prolazi.
- `qnc-conformance` s apsolutnim QNC rootom: sve provjere prolaze.
- Provjera novog kataloga: 23 obrasca, 17 dokumentiranih kandidata za analizu,
  7 poznatih praznina; runtime detector i dalje nije implementiran.

Stvarna kartica citana je iskljucivo read-only: `MEDIAPRO.XML` i 103
povezana `Clip/*M01.XML` zapisa. Rust stdin primjer dobio je njihove tekstove
i sintetske QNC identitete samo za provjeru parsera. Nije testiran stvarni
resolver binding, nisu citani media streamovi, nije pokrenut ffprobe/decode
niti je bilo upisa u karticu ili Ingest bazu.

| Rezultat | Broj |
| --- | ---: |
| Originali iz indexa | 103 |
| Izricito povezani proxyji | 103 |
| Datumi snimanja s izvornim UTC pomakom | 103 |
| Originali s podrzanom exact frame semantikom | 103 |
| Proxyji s vlastitim fps/dur za exact frame podatak | 22 |
| Proxyji bez tih vlastitih podataka; nisu naslijedeni | 81 |
| Konflikti readera | 0 |
| Potpuni metadata zapisi za downstream ugovor | 0 |
| Procitani XML fileovi s promijenjenim SHA-256 | 0/104 |

Posljednji prolaz: 125 ms za parsiranje i mapiranje XML-a vec u RAM-u.
To nije vrijeme kompletnog Selecta, citanja kartice, JSON prijenosa,
hashiranja ili media obrade. Prethodni prolaz bio je 120 ms.

Nula potpunih zapisa nije tvrdnja da nema korisnih podataka: ovi XML-i ne
dokazuju potpuni container stream layout/timing i audio formate. Buduci
Select postupak mora dopuniti nedostajuce podatke u jedinom ingest probe
prolazu i sacuvati izvorne camera factove. Reader sam ne odlucuje pokrenuti
probe i ne spaja rezultate ffprobea.

Nije potvrdeno: drugi modeli/firmware i recording modovi, stvarni LAN/Intranet
transport, Linux/macOS/ARM izvrsavanje, cjelovitost medija ni puni Ingest
live workflow. UI i aplikacijski runtime nisu mijenjani.
