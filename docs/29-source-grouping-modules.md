# Javno grupiranje izvornih datoteka

Datum: 2026-09-07. Nastavak docs/28; ugovori prije implementacije.

## Granice

- `qnc-source-contract`: postojeci SourceReference i wire modeli izdvojeni iz
  transporta. Samo podaci i validacija, bez filesystema, mreze ili workflowa.
  Source reader ih ponovno izvozi radi jednog tipa i jedne implementacije.
- `qnc-source-groups`: cisti javni model/validator grupiranja. Prima dokazive
  original/proxy/related veze i rezultate dostupnosti. Ne cita direktorije,
  XML ili baze, ne racuna probe ni clip ID i ne zna proizvodjaca/aplikaciju.
- `qnc-scanner`: javna read-only kompozicija detektora, transporta i
  registriranih index readera. Nema hardkodirane Sony/brand/app grane.
  Caller predaje citace kroz uski javni trait, ne tablicu dozvoljenih appova.
- Sony reader implementira trait cistim citanjem XML-a u memoriji. Ne dobiva
  filesystem, transport klijent, bazu, UI niti Ingest radne postavke.

Zapisivanje rezultata je zasebna odgovornost owner DB modula/komponente.
Ovaj korak zatvara grupiranje prije DB povezivanja: scanner nikad ne pise u
Ingest bazu, ne bira aktivan projekt, ne pokrece probe i nije aplikacija.
Postojeci Ingest store jos nema transportni write ugovor; nije dopusteno
preskociti ga lokalnim SQLite upisom iz novog scannera.

## Ugovor grupe 0.1.0

Index reader vraca prijedloge: recording root QNC referenca, dokument
dokaza, reader capability, recording identity, tocno jedan original,
nula ili vise proxy referenci te pratece reference s vrstom iz indeksa.
Identitet je scopean na izvor i recording root; nije globalni clip ID.
Sve reference moraju biti unutar istog recording roota.

Grupiranje koristi stvarni file stat/listing rezultat od caller transporta.
Nedostajuci/necitljivi original ili proxy blokira grupu; nikad ne kreirati
proxy kao original ili potiho odbaciti proxy iz indeksa. Nepostojeci
thumbnail/sidecar ostaje eksplicitno zabiljezen bez izmisljanja podataka.
Vise proxyja cuva se bez proizvoljnog odabira; izbor podrzane reprezentacije
je odvojen korak, ne 1. proxy iz liste.

Ista media referenca u dvije grupe, ista scoped identity s vise prijedloga,
original=proxy i prateca datoteka predstavljena kao media su sukobi.
Ne primjenjivati first-match-wins. Jednaki nazivi nisu dokaz veze.
Svaka izlazna grupa cuva identitet izvora, root, evidence dokument i sve
original/proxy/related reference. Nema ugradjenih imena aplikacija.

## Scanner 0.1.0

Katalog -> detector -> index file iz kataloga -> read-only transport ->
registrirani reader prema namespaceu -> provjera referenci -> grupiranje.
Reader mora biti jedinstven za izabrani namespace; vise mogucih readera je
nerijeseno stanje. Pogresan/nedostajuci index ili nepodrzana shema ne pokrecu
probe, suffix pairing ili tihi original-only fallback.

Kandidati koje nijedan potvrdeni index nije obuhvatio ostaju u unresolved
popisu s ulogama. To ukljucuje moguce kopirane datoteke; njihov status nije
"gotov probe" niti siguran original. Ostaje zaseban buduci put za podatke iz
sidecara i jedini probe tijekom Select/Ingesta.

Listanje i svi stat/read pozivi idu kroz isti QNC source transport. Ograniciti
broj index dokumenata, grupa i provjera datoteka. Cache vrijedi samo jedan
poziv, nije globalno stanje niti veza izmedju aplikacija. Rezultat nije
atomski snapshot aktivno mijenjane kartice.

## Verifikacija

Testirati konflikte, osirotjela media, nedostajuci original/proxy/support,
vise proxyja, zasebne rootove, pogresan URI, serialization, drugi reader bez
izmjene scannera i local/LAN/intranet isti rezultat. Zatim read-only stvarna
kartica, hash prije/poslije. UI i app manifesti ne tvrde da je Select dovrsen.

## Izvrseno

Windows x86_64, 2026-09-07:

