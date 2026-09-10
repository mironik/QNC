# Ingest: audit transporta, projektnih baza i izvrsnog puta

Vrijeme presjeka: 2026-09-08, oko 23:45 CEST.
Root: `C:\Users\miron\Projects\QNC`.
Git HEAD: `875d981`; radno stablo NIJE cisto. Audit ukljucuje zatecene
necommitane player i Ingest izmjene, ne samo Git verziju.
Referenca: `C:\Users\miron\Projects\qnc_v4\AGENTS.md` i postojeci kod.

Ovo je audit i prijedlog, ne odobrenje implementacije. Project i Ingest
ostaju zakljucani. Izvorni kod, konfiguracija i pravila nisu mijenjani.
Izravne SQL provjere bile su read-only uz `query_only`; nije pokrenut stvarni
ffprobe, import ni media player. Kartica je koristena samo za citanje.
Na korisnikov zahtjev pokrenuti su samostalni Project i Ingest; njihovo
vlastito startup ponasanje i korisnikovi novi projekti nisu audit SQL upisi.
Jedina nova izvorna datoteka ovog audita jest ovaj izvjestaj.

## Sazetak incidenata

1. Project -> Ingest navigacija: u `target/release` nedostajao je
   `qnc-project.exe`. Shell provjerava prisutnost izvrsne datoteke i za
   polaznu aplikaciju. Izgradjen je samo nedostajuci executable iz postojeceg
   koda; korisnik je potvrdio da Project ponovno radi. Nije vracana starija
   verzija izvornog koda.
2. `Source transport unavailable`: korisnik prijavljuje gresku u postojecem
   shell procesu, ali ne u novom samostalnom Ingestu. Svjezi javni source
   reader/scanner uspjesno cita istu karticu. Uzrok starog procesa nije
   dokazan do konkretnog OS error koda; postoje dokazivi lifecycle i
   dijagnosticki nedostaci opisani u F2/F3. Ne tvrditi da je mreza pala.
3. Samostalni Ingest: korisnik potvrduje da browser radi, a `Odaberi` ne.
   Na snimci upravo tog prozora vidljivo je
   `attempt to write a readonly database`. Projektne postavke postoje i
   citljive su. Blokada je dalje u otvaranju/zapisu Ingest sheme, ne dokaz
   nedostatka Project postavki. F1 je prvi prioritet.

## Stvarni podaci

- Konfiguracija postoji: `data/ingest-transport.json`, verzija `0.1.0`.
  Dva izvora: C:/ i G:/, oba Local; nema konfiguriranog LAN/Intranet izvora.
- G: je exFAT, serijski broj `DE666C9F`, jednak konfiguraciji `de666c9f`.
  U korijenu su `PRIVATE` i `System Volume Information`.
- Camera catalog: `camera-patterns-2026.09.07.1.sqlite`.
- Javna read-only provjera kartice: 98 original/proxy grupa, 98 proxyja,
  196 povezanih pomocnih datoteka, 393 file facts, jedan index XML,
  10 listanih direktorija, nula issues/blocked/unresolved.
- `data/ingest_source_index.db`: 98 source records, 196 media references,
  294 support references, 56 write receipts.
- `data/ingest_media_records.db`: 98 media heads, 196 snapshots,
  295 evidence documents, 196 acquisition zapisa. Broj nije tvrdnja da je
  svih 196 acquisition ishoda uspjesno; njihove ishode ovaj SQL presjek
  nije zasebno klasificirao.
- Projekt `novi-cjeloviti-1` ima 98 `clips` i 98 `probe_records`.
  Noviji `sony fx3` i `jjjjjjjj` imaju Ingest shemu, ali nula klipova;
  drugi pregledani novi projekti jos nemaju Ingest shemu.
- Pregledani projekti imaju vlastiti `qnc_project.db`, zapisane postavke i
  valjan slijed grupa Project a -> Ingest b. Aktivni projekt jest podatak
  registra. Korisnik je tijekom audita stvarao/otvarao projekte, pa naziv
  aktivnog projekta nije stalan rezultat ovog izvjestaja.
- Stvarni DB-ovi koriste WAL. Pregledane `qnc_project.db-wal` i
  `qnc_project.db-shm` datoteke imaju ReadOnly atribut. Kod nekih je vec
  uklonjen ReadOnly s glavne DB datoteke, ali nije s pratecih datoteka.
- Na C: je pri prvom presjeku bilo oko 3.05 GiB slobodno. To je operativni
  rizik za build/baze; nema dokaza da je disk bio pun pri prijavljenoj gresci.

## Nalazi

### F1 - P1: delete-lock i SQLite radne datoteke nisu uskladeni

