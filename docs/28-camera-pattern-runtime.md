# Primjena camera-pattern kataloga

Datum: 2026-09-07. Nastavak docs/27. Ugovori prije implementacije.

## Odvojene odgovornosti

- `qnc-camera-patterns`: read-only snapshot postojecih public DB viewova.
  QNC catalog URI -> resolver -> lokalni read-only SQLite ili JSON odgovor
  storage endpointa. JSON je prijenos istog kataloga, ne druga baza/seed.
- `qnc-source-reader`: dodaje bounded directory listing uz postojece stat/read.
  Nema navigacijskog/browser statea, UI-ja, kamera pravila ili clip uloga.
- `qnc-camera-detector`: primjenjuje podatkovne obrasce kroz source transport.
  Nema app ovisnosti, DB writea, media probea, Sony XML parsera ili importa.
  Ne kopira aktivni scanner/staru aplikacijsku logiku iz v4.

## Katalog

Postojece SQLite publikacije, schema.sql i seed ostaju nepromijenjeni.
Provjeriti application_id, schema_version, tocan fizicki schema opis,
integritet i javni identitet prije citanja public viewova. Ne izvrsavati SQL
iz JSON-a ili iz korisnickog inputa. Snapshot ima URI, contract version,
dataset version, research status, patterns i njihove roots/file rules/
metadata/evidence. Statusi i dokumentirani izvori ostaju dio rezultata.

Reader ne pretvara research_only u tvrdnju da su sve kamere podrzane.
Enabled + observed/documented + poznati root scope znaci kandidat za analizu,
ne certificirani clip parser. Disabled, incorrect i partial ne sudjeluju.
Nepoznat format/shema/semantika su greska, ne fallback na ugradeni seed.
Pristup katalogu je javan modulni API bez liste dozvoljenih aplikacija.

## Transport listing 0.2.0

`source.directory.list`: QNC referenca direktorija + limit broja stavki.
Period (`.`) oznacava ownerov source root i ima source URI, ne OS path.
Odgovor: directory URI, stvarna imena, relativne QNC reference i vrste.
Linkovi/special fileovi se prijavljuju, ne slijede se u rekurziji.
Limit ili necitljiv direktorij ne vraca laznu praznu/potpunu listu.
Nema pisanja, filesystem globovanja kroz shell ili pristupa izvan capability
roota. Isti wire format vrijedi za Local/LAN/Intranet.

Owner transporta eksplicitno postavlja `MatchCase` za izvor; default je
konzervativni Exact, ne pogadanje filesystema iz OS-a. Listing prenosi taj
izbor. Matcher nikad ne mijenja spremljeno ime/reference. Automatsko
otkrivanje filesystem case policyja nije ovaj korak.

## Detekcija kandidata

Caller odreduje scope ulaza: card_relative, recording_relative ili
reel_relative. Ne pretpostaviti da folder klipa predstavlja korijen kartice.
Rootovi istog obrasca su alternative; svi pronadjeni rootovi ostaju u
rezultatu. Obrasci se citaju iz baze, nema switcha na marku/model kamere.

