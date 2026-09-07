# Media record DB: snapshoti podataka, bez izvrsavanja probea

Datum: 2026-09-07. Nastavak docs/25 i docs/30.

Naknadna dopuna: docs/33 podize contract/shema na 0.2.0 i dodaje trajni
acquisition pocetak/ishod, bez probe executora u ovom modulu. Donji opis
pocetnog snapshot koraka i tada izmjerena verifikacija ostaju povijesni zapis.
Acquisition provjerava JSON envelope identitet, ne tumaci tehnicka polja.

## Javne granice

- `qnc-json-transport`: ograniceni JSON zahtjev/odgovor, resolver, HTTPS,
  read/write bearer ovlasti. Nema DB sheme, poslovnih akcija, aplikacijskog
  registryja ili workflowa. Koriste ga source-index i media-record DB adapteri.
- `qnc-media-records`: cisti ugovor za povezivanje javnog source zapisa,
  postojecih media factova i dokumenata dokaza. Nema I/O ili mergea factova.
- `qnc-media-record-db`: zasebna owner-bound SQLite baza i transport adapter.
  Ne ovisi o scanneru, Sony parseru, source readeru, UI-ju ili aplikaciji.

Produkt je `qnc.db.media_records`, odvojen od source-indexa i registryja
kartica. Nema promjene postojecih baza, migracije, Ingest runtime povezivanja
ili promjene forme. `ingest_content` se u ovom koraku ne mijenja.

## Ulaz i identitet

Write sadrzi request ID, expected revision, phase, source-index DB URI i
javni `Record` iz source-index baze, `ClipMetadata` iz docs/25 i UTF-8
XML/JSON dokumente na koje evidence pokazuje. To je podatkovni snapshot iz
javnog DB ugovora, ne runtime context druge aplikacije.

Adapter ne cita izvorne datoteke ni source-index bazu. Producer mu daje vec
procitani javni zapis. Ugovor provjerava original/proxy URI-je protiv tog
zapisa. Ne moze dokazati autenticnost producerovih tvrdnji; write ovlast daje
owner. Trajni binding cuva DB URI, source record ID i original/proxy reference.

`clip_id` dolazi od ownera. Isti source record ne moze pod drugim clip ID-em
stvoriti duplikat u istoj bazi. Promjena source bindinga postojeceg clip ID-a
se odbija. Proxy nije zaseban clip. Postojeci metadata ugovor podrzava 0/1
proxy: source grupa s vise proxyja se izricito odbija, bez izbora prvoga.

## Dvije odvojene oznake

- `phase = camera`: pocetni snapshot kamera podataka, bez ffprobe evidencea.
- `phase = final`: zavrsen jedini producerov prolaz; nema daljnjih promjena.
- `completeness = partial/complete`: izracunava se iskljucivo javnim
  `media.metadata.validate` ugovorom, ne prihvaca se od klijenta.

Final moze ostati partial ako ni jedini prolaz nije dao potreban podatak.
To nije nalog za naknadni probe ili repair. Complete camera zapis nije dokaz
da je ffprobe bio izvrsen. Nijedna oznaka ne pokrece proces.

Dozvoljeno: prvi camera ili prvi final zapis; camera -> final uz tocnu
expected revision. Ponovljeni identicni request vraca isti receipt.
Promijenjen request pod istim ID-em, zastarjela revizija, camera -> camera,
final -> bilo koja nova verzija i promjena identiteta su greske.
DB modul ne spaja, dopunjava ili izmislja metadata polja. Producer mora
pripremiti cijeli sljedeci snapshot; prethodni ostaje trajno sacuvan.

Nevaljani factovi odbijaju cijeli zapis. Nedostajuci factovi se cuvaju s
reportom, bez default FPS-a/boje/kanala i bez maskiranja potpunosti.

## Dokazi i pohrana

Dokumenti dokaza zapisuju se u istoj transakciji kao snapshot i receipt.
Referenca na karticu nije jedina kopija: originalni XML/JSON tekst ostaje u
bazi i moze se procitati bez kartice ili Ingest procesa. Jedan dokument moze
opisivati vise klipova i pohranjuje se samo jednom po document URI-ju.
Isti URI s drugim tekstom/media typeom je konflikt, nikad overwrite.
Camera evidence mora referencirati indeks ili povezanu dostupnu pomocnu
datoteku iz source zapisa. Ffprobe evidence cuva vlastiti JSON dokument.
Modul ne parsira XML/ffprobe JSON i ne ponavlja posao metadata producera.

