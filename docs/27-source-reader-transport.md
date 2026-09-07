# Source reader transport

Datum: 2026-09-07. Nastavak docs/26, uski preduvjet za Select.

Naknadni korak istog datuma: aktualni contract/crate je 0.2.0. Dodan je
`source.directory.list`, source root referenca `.` i eksplicitni case policy.
Potpuni ugovor i provjera dodatka su u docs/28. Slijedeci opis 0.1.0 i njegovi
rezultati ostaju povijesni zapis prethodnog koraka, ne zasebni runtime.
SourceReference i wire podatkovni tipovi potom su izdvojeni u javni cisti
`qnc-source-contract` (docs/29). Reader ponovno izvozi iste tipove; zadrzan
je isti 0.2.0 wire format, bez alternativnog lokalnog validatora.

## Granica prije implementacije

Postojeci resolver vraca endpoint, ali ne cita izvorne dokumente. Novi javni
`qnc-source-reader` koristi taj resolver i daje samo `source.file.stat` i
`source.text.read`. Nije Dir Browser, scanner, kamera detector, probe ni
aplikacija. Ne pise u izvor ili bazu, nema app allowlist ni UI ovisnosti.
Sony reader ostaje cisti parser: prije njega transport procita XML i potvrdi
reference originala/proxyja. Poslovni odabir i upis ostaju buduca komponenta
Ingesta; ovaj korak ih ne prikazuje kao implementirane.

## Ugovor 0.1.0

- Izvor: `qnc://local/source/{id}`, `qnc://lan/{authority}/source/{id}` ili
  `qnc://intranet/{authority}/source/{id}`. ID je ownerova oznaka izvora,
  ne identitet fizicke kartice izracunat iz slova diska.
- Referenca: source URI i dekodirana relativna referenca unutar tog izvora.
  Jedini separator je `/`. Nema raw OS putanja, `..`, ADS, backslasha,
  query/fragmenta ni dvostrukog URI dekodiranja.
- Izlazni URI: `{source_uri}/file/{percent-encoded-relative-segments}`.
  To je lokacijska referenca, ne clip ID niti dokaz nepromjenjivosti medija.
- Stat vraca URI, vrstu (datoteka/direktorij) i velicinu datoteke.
  Ne otvara/dekodira media streamove i ne daje media probe metapodatke.
- Read vraca isti opis i UTF-8 tekst, maksimalno 8 MiB ili manji caller limit.
  Dokument se cita kroz isti otvoreni handle; nepotpuno/izmijenjeno citanje
  ne smije biti predstavljeno kao potpuni dokument.
- Mreza: POST `/v1/source/read`, versioned JSON request/reply, bearer token.
  Klijent zahtijeva HTTPS osim loopback testa, ne slijedi redirecte i
  provjerava source/URI, operaciju, verziju i velicinu odgovora.
- Endpoint dobiva samo owner-konfigurirani izvor, ne proizvoljnu OS putanju
  od klijenta. Greske ne vracaju privatne putanje ili tokene.
- HTTP handler je javni modulni adapter za storage proces. Nije Ingest
  servis i ne zahtijeva pokrenut Ingest. TLS/reverse proxy i upravljanje
  korisnicima pripadaju deploymentu; token odobrava samo konfigurirani izvor.
  Host/proxy mora ograniciti broj veza i vrijeme citanja zahtjeva. Klijent
  ima 10-sekundni timeout; handler nije samostalan produkcijski HTTP server.
  Kod reverse-proxy podputanje proxy uklanja prefix prije javnog endpointa.

## Lokalna granica