- Cijeli workspace: 291 test, svi prolaze. Novi contract 3, grupiranje 12,
  scanner 12; svi prethodni transport/parser testovi ostaju zeleni.
- QNC conformance: all checks passed.
- Ciljani Clippy za pet dodanih/izmijenjenih modula i njihove testove/primjere
  prolazi s `-D warnings`. Ciljani cargo fmt i git diff --check.
- `cargo tree -p qnc-source-groups --edges normal`: samo source-contract,
  QNC contract validacija, serde/JSON i percent-encoding (te derive build
  ovisnosti). Nema SQLite, HTTP, filesystem adaptera, UI-ja ili app crateova.
- Testni drugi IndexReader radi bez promjene produkcijskog scannera. Dva
  readera za isti namespace prijavljuju ambiguity; nema prvog pobjednika.
- Stari source-reference JSON i source-reader javni tipovi ostaju identicni
  kroz re-export jedne implementacije; nema kopije validatora ili migracije.
- SourceReference descendant/containment provjere koriste samo QNC source
  identitet i neutralne relativne komponente, ne trenutni OS path.

Read-only stvarna kartica, od korijena `G:\` kao privatnog testnog bindinga:

| Nacin | Recording grupe | Proxy | Related | XML index citanja | File facts | Trajanje |
| --- | --- | --- | --- | --- | --- | --- |
| local | 103 | 103 | 206 | 1 | 413 | 194 ms |
| lan-loopback | 103 | 103 | 206 | 1 | 413 | 365 ms |
| intranet-loopback | 103 | 103 | 206 | 1 | 413 | 334 ms |

U svakom prolazu: 10 directory listinga, 0 blocked grupa, 0 unresolved
datoteka i 0 scanner gresaka. File facts = 412 stat provjera (original,
proxy, XML sidecar i thumbnail) + 1 procitani index. Index dokazuje veze;
sidecar sadrzaj ovdje nije ponovno parsiran niti se otvaraju media streamovi.
Javni scanner ne sadrzi Sony branch: testni pozivatelj registrira
`SonyIndexReader`. Produkcijski Sony reader ostaje cisti XML parser.

SHA-256 prije/poslije: svih 106 XML datoteka u XDROOT-u i obje katalog
publikacije ostale su identicne (108/108). Nema probe/ffmpeg, importiranja,
brisanja ili zapisa u karticu/bazu. Privremeni loopback hostovi zavrsavaju
nakon poziva; ne ostaje aktivan Ingest ili novi servis.

Ponovljivo iz QNC roota:

```powershell
cargo run -p qnc-scanner --example source_groups --locked --offline -- local catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-scanner --example source_groups --locked --offline -- lan-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-scanner --example source_groups --locked --offline -- intranet-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
```

Mjerenja su pojedinacni prolazi s cacheom na jednom racunalu. Loopback nije
provjera fizicki odvojenog LAN/intranet servera ni TLS deploymenta.

## Nije implementirano ovim korakom

- UI/Select prikljucak i upis klipova/postojecih metadata factova u Ingest DB.
  Sljedeci modul mora ponuditi eksplicitni, versioned DB-write ugovor preko
  local/LAN/intranet transporta. Scanner/grupiranje ga ne smiju pozivati.
- Put za neindeksirane kopije, druge index sheme i finalno media/probe
  popunjavanje. Nerijesene datoteke nisu izgubljene ni predstavljene kao
  potvrdeni originali. Jedini probe ostaje u buducem Select/Ingest prolazu.
- Clip ID deduplikacija kroz vise sessiona/kartica; recording identity je
  samo source/root scoped podatak za grouping, ne globalni clip identitet.
- Automatsko ucitavanje binarnih plugina ili poseban RPC host za IndexReader.
  Ovaj trait je javni in-process adapter; serijalizirani ulazi/izlazi mogu
  dobiti odvojeni procesni adapter bez dodavanja app logike u scanner.
- Linux/macOS/ARM izvrsvanje, fizicki LAN/intranet, hot-unplug i sve kamere.

Nema izmjena app/UI/Project koda, Ingest storea/komponente, keyboarda ili
objavljenih katalog baza. `relationships_resolved` oznacava samo potvrdu
veza, nikad potpun media/probe zapis ili uspjesan import. Reader ne dobiva
transport handle, a grupiranje ne poziva reader: kompozicija je u scanneru.
