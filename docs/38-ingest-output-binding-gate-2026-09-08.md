# Ingest: izlazni DB adapter i ispravak storage audita

Datum: 2026-09-08. Polazni HEAD: 9a09e93.
Naknadni implementacijski korak i njegova provjera vode se u docs/39.
Opis nepovezanog adaptera i uklonjenog eksperimenta ispod je povijesni zapis,
ne aktualni status nakon docs/39.
Status: Ingest NIJE zavrsen. Import, Filmstrip i Wave nisu implementirani
ovim korakom. Projects, Shell i druge aplikacije nisu mijenjani.

## Ispravak zakljucka nakon ponovnog pregleda v4

Korisnik je pojasnio: zamrznut je Project kod, ne projektni radni direktoriji.
Prethodni zahtjev za odmrzavanje Projecta radi dodavanja pojedinacnih izlaznih
postavki bio je pogresan. V4 koristi zapisanu lokaciju konkretnog projekta i
standardni relativni raspored. Svako odrediste nije zasebno settings polje.

Izvori iz `C:\Users\miron\Projects\qnc_v4`:

- `qnc-host/src/project/templates.rs:550`: kreira konkretni projektni direktorij,
  zapisuje lokaciju, stvara standardne podmape i sprema postavke u bazu.
- `qnc-host/src/project/db.rs:121`: cita spremljeni `projects.project_dir`.
  `ensure_project_dirs_at` na retku 825 definira standardne relativne podmape.
- `qnc-host/src/ingest/db.rs:30`: Ingest otvara `qnc_project.db` i inicijalizira
  svoje tablice. Poster na retku 1167 ide u
  `ingest/thumbnails/<clip_id>/poster.jpg`.
- `qnc-host/src/ingest/store.rs:1342`: import cita projektne postavke i DB
  selekciju prije odluke o link/copy nacinu; ne trazi nove postavke u formi.
- `qnc-host/src/filmstrip/store.rs:39`: JPEG-ovi idu u `filmstrip/<clip_id>`,
  a `filmstrips` i `filmstrip_frames` u istu projektnu bazu.
- `qnc-host/src/waveform/store.rs:49`: `audio_waveforms` u istoj bazi cuva
  amplitude kanala, bez posebne wave baze ili direktorija za PNG.

U novoj organizaciji primjenu tog rasporeda i upis vlastitih tablica nose
javni storage/DB moduli preko URI-ja i transporta. Ne kopiraju se privatni
v4 Project pozivi, globalni workflow, migracije ni fallbacki za nestali projekt.
Read-only se odnosi na projektne postavke; vlastiti Ingest rezultati su upisivi.
Nije potrebna izmjena Project koda samo radi pisanja rezultata u projekt.

## Provjerena procedura

`qnc-work-settings/src/local.rs` cita oznaku aktivnog projekta iz
`public_app_settings`, njegov identitet iz `public_projects`, privatni
storage binding iz `project_storage_locations` i postavke iz
`public_project_settings.settings_json`. Otvara baze read-only/query-only.

Stvarna razvojna baza sadrzi `storage.ingest_media=link`,
`storage.ingest_profile=field`, `storage.original_policy=ignore_for_fast_news`,
`storage.proxy_policy=link_when_available` i
`playback.input=proxy_if_available`. Nije potrebno ponovno definirati ove
postavke niti uvoditi vezu Ingest -> Projects.

Pregledani su svi javni prikazi stvarne projektne baze i registryja.
`project_settings_kv` je u ovom projektu prazan; `settings_json` sadrzi
postavke. Nije pronadjen zaseban zapis svake izlazne podmape. To NIJE dokaz
nedostajucih Project postavki: v4 primjenjuje standardni storage raspored
unutar zapisane lokacije konkretnog projekta. AGENTS 5.1 sada jasno razlikuje
primjenu tog rasporeda od izmisljanja novih odredista.

V4 referenca: `qnc-host/src/project/templates.rs` zapisuje postavke;
`qnc-host/src/ingest/store.rs::queue_import` cita postavke i DB odabir prije
`resolve_import_plan`. Player i Story zasebno citaju iste spremljene postavke.
Privatni host/Project pozivi iz v4 nisu preneseni.

## Sto je ostalo u kodu

- `qnc-ingest-store::content`: javni projektno ograniceni DB adapter.
- Eksplicitni owner binding datoteke ili autentificirani HTTP transport;
  nema biranja direktorija, imena baze ili putanje iz projektnog korijena.
- Zapis vec spremljenog metadata snapshota, original/proxy veze, serijskog
  broja, naziva izvora, thumbnail URI-ja, datuma klipa i odabira.
- Kratke transakcije, ograniceni odgovori, citanje po stranicama, trajni DB
  statusi redanja/preuzimanja/zavrsetka posla. To NIJE izvrsni import worker.
- Zavrseni metadata zapis ne prepisuje se drugim snapshotom. Ponovna objava
  istog zapisa cuva odabir i import status. Nema scanner/probe poziva.