Privatni binding direktorija postoji samo u transport adapteru. Otvaranje
potom koristi capability-root iz `cap-std`, ne provjeru string prefixa pa
ponovno otvaranje apsolutne putanje. Time relativne i symlink reference ne
dobivaju pristup izvan ownerova izvora. Biblioteka dokumentira ovaj model:
[cap-std filesystem sandbox](https://github.com/bytecodealliance/cap-std).

Izvor za ingest je read-only za ovaj modul. Owner mora izvoziti odgovarajuci
direktorij, ne cijeli OS root bez potrebe. Modul ne jamci da neki drugi
proces ne mijenja sadrzaj; stat nije content hash niti trajni snapshot.

## Verifikacija prije nastavka

Testirati lokalni i stvarni HTTP loopback poziv za LAN/Intranet URI-je,
autentikaciju, pogresan izvor/odgovor, limite, invalid UTF-8, missing file,
traversal i link izlaz iz izvora. Zatim Sony reader preko transporta nad
stvarnom karticom read-only, bez ffprobe i bez upisa u poslovne baze.

UI, Project, Ingest runtime manifest i Select ostaju nepromijenjeni dok
ovaj transport preduvjet ne prodje provjeru. Jos nema scanner/catalog runtime
citaca, jednog probe prolaza ni content DB upisa. Ne uvoditi lokalnu
zaobilaznicu da bi se prije toga prividno popunio UI.

## Rezultati 2026-09-07

- `cargo test -p qnc-source-reader --locked --offline`: 13/13 testova.
- `cargo test --workspace --locked --offline --quiet`: 242/242 testova.
- Clippy za novi modul s `--all-targets -- -D warnings`: prolazi.
- QNC conformance s apsolutnim rootom: sve provjere prolaze.
- Stvarni HTTP testovi potvrduju isti rezultat za LAN i Intranet URI,
  izolaciju source bindinga, token, limite, invalid UTF-8, request/response
  identitete, verzije, operacije i zabranu redirecta.
- Windows test stvorio je junction prema zasebnom testnom direktoriju;
  `stat` i `read_text` odbili su izlaz iz izvora. Nema preskocenog testa.
  Odgovarajuci Unix symlink test postoji, ali ovdje nije izvrsen.

Primjer `crates/qnc-source-reader/examples/sony_source.rs` radi transportnu
provjeru prije parsera. Ne sadrzi scanner ili detector: recording root je
izriciti argument testu, a `MEDIAPRO.XML` je poznati Sony ulaz iz docs/26.
Stvarni original/proxy URI-ji sada dolaze iz transporta nakon provjere vrste
datoteke, ne iz sintetskih media bindinga kao u prvom XML-only testu.
Samo clip ID-evi su privremene oznake provjere; nisu zapisani u bazu.

Read-only nad dostupnom karticom, tri uzastopna debug prolaza:

| Nacin | Originali/proxyji | Procitani XML-i | Media stat | Trajanje |
| --- | --- | ---: | ---: | ---: |
| Local | 103/103 | 104 | 206 | 1179 ms |
| LAN URI, stvarni HTTP loopback | 103/103 | 104 | 206 | 281 ms |
| Intranet URI, stvarni HTTP loopback | 103/103 | 104 | 206 | 315 ms |

Vrijeme ukljucuje citanje kroz adapter, stat, XML parsiranje/mapiranje i
provjeru metadata ugovora. Redoslijed i OS cache utjecu na mjerenje; ovo nije
dokaz da je mreza brza od lokalnog pristupa. Nije mjerena stvarna mreza,
probe ili ukupno trajanje Selecta.

U sva tri prolaza: 103 datuma snimanja, 103 originalna exact frame podatka,
22 proxy exact frame podatka, 0 konflikata i 0 potpuno popunjenih metadata
ugovora. Za 81 proxy nedostajuci podaci nisu naslijedeni od originala.
SHA-256 svih 104 XML datoteka prije i nakon sva tri prolaza je identican.
Media datoteke su samo statane, bez citanja/dekodiranja streamova.
Nije bilo ffprobe/FFmpeg poziva, upisa na karticu ni upisa u poslovne baze.

Nije verificirano: udaljeni server, TLS deployment, Linux/macOS/ARM
izvrsavanje, konkurentne izmjene izvora, puni scan/Select/probe/import i UI
live. Public source ID jos ne rjesava fizicki identitet/deduplikaciju kartice.

## Sljedeci rez

Runtime citac postojecih camera-pattern DB viewova treba odrediti recording
root i index/sidecar obrasce kroz transport; ne hardkodirati Sony raspored
u Ingest formu. Nakon toga komponenta moze spojiti Select, original/proxy
grupiranje, jedini probe za nedostajuce podatke i trajni content zapis.
Ovaj reader ne zamjenjuje taj redoslijed niti tvrdi da je Select dovrsen.
Za mreze s latencijom potrebni su ograniceni batch zahtjevi prije tvrdnje o
brzini punog Selecta; trenutni test salje pojedinacne stat/read zahtjeve.