Project `lock_project_dir` rekurzivno postavlja ReadOnly svim datotekama.
To radi i za vec registrirane projekte pri pokretanju/otvaranju, ne samo
za novostvoreni projekt. Time zahvaca baze i postojece WAL/SHM datoteke
koje drugi owneri moraju koristiti za vlastite rezultate.

Ingest `enable_owner_write` na Windowsu uklanja ReadOnly samo s glavnog DB
filea. Postojeci WAL rezim se zadrzava; WAL/SHM prava ne popravljaju se.
Zateceni atributi i prikazana SQLite greska potvrduju stvaran nesklad.
Tocan SQLite extended error code i prva neuspjela datotecna operacija nisu
uhvaceni; nije radjen probni upis u stvarne baze radi ovog audita.

`catalog::load` otvara `Access::ReadWrite` prije vracanja kataloga i radnog
plana. Neuspjeh se propagira kao `settings_failed`, brise prikaz i blokira
Select iako su same projektne postavke citljive. Popravak nije nova baza,
novi Project podatak, migracija niti uklanjanje delete-lock zastite.

Lokacije: [Project lock](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:963),
[rekurzija](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:976),
[Ingest write prava](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/content/database.rs:478),
[ucitavanje](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/catalog.rs:32).

Test gap: `windows_delete_denied_directory_supports_durable_content_writes`
stavlja ReadOnly glavnoj bazi, ne postojecem WAL/SHM paru. Zasebni WAL test
nema ReadOnly atribute. Njihov zajednicki prolaz nije live dokaz ove kombinacije.

### F2 - P1: jedan nedostupan izvor rusi cijelu transport browser inicijalizaciju

`SelectionConfig::browser` otvara sve konfigurirane izvore i prvim `?`
odustaje od cijelog rezultata. `with_store_root` tada ne zadrzava ni
SelectionConfig ni transport browser. Vidljiv ostaje lokalni OS browser,
ali potvrda trazi transport browser koji nije napravljen. Nedostupna jedna
kartica moze onemoguciti i drugi ispravan izvor, ukljucujuci mrezni izvor.
Nema automatskog ponovnog ucitavanja te konfiguracije na kasniji Select.

Lokacije: [browser init](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/selection_config.rs:232),
[komponenta](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:390),
[potvrda](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:850).

### F3 - P2: browser ne obnavlja source handle, a greska gubi uzrok

LocalSource zadrzava otvoreni `Arc<Dir>`. Browser se izgradi jednom;
`browse_registered` klonira istu sesiju/handleove. `roots()` samo vraca
registracije, ne provjerava niti ponovno otvara uredaj. Reload radnih
postavki ne gradi novi transport browser.

Zato nema pouzdanog postupka oporavka nakon vadjenja/ponovnog spajanja
kartice. Stari handle je moguce objasnjenje razlike shell/standalone,
ali nije dokazani konkretni uzrok ovog incidenta bez originalnog OS errora.
Svjezi reader je u ovom auditu radio sva tri read-only testa.

`io_error` sve OS greske osim NotFound/PermissionDenied pretvara u
`Unavailable`. Nestaju operacija, javni source URI i OS error code.
Za lokalni izvor korisnik tako dobiva naizgled mreznu gresku.

Lokacije: [handle](C:/Users/miron/Projects/QNC/crates/qnc-source-reader/src/local.rs:16),
[mapiranje greske](C:/Users/miron/Projects/QNC/crates/qnc-source-reader/src/local.rs:227),
[browser roots](C:/Users/miron/Projects/QNC/crates/qnc-dir-browser/src/transport.rs:36),
[klon sesije](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:945).

### F4 - P1: Uvezi jos nije izvrsni workflow

`INGEST_IMPORT_SELECTED` izricito vraca `Media import jos nije implementiran.`
Store ima queue/claim/finish metode, ali komponenta ih ne koristi za import.
Projektni `storage.ingest_media` i odredista jesu procitani u WorkPlan;
to ne znaci da su link/copy, poster-copy i zavrsetak importa izvrseni.
Select/detektirani katalog i dovrseni import nisu isto.

Lokacije: [dispatch](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:912),
[plan iz postavki](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/work_plan.rs:24),
[queue API](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/content/transport.rs:160).

### F5 - P1: jedan globalni aktivni projekt nije izolacija radnih stanica

Reader cita `public_app_settings.active_project_id` bez korisnika ili radne
stanice u read zahtjevu. Na dijeljenom registru sve stanice vide isti aktivni
projekt. Identitet porijekla projekta ne izolira trenutni odabir stanice.
Select ponovno cita taj globalni zapis. Player barem odbija drugaciji
workspace, ali to ne rjesava samostalni paralelni rad razlicitih projekata.
Ovo je potvrdeno svojstvo ugovora, ne izveden test dviju fizickih stanica.