Limiti: 16 MiB zahtjev/odgovor, 8 MiB po dokumentu, najvise 16 dokumenata,
128 streamova po reprezentaciji, 64 evidence zapisa. Provjere i
serijalizacija inputa izvode se prije kratke write transakcije.

Baza cuva head, nepromjenjive revizije, deduplicirane dokumente i receipt.
Public viewovi objavljuju binding, metadata, completeness report i dokaze.
SQLite WAL/busy_timeout/foreign_keys i tocna provjera sheme; nema migracija.
DB je lokalna na storage hostu, ne otvara se kao mrezni SQLite file.

URI: `qnc://local/db/media_records`, odnosno LAN/intranet authority varijanta.
POST `/v1/media-records`; isti ugovor za lokalni i udaljeni pristup.
Samo owner bootstrap postavlja privatni binding i read/write ovlasti.
Nema consumer allowlista. TLS i lifecycle hosta nisu Ingest workflow.

## Verifikacija i preostali koraci

Provjeriti: partial/complete neovisno o phase, original/proxy binding,
izgubljeni/konfliktni dokazi, atomicni rollback, ponavljanje, CAS/final lock,
read-only ovlasti, restart i isti local/LAN/intranet ugovor.
Live modulni test cita stvarne Sony dokumente read-only, sprema ih u
privremeni DB artefakt i provjerava da se svi podaci/dokazi citaju nakon
zatvaranja source readera/writera. Nije puni UI Select niti probe test.

Slijedi priprema jedinog metadata/probe prolaza u zasebnim javnim modulima,
pa povezivanje kroz Ingest komponentu. Nijedan DB/transport modul ne smije
preuzeti te odgovornosti niti pozivati ffprobe.

## Rezultat provjere 2026-09-07

- 18 novih testova: 14 media DB/contract testova, 3 transport testa i 1
  regresija praznog opcionalnog camera taga. Workspace: 325/325 prolazi.
- Ponovno prolazi 12 source-index DB testova nakon prebacivanja na zajednicki
  transport. Njegov javni wire ugovor ostao je nepromijenjen.
- Ciljani Clippy all-targets s `-D warnings`, cargo fmt --check,
  git diff --check i puni conformance prolaze.
- Stvarna Sony kartica: 103 clip snapshota, 103 pripadajuca proxyja, 103
  originalna datuma kreiranja. Svih 103 je camera/partial, nula complete.
- Spremljena su 104 izvorna XML dokumenta (jedan zajednicki indeks i 103
  sidecara), svaki jednom. Ponovljeni writeovi daju isti receipt.
- Svi metadata snapshoti i svi dokumenti ponovno su procitani iz baze nakon
  zatvaranja source readera, writera i mreznog testnog hosta.
- Local: 5783 ms; LAN loopback: 7532 ms; intranet loopback: 7557 ms. To su
  pojedinacni debug modulni testovi koji ukljucuju source grupiranje/upis,
  parsiranje XML-a, dvostruki metadata write radi replay testa i readback.
  Nisu mjerenje ffprobea, punog Ingesta ili same brzine jednog DB upisa.
- Hashovi 106 XML datoteka kartice i dva objavljena camera kataloga identicni
  su prethodnom koraku i nakon svih testova (108 SHA-256 provjera).
- Testne baze su privremeni SQLite artefakti, automatski uklonjeni nakon
  testa. Postojece razvojne baze i aplikacijski runtime nisu mijenjani.

Live je otkrio da Sony `Attached/@mediaName` moze biti prisutan, ali prazan.
`qnc-media-metadata` je pogresno odbijao tu opcionalnu izvornu vrijednost.
Sada je cuva uz obvezni naziv taga i porijeklo. Prazan obavezni container/
codec i dalje je nevaljan; regresijski test potvrduje obje strane pravila.

Reprodukcija na Windows testnom hostu (putanja je samo privatni testni ulaz):

```powershell
cargo run -p qnc-media-record-db --example sony_records --locked --offline -- local G:\PRIVATE\XDROOT
cargo run -p qnc-media-record-db --example sony_records --locked --offline -- lan-loopback G:\PRIVATE\XDROOT
cargo run -p qnc-media-record-db --example sony_records --locked --offline -- intranet-loopback G:\PRIVATE\XDROOT
```

Nije provjeren fizicki LAN/intranet/TLS deployment, Linux/macOS/ARM runtime,
UI Select, ffprobe, filmstrip, wave ili player. Od tih testova ne izvodi se
tvrdnja da je puni Ingest zavrsen. Sljedeci stvarni nedostatak su tehnicka
polja koja kamera ne zapisuje; njih smije pripremiti samo jedini ingest
metadata/probe prolaz kroz zaseban javni modul, nikad ovaj DB adapter.