Koristiti globset za segmentne `*` i `?`, a `**` za nula ili vise
direktorija, uz literal separator i bez platform-specific escape defaulta.
Prosirena glob sintaksa izvan ugovora ne prihvaca se.
[GlobBuilder](https://docs.rs/globset/latest/globset/struct.GlobBuilder.html).

Listati samo direktorije potrebne za root/file izraze, uz cache jednog
listing rezultata po direktoriju i limit dubine, direktorija i rezultata.
Nema punog citanja medija. Kandidat zahtijeva root i barem jedan matched
original_candidate. Prazni rootovi i rootovi bez originala ostaju zabiljezeni,
ne pretvaraju se u klipove. Nedostajuci proxy je dozvoljen.
Index, metadata, thumbnail, audio i ostale uloge ostaju odvojene reference.
Uvjeti iz kataloga su opis dokazivanja, ne skripte koje se izvrsavaju.
Uloge su kandidati dok specijalizirani reader ne potvrdi veze.
Ako vise pravila istog obrasca pogodi istu datoteku i ulogu, vraca se jedna
stavka s popisom svih podudarnih pravila. Ne duplicirati clip kandidate.

Vise obrazaca/uloga za istu datoteku je eksplicitna dvosmislenost, ne bira se
prvi po redoslijedu. Popis ne tvrdi da obuhvaca svaki nepoznati file na kartici;
biljezi scope i ogranicenja analize. Nema ffprobe fallbacka.

## Provjera

Testirati objavljena oba kataloga, neispravnu shemu/status/pattern, katalog
preko stvarnog HTTP loopbacka, filter disabled/incorrect, vise rootova,
zamjenske znakove, case policy, nema proxyja, preklapanje uloga, root scope,
greske/limite listinga i nedostajuci original. Zatim read-only stvarna kartica
od source roota kroz katalog -> transport -> kandidati, uz Sony XML provjeru
izricitih veza. UI, aplikacijski manifesti i Project ostaju nepromijenjeni.

## Izvrsena verifikacija

Windows x86_64, 2026-09-07:

- `cargo test --workspace --locked --offline`: 264 testa, svi prolaze.
- Novi ciljani testovi: 8 camera-patterns, 11 camera-detector; source-reader
  sada 16 testova. Testni direktoriji su privremeni, ne kartica.
- Clippy za sva tri modula i njihove testove/primjere: bez upozorenja.
- QNC conformance: all checks passed. Ciljani cargo fmt i git diff --check.
- Objavljena izvorna i nova SQLite revizija citljive bez izmjena. Testovi
  odbijaju modificirane viewove prije izvrsavanja, krivi schema/status,
  nevaljane reference, nedostajuci evidence i nevaljan mrezni odgovor.
- Fixture strukture koriste postojece katalog obrasce Sony, Panasonic P2,
  PANA_GRP, Canon, RED, GoPro, JVC i Blackmagic. To nisu stvarne kartice svih
  proizvodjaca niti certifikacija svih nacina snimanja.
- Testovi potvrduju proxy-only root bez clip kandidata, opcionalni proxy,
  alternativne rootove, `?`, `*`, `**`, case policy, cache, limit/gresku i
  eksplicitnu RED original/proxy dvosmislenost.

Read-only primjer `catalog_source` dobiva privatni binding **korijena
kartice**, ne unaprijed zadani recording direktorij. Zatim katalog -> source
transport -> kandidati -> zasebna Sony XML provjera u primjeru. Produkcijski
detector nema dependency na Sony parser ili media metadata.

| Nacin | Direktoriji listani | Detekcija + katalog | Ukupno s XML provjerom |
| --- | --- | --- | --- |
| local | 10 | 74 ms | 322 ms |
| lan-loopback | 10 | 66 ms | 410 ms |
| intranet-loopback | 10 | 65 ms | 380 ms |

Jednokratna mjerenja na istom racunalu, s OS cacheom; nisu usporedba brzine
stvarnog LAN-a/intraneta. Sva tri nacina vratila su isti nalaz:

- `PRIVATE/XDROOT`: 103 originala, 103 proxyja, 103 XML sidecara, 103
  thumbnaila i 1 MEDIAPRO index; svi su jos role kandidati detektora.
- Zasebni Sony parser potvrduje 103 Material -> original/proxy veze preko
  indeksa, uz 104 XML citanja i 206 media stat poziva. Nema media dekodiranja.
- 103 datuma kreiranja; tocan broj frameova za 103 originala i 22 proxyja.
  Preostalih 81 proxy frame count nije izmisljeno niti preuzeto iz originala.
- 0 XML konflikata i 0 potpunih media-metadata zapisa: kamera ne daje sve
  ugovorene podatke. To nije zavrsen probe niti razlog za probe u detektoru.
- `PRIVATE/M4ROOT` i root `.` obrasca `sony-xdcam-mxf` nemaju
  original kandidate. Prazna root podudaranja ne pretvaraju se u klipove.
- 0 gresaka traversala; 0 dvosmislenosti na ovoj kartici. Sedam coverage gap
  zapisa ostaje vidljivo, ne maskiraju se fallbackom.
- SHA-256 svih 106 XML datoteka unutar stvarnog XDROOT-a i obje SQLite
  publikacije jednak prije/poslije sva tri prolaza (108/108).
- Testni HTTP hostovi su ugaseni i threadovi spojeni po zavrsetku.

Ponovljiv read-only primjer iz root direktorija:

```powershell
cargo run -p qnc-camera-detector --example catalog_source --locked --offline -- local catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-camera-detector --example catalog_source --locked --offline -- lan-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-camera-detector --example catalog_source --locked --offline -- intranet-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
```

`G:\` je samo privatni binding ove Windows testne kartice. Javni output i
reference modula su QNC URI; nije upisan kao runtime default.

## Granica i nastavak

Ovaj korak ne mijenja UI, Project, Ingest komponentu/store, app manifeste,
keyboard ni objavljene katalog baze. Nema probe/ffmpeg procesa, importiranja,
novih klipova u bazi, filma/filmstripa ili automatskog fallback skeniranja
nepoznatih direktorija. Select jos ne poziva ove module.

Sljedeci korak je ugovor source-role/grouping rezultata: podatkovni detektor
daje kandidate, specijalizirani reader potvrduje veze iz indeksa, a datoteke
bez index/sidecar podataka ostaju odvojeno oznacene za jedini buduci probe u
Select/Ingest prolazu. Upis clip identiteta i rezultata pripada Ingest owneru,
ne ovom detektoru. Prije UI povezivanja treba provjeriti taj upis i obradu
nedostajucih podataka; ne dodavati probe drugim modulima.

Nije provjeren fizicki udaljeni server, TLS deployment, Linux/macOS/ARM,
hot-unplug ni sve kamere. Listing cache je ogranicen na jedan poziv; nije
atomski filesystem snapshot. Owner mora osigurati mirujuci izvor za dosljedan
ingest. Link/special stavke u podudarnom putu i prekoraceni limiti daju
nepotpun rezultat, nikad tihi uspjeh. Ne pokusava se certificirati kameru
prema samoj ekstenziji niti izvoditi opisne uvjete iz kataloga kao kod.