Lokacije: [active read](C:/Users/miron/Projects/QNC/crates/qnc-work-settings/src/local.rs:84),
[remote request](C:/Users/miron/Projects/QNC/crates/qnc-work-settings/src/lib.rs:185).

### F6 - P2: dio mreze postoji kao modul, ne kao potpuna konfiguracija aplikacije

Source reader, source/media DB i content DB imaju mrezne ugovore/adapterske
testove. Medjutim, `IngestStore::open` uvijek kreira lokalni registry i nema
remote backend za kartice/source sessione. Stvarna konfiguracija ima samo
C:/ i G:/; nije konfiguriran fizicki LAN/Intranet endpoint.
Work-settings helper servira samo svoj read endpoint, ne cijeli niz drugih
DB/source/stream endpointa. Potreban je provjeren deployment, ne URL iz UI-ja.
Postojeci loopback testovi ne dokazuju kompletnu LAN instalaciju.

Lokacije: [registry](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/lib.rs:65),
[local-only otvaranje](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/lib.rs:266),
[settings helper](C:/Users/miron/Projects/QNC/crates/qnc-work-settings/src/main.rs:39).

### F7 - P2: prikaz pri ucitavanju ceka sve thumbnaile

`catalog::load` skuplja cijeli katalog i serijski cita/dekodira thumbnaile
prije nego worker vrati jedan Loaded rezultat. Stari spremljeni zapisi ne
pojavljuju se postupno. Na sporom/offline izvoru, osobito mrezi, postavke i
cijeli prikaz cekaju. To nije isto sto i vec implementiran postupni prikaz
tijekom novog Selecta. Slike nisu trajno prenesene na projektno odrediste.

Lokacija: [hidracija](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/catalog.rs:36).

### F8 - P2: incremental Select i dalje skenira i zapisuje postojeci source index

Prije preskakanja Final zapisa radi se `scan_roles` i source DB write za
sve grupe, s novim batch UUID-em. Time svaki ponovni Select dodaje receipts
i ponavlja otkrivanje/stat/index obradu. Nema novog probea za Final zapis,
ali to nije postupak bez ponovnog citanja/zapisa postojecih grupa.
Source index takodjer odbija promijenjen postojeci group JSON kao Conflict;
promjena availability/support odnosa nema razradjen revision workflow.

Lokacije: [Select](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/selection.rs:126),
[write prije skipa](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/selection.rs:181),
[immutable group](C:/Users/miron/Projects/QNC/crates/qnc-source-index-db/src/database.rs:135).

### F9 - P2: poruke i odbijene akcije nisu dosljedno vidljive

Ingest UI dispatch koristi samo `request_repaint`, ne prikazuje returned
rejection message ako komponenta nije istodobno zapisala view error/message.
Primjer: rani izlazi `start_selection` vracaju rejected bez view poruke.
Neke jos nespojene akcije (poster approve/audio lane) zavrsavaju u opcem
accepted odgovoru, iako nisu izvrsene. To stvara dojam da gumb ne radi.

Shell pak daje prednost stalnom app `footer_status()` pred vlastitom
navigacijskom greskom. Zato nedostajuci executable nije jasno prikazan u
footeru, premda ga `consume_navigation` zabiljezi kao gresku.

Lokacije: [Ingest dispatch](C:/Users/miron/Projects/QNC/crates/qnc-ingest-desktop/src/app.rs:58),
[start selection](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:978),
[shell footer](C:/Users/miron/Projects/QNC/apps/qnc-app/src/main.rs:458).

### F10 - P2: player integracija ne dokazuje trenutni prekid u svim uvjetima

`Player::close` odmah resetira projekciju i poveca generaciju, ali stvarni
child proces zaustavlja worker tek nakon tekuceg blokirajuceg poll zahtjeva.
HTTP deadline je 2 s; graceful Shutdown i cekanje izlaska mogu dodati vrijeme.
Stara sesija se uklanja prije pripreme nove, ali to nije dokaz trenutacnog
prekida zvuka pri kliku pod zastojem transporta. U ovom auditu playback
nije pokretan niti su mijenjane sesije koje korisnik testira.

Lokacije: [worker i close](C:/Users/miron/Projects/QNC/crates/qnc-player-client/src/lib.rs:107),
[shutdown](C:/Users/miron/Projects/QNC/crates/qnc-player-client/src/connection.rs:219).

## Pravila i granice

