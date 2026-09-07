# Source index: javni zapis potvrdenih grupa

## Granica koraka

`qnc-source-index-contract` je cisti podatkovni ugovor. `qnc-source-index-db`
je javni DB adapter s lokalnim pristupom i transport handlerom. Ne ovisi o
scanneru, Sony parseru, formi, Projectu ili Ingest komponentama.

Rezultat grupiranja ide u zasebnu malu `qnc.db.source_index` bazu. To je
modularni dio Ingest proizvoda: odnosi original/proxy/pomocne datoteke i
dokaz iz indeksa kartice. Nije zamjena za `ingest_content`, registry kartica
ili buduci zapis potpunih media metapodataka. Postojece baze se ne mijenjaju.

Ovaj korak ne povezuje `Odaberi` i ne dodaje runtime ovisnosti aplikaciji.
Ne kreira novu projektnu bazu ili paralelni razvojni projekt. Produkcijski
binding putanje odreduje owner pri kasnijem povezivanju workflowa.

## Ugovor

- Verzija 0.1.0; URI `qnc://local/db/source_index` ili
  `qnc://lan/{authority}/db/source_index` / intranet ekvivalent.
- `source.index.write`: batch ID, source URI, izricite grupe i vec ocitane
  file cinjenice. Najvise 256 grupa, 4096 cinjenica i 4 MiB po zahtjevu.
- Validator provjerava samo dostavljene podatke. Ne cita karticu, ne radi
  stat/scan/probe i ne odlucuje koje datoteke cine klip.
- Sve grupe u batchu moraju proci postojeci cisti grouping contract.
  Blokirana grupa odbija cijeli batch prije transakcije.
- Jedna kratka SQLite transakcija zapisuje batch, grupe i odnose. Ne izvodi
  media ili mrezni I/O unutar transakcije.
- Stabilni `record_id` dodjeljuje DB. Identitet unutar baze je par
  `(recording_root_uri, recording_identity)`. Nije globalni clip ID.
- Ista grupa ponovno daje isti ID. Isti batch ID s drugim sadrzajem ili
  postojeci identitet s promijenjenim odnosima daje konflikt, bez prepisivanja.
- Ista media referenca ne smije pripadati razlicitim zapisima, niti biti
  pomocni dokaz drugog zapisa. Provjera vrijedi i izmedu odvojenih batcheva.
- Prijenos na drugi transport/source URI ne prepisuje identitet automatski.
  Mapiranje prenosivog izvora je zasebna odgovornost, ne heuristika DB modula.
- Cuvaju se sve proxy reference, ne odabire se prva. Proxy nije zaseban zapis.
- Nedostajuca pomocna datoteka ostaje oznacena missing/unavailable.
- `recorded_at_unix_ms` je vrijeme DB zapisa, ne datum snimanja klipa.
- `source.index.read` vraca javni zapis po ID-u, bez privatne putanje.
- Javne SQLite viewove mogu citati druge aplikacije bez Ingest procesa.

## Transport i vlasnistvo

Lokalni pristup ide URI -> resolver -> privatna lokalna SQLite datoteka.
Mrezni pristup koristi POST `/v1/source-index`, JSON i bearer ovlast.
Od docs/31 taj zajednicki mrezni dio implementira javni `qnc-json-transport`;
source-index DB adapter zadrzava svoj nepromijenjeni wire ugovor i DB granicu.
HTTPS je obvezan osim doslovnog loopback hosta za lokalni proxy/test.
Redirecti su zabranjeni; zahtjevi, odgovori i vrijeme cekanja su ograniceni.
Ne prenosi se SQL, naziv aplikacije kao ovlast ili privatna putanja.

Owner bootstrap postavlja DB binding i dvije odvojene ovlasti (read/write).
Mrezni korisnik ne moze sam dodijeliti write ovlast. Modul nema popis
dopustenih aplikacija. Read ovlast ne dopusta write operaciju.
Host/TLS/lifecycle su odvojeni od Ingest workflowa; handler ne pokrece server.
Isti javni modul moze raditi u samostalnom storage/proxy procesu. SQLite
datoteka zivi lokalno na storage hostu, ne otvara se preko SMB/NFS-a.

Otvaranje postojece baze provjerava tocnu shemu; nema migracije ni popravka.
Inicijalizacija je izricita i samo za praznu bazu. WAL, foreign_keys i
busy_timeout vrijede za DB writer. Read-only otvaranje ne stvara bazu.

## Verifikacija

Obvezno: ugovor i granice ovisnosti, ponavljanje zahtjeva, restart/readback,
atomicni rollback, konflikt izmedu batcheva, dvije konekcije, read-only
ovlast, nevaljani URI/schema/payload i lokalni/LAN/intranet loopback.
Stvarna kartica ostaje read-only. Modulni live test zapisuje samo testni
SQLite artefakt izvan kartice. Fizicki LAN i Linux/macOS/ARM ne smiju se
proglasiti provjerenima Windows loopback testom.

## Rezultat provjere 2026-09-07

- 16 novih testova (4 contract + 12 DB/transport), workspace ukupno 307/307.
- Conformance: all checks passed. Ciljani Clippy all-targets s `-D warnings`
  prolazi; ciljani cargo fmt i git diff --check prolaze.
- Stvarna kartica `G:` citana iskljucivo read-only. U svakom nacinu 103 grupe,
  103 proxy reference i 206 povezanih datoteka. Upis ide samo u privremenu
  SQLite testnu datoteku; testni adapter uklanja je pri zavrsetku.
- Local: 461 ms ukupno, 174 ms upis + ponovljeni zahtjevi.
- LAN loopback: 787 ms ukupno, 200 ms upis + ponovljeni zahtjevi.
- Intranet loopback: 729 ms ukupno, 216 ms upis + ponovljeni zahtjevi.
- Ukupno ukljucuje source scanner, DB upis/replay i readback, nije mjerenje
  punog Ingesta ili probea. Ovo su pojedinacna mjerenja, ne benchmark jamstvo.
- Svi ponovljeni zahtjevi vracaju isti receipt bez duplikata. Svi zapisi
  procitani su i nakon zatvaranja writera/storage handlera, bez aplikacije.
- 106 XML datoteka kartice i oba objavljena camera kataloga: SHA-256 prije
  i nakon sva tri testa identican (108 datoteka).
- App/UI/Project/keyboard/seed/runtime store datoteke nisu mijenjane.
- Nije provjeren fizicki LAN/intranet, TLS deployment, Linux/macOS/ARM ili UI
  Select workflow. Nije pokrenut probe niti napravljen filmstrip/wave.

Reprodukcija modulnog live testa:

```powershell
cargo run -p qnc-source-index-db --example source_index --locked --offline -- local catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-source-index-db --example source_index --locked --offline -- lan-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
cargo run -p qnc-source-index-db --example source_index --locked --offline -- intranet-loopback catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite G:\
```

Sljedeci korak: javni zapis camera/media metapodataka s referencom na trajni
source record, uz jasno odvajanje djelomicnih kamera podataka od dovrsenog
jedinog probe prolaza. Tek nakon toga povezati cijeli Select tijek u Ingest
komponenti, ne u formi. Source index modul ne smije preuzeti te odgovornosti.