- Adapter trenutno odbija projektnu bazu postavki i druge postojece sheme.
  To je ogranicenje ovog nepovezanog adaptera, ne arhitekturna zabrana da
  Ingest ima svoje tablice u istoj datoteci. Provjera projektnog identiteta i
  zastita od upisa u tudje tablice moraju ostati; nema migracije.
- PERSIST journal s FULL sinkronizacijom i busy timeoutom radi pod Windows
  delete-deny ACL-om bez otkljucavanja direktorija. To je tehnicka politika
  ovog DB adaptera, ne promjena prava datoteka.
- `qnc-source-contract` dobio je kanonsko citanje spremljenog source URI-ja
  s provjerama scopea, kodiranja i traversal pokusaja.

## Sto je uklonjeno iz ovog pokusaja

Eksperimentalno runtime povezivanje uzimalo je projektni korijen i samo
sastavljalo `<project>/ingest/ingest_content.db`. Ta dodatna baza nije dio
provjerene v4 procedure; nije ju trebalo uvoditi kao zamjensko odrediste.
Takvo povezivanje, novo UI ponovno ucitavanje kataloga i DB odabir iz forme
uklonjeni su iz ovog pokusaja; Ingest runtime i UI vraceni su na polazni kod.
Javni adapter ostaje nepovezan. Potrebno ga je uskladiti s javnim storage
pristupom stvarnoj projektnoj lokaciji i vlasnistvom vlastitih tablica.
Nije ostavljen skriveni fallback niti lokalni settings override.

Stari runtime i dalje stvara globalni `data/ingest_content.db`; to je stari,
jos nepovezani katalog opisan u docs/37, a ne novi projektni javni izlaz.
Njegova zamjena nije proglasena zavrsenom.

## Live i automatizirana provjera

- Eksperimentalna Windows izvedba prikazala je 98 klipova i stvarne karticne
  thumbnaile s kartice G:, koja je koristena read-only.
- Eksperimentalni izlaz imao je 98 `public_clips`, 98 `public_probe_records`
  i 98 `public_clip_proxy` redaka. Odabir jednog klipa bio je spremljen.
- Broj `public_media_acquisitions` i `public_media_snapshots` ostao je 196
  prije/poslije tog Selecta. Ponovni probe nije obavljen u tom prolazu.
- Ovi live rezultati vrijede za UKLONJENO runtime povezivanje, ne dokazuju
  da je trenutno povezivanje zavrseno. Eksperimentalni proces je zatvoren.
- Novostvoreni eksperimentalni DB i journal ostali su u razvojnom projektu
  `C:\Users\miron\Test projekt\novi-cjeloviti-1_2aac7f3f86b74ff3a0df9f660252864c\ingest`.
  Trenutni runtime ih ne koristi. Nije otkljucavana zastita radi njihova
  brisanja; postojeci projekti i kartica nisu brisani.
- U stvarnoj `qnc_project.db` nema dodanih Ingest tablica. Zavrsna usporedba
  nakon Selecta i zatvaranja procesa dala je jednak SHA256
  `256dacc5d3f523b6dab4d0a513ff46b60a49f77661700b29caba912c1860292a`.
  Ranija biljeska prije eksperimentalnog rada ima drugi hash; zato cijeli
  live prolaz ne proglasavamo byte-identicnim bez dodatnog polaznog snimka.
- Konacni ciljani testovi: 40 prolazi (17 components, 10 store, 4 source
  contract i 9 work-settings integration). Conformance: all checks passed.
- Standalone Ingest build prolazi. Cijeli workspace, stvarni LAN server,
  Linux i macOS nisu live verificirani.

## Preostali rad, bez odmrzavanja Projecta

Povucen je prethodni zakljucak da odmrzavanje Project ugovora uvjetuje nastavak.
Ugovor javnog Ingest DB adaptera i njegova provjera fizicke baze jos odrazavaju
pogresnu zabranu zajednicke datoteke; moraju se uskladiti prije runtime upotrebe.
Odvojeno vlasnistvo tablica i zastita Project postavki ostaju obvezni.

Stvarni tehnicki rizik ostaje: `qnc-project-store::set_project_tree_read_only`
rekurzivno postavlja read-only na datoteke; Unix adapter uklanja sve write
bitove i direktorijima. To moze blokirati pisanje rezultata drugih aplikacija,
ne samo brisanje. To je stanje OS dozvola radnih podataka, a ne zabrana razvoja
Ingesta niti dokaz da trebaju nove projektne postavke. U ovom ponovnom auditu
nije mijenjan Project kod, runtime podaci ni OS zastita. Potrebna je provjera
zapisa vlastitih rezultata kroz storage adapter prije tvrdnje o MultiOS radu;
zastita od brisanja ne smije se zamijeniti zabranom radnog upisa.

Slijed ostaje: javni storage binding postojeceg projekta -> trajni Select izlaz i
reload -> import worker koji primjenjuje spremljene politike -> Filmstrip
i Wave kao odvojeni javni generatori iz baze. Bez novog probea i bez
aktivnog koda u UI formi.

Ponovni audit nakon pojasnjenja provjerava kod v4 i ispravlja pravila/izvjestaj.
Nije novi live test i ne mijenja gore navedene granice ranijih rezultata.