- Project zapisuje postavke, javni SettingsReader ih cita `query_only`;
  Ingest primjenjuje DB-derived WorkPlan i vlastiti content u tom projektu.
  Nema Ingest -> Project app/store ovisnosti. Nije potrebno izmisljati nove
  putanje ili nove projektne postavke da bi se rijesio F1.
- Forma ne izvrsava scan/probe/DB posao; posao je u komponentama/modulima.
  Shell koristi javne desktop adaptere. Ovdje nije dokazan novi poslovni monolit.
- Final media zapisi i trajni acquisition claimovi cuvaju probe-once pravilo.
  Proxy ostaje reprezentacija originala. Novi player ulaz cita spremljene
  podatke, ne radi ffprobe. Nije provjerena svaka moguca media varijanta.
- Camera patterns su podatkovni, ali konkretni Ingest adapter registry ima
  samo Sony reader. Postojanje obrasca nije implementacija parsera svake kamere.
- Filmstrip, Wave i pasivni timeline jos nisu zavrsen live lanac. Ne zatvarati
  taj korak na temelju player manifesta ili samog postojanja tablica.
- Trenutni workspace ima 52 clana, 46 module manifesta, 5 application
  manifesta i 8 DB manifesta. Stare brojke 41/36 nisu trenutno stanje.
- Root AGENTS ima eksplicitan Project freeze; najnoviji korisnikov Ingest
  freeze vrijedi neovisno o tome sto nije posebno zapisan u root datoteci.
  Referentni v4 AGENTS eksplicitno zakljucava i Ingest, selection i MediaProbe.
  Audit nije ovlast da se ista od tih granica mijenja.
- Uvjet iz AGENTS §16 da scanner/probe moraju postojati prije deklariranja
  manifesta nije sam po sebi trenutni konflikt: implementacije postoje.
  To ne znaci da je kompletan ingest workflow zavrsen.

## Verifikacija u ovom prolazu

- 74 lib testa: Dir Browser 9, Ingest components 29, Ingest store 16,
  player client 3, source reader 17. Svi prolaze, serial test harness.
- 9 integracijskih work-settings testova prolazi. Ukupno 83 testa.
- `qnc-conformance`: all checks passed.
- Read-only `source_groups` Local: 98 grupa, bez gresaka, 1147 ms.
- Isti javni put LAN-loopback: 98 grupa, bez gresaka, 1070 ms.
- Intranet-loopback: 98 grupa, bez gresaka, 136 ms.
  Cache/order utjecaj nije kontroliran; to nije usporedni benchmark mreza.
- Project/shell izvorni scope identican HEAD-u. Korisnik je potvrdio
  popravljenu navigaciju nakon izgradnje nedostajuceg Project executablea.
- Korisnik potvrdio da standalone Project kreira projekte. Standalone
  Ingest otvoren; korisnik potvrduje da nema source transport greske, ali
  Select ne radi. Read-only snimka prikazuje SQLite readonly gresku.

Nije izvedeno: ponovno spajanje kartice radi reprodukcije starog handlea,
stvarni upis u zakljucani WAL radi extended error tracea, novi ffprobe,
import, puni workspace test suite, fizicki LAN/Intranet s vise stanica,
Linux/macOS live test, ponovno player AV mjerenje. Ostali testovi nisu
zamjena za te provjere.

## Redoslijed nastavka uz posebno odobrenje

1. F1: uskladiti SQLite owner write pristup i OS delete-lock. Sacuvati
   zastitu od slucajnog brisanja i zabranu pisanja Project postavki.
   Test mora spojiti stvarni Project WAL/SHM + ReadOnly + ponovni Project
   open/startup dok Ingest radi. Ako treba dirati Project lock rutinu,
   najprije izricito odobriti upravo taj uski opseg; ne odmrzavati formu.
2. F2/F3/F9: po-izvorna dostupnost, obnova browser transport sesije i
   strukturirana greska s operacijom/javnim URI-jem/OS kodom, bez tajni.
   Live: novi proces, postojeci proces, izvadjena/vracena kartica,
   jedan nedostupan i jedan dostupan izvor.
3. Ponoviti Select u novom i postojecem projektu, potvrditi broj klipova,
   spremljene rezultate i isti izvor bez drugog probea. Tek potom Uvezi.
4. Izolacija aktivnog projekta po stanici i stvarni LAN deployment zaseban
   su DB/transport ugovor; ne pokusavati rijesiti to shell memorijom.
5. Player, Filmstrip i Wave dalje tek nakon stabilnog osnovnog Ingest puta
   i izricitog odobrenja za odabrani zakljucani opseg.
