# Shell: prijelaz na sljedecu odabranu grupu

Odobrenje: korisnik trazi prijelaz klikom na postojeci projekt i nakon
kreiranja novog projekta. Oba postojeca Project ulaza ostaju; UI layout se
ne mijenja. Implementacija je u komponenti, owner DB readeru i javnom
desktop adapteru, ne u paint kodu forme.

## Ugovor

- `DesktopNavigation::NextGroup` je prolazni signal bez projektnog payloada.
- Signal se ne zapisuje u DB, datoteku ili trajni red poruka.
- Project komponenta signal priprema samo nakon uspjesnog create/open.
- Shell preuzima signal jednom nakon prikaza hostane povrsine.
- `navigation_sequence()` je javni read-only upit adaptera za navigacijske
  reference iz baze upravo otvorenog projekta. Ne vraca radne postavke,
  media podatke, privatne putanje ili poziv poslovnog workflowa.
- Owner postavlja postojeci QNC URI/resolver binding i otvara bazu read-only.
  Citaju se samo javni sequence/workflow prikazi. Nema migracije ili repaira.
- Podrzan je samo novi javni sequence prikaz s grupama a-z. Nema starog
  workflow fallbacka, izvodjenja grupe iz pozicije ili migracije stare baze.
- Novi zapis mora imati rastuce grupe i najvise jedan izbor po grupi.
- Shell aktivira sljedecu odabranu grupu istim putem kao rucni precac.
- Ako cilj nedostaje, adapter nije dostupan ili citanje ne uspije, ostaje
  trenutna povrsina i prikazuje se greska. Ne bira se zamjenska aplikacija.
- Kraj slijeda nije greska i ne uzrokuje prijelaz.
- Bez shella Project normalno radi; nema obveznog shell procesa ili veze.
- Nijedna poslovna aplikacija ne dobiva podatke od druge aplikacije.
  Sljedeca aplikacija svoje radne podatke mora samostalno citati iz baze.

Ovo je ugovor hostane navigacije, ne novi poslovni servis i ne dovrsenje
LAN/Intranet DB transporta iz nalaza F07. Automatizirani testovi smiju imati
privremene fixture baze; live provjera koristi stvarni QNC razvojni root.

## Verifikacija 2026-09-06

- `cargo test --workspace --quiet`: 150 testova prolazi.
- `qnc-conformance`: all checks passed.
- Izgradjeni su `qnc-app`, `qnc-project` i `qnc-ingest`.
- Stari shell i standalone Project procesi zaustavljeni su po tocnoj
  executable putanji. Novi `target/debug/qnc-app.exe` pokrenut je iz QNC roota.
- Korisnik je u live testu potvrdio: "ok,, tosad radi....". Agent je provjerio
  otvoreni shell, ali nije samostalno dovrsio zasebna UI create/open mjerenja.
- Testovi pokrivaju jednokratno preuzimanje signala, odabranu varijantu,
  preskocene grupe, odbijanje nevaljanog/starog slijeda, read-only DB upit i
  zadrzavanje trenutne forme ako nedostaje ciljna aplikacija ili adapter.
- Iz stvarne registry baze kroz owner komponentu obrisani su korisnicki
  templatei `Breaking news NOVI` i `Breaking news XXXX`: nisu imali novi
  `application_selection` zapis. Sistemski templatei nisu obrisani.
- U trenutku provjere projekti `test1` i `test 2` imali su valjan novi zapis
  a-b i nisu mijenjani ni brisani. Stari projekti iz prethodnog popisa vise
  nisu postojali u registryju; nisu migrirani.
- Ovaj korak ne zatvara LAN/Intranet DB transport niti Ingest citanje radnih
  postavki iz audita. Nisu izvrseni Linux/macOS live testovi.
