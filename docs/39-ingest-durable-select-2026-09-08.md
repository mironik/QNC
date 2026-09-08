# Ingest: trajni Select i selekcija

Pocetni rez. Naknadni korisnicki zahtjev za inkrementalni Select i prikaz prije
pozadinskog upisa opisan je u docs/40-ingest-incremental-select.md; on zamjenjuje
donji prvotni redoslijed prikaza i zatvara pending live provjeru.

Opseg: javni pristup postojecoj projektnoj bazi, Ingestove vlastite tablice,
trajni rezultat postojeceg Select procesa i DB ponovno ucitavanje/odabir.
Project kod i njegovi ugovori ostaju zamrznuti. Nema UI/layout promjene.

Referenca v4: `qnc-host/src/ingest/db.rs:30` (vlastite tablice u projektnoj
bazi), `qnc-host/src/ingest/store.rs:1342` (odabir iz baze),
`qnc-host/src/project/db.rs:121` (stvarna lokacija projekta iz registra).
Privatni Project pozivi, migracije i fallbacki ne prenose se.

Javni `qnc-work-settings` razrjesava postojeci owner binding baze read-only.
`qnc-ingest-store::content` koristi taj binding kroz isti URI ugovor lokalno
ili preko autentificiranog HTTP transporta. Ne stvara zamjenski projekt.
Postavke i identitet projekta ne mijenjaju se. Vlasnistvo rezultata ostaje
ograniceno na Ingest tablice; ne uvodi se proizvoljni SQL endpoint.

Komponenta salje DB potvrdjene zapise pasivnoj formi. Ponovno ucitavanje
ne poziva scanner/probe; thumbnail se cita preko spremljene URI reference.
Odabir se potvrduje tek nakon DB upisa, izvan UI threada.

Uvezi, kopiranje medija, generiranje Filmstripa i Wavea nisu opseg ovog koraka.
Provjera slijedi: ciljani testovi, conformance, Windows live Select/restart
na stvarnom razvojnom projektu, kartica read-only. Mrezni testovi na loopbacku
nisu dokaz stvarnog LAN/Intranet deploymenta; ne tvrditi neprovjereno.

## Provjereno

- 45 ciljanih testova: 19 Ingest components, 13 Ingest store, 9 work-settings,
  4 source-contract. Conformance prolazi. Standalone build prolazi.
- SQL authorizer odbija upis u Project/tudje tablice, ukljucujuci upis preko
  triggera. Kolizija postojece tablice ne migrira bazu; bootstrap se rollbacka.
- Postojeci WAL se ne mijenja. Ingestov PERSIST rezim i kratke transakcije
  verificirani su s vise writera te Windows delete-deny ACL-om i read-only
  atributom projektne DB datoteke. Delete ACL nije uklonjen.
- Restart, DB selekcija, odaberi sve/ocisti i promjena aktivnog projekta
  provjereni su komponentnim testom. Nestala projektna baza se ne kreira.
- Runtime vise ne stvara globalni `data/ingest_content.db`. Eventualna stara
  razvojna datoteka ne koristi se i ne migrira se.
- Windows live Select na stvarnoj kartici: u aktivnom `qnc_project.db`
  zapisano 98 klipova, 98 probe zapisa i 98 proxy veza; prikazani stvarni
  karticni posteri. Projekt `novi-cjeloviti-1_2aac7f3f86b74ff3a0df9f660252864c`.
- Usporedba svih 10 prethodno postojecih Project tablica i svih tablica globalnog
  Project registra prije/poslije jednaka je. Probe acquisitions/snapshots ostaju
  196/196; ovaj Select nije pokrenuo dodatni probe.
- UI restart/klik provjera naknadno zatvorena uz korisnicko odobrenje, docs/40.

Nije verificirano: stvarni LAN/Intranet server, Linux/macOS live, puni workspace.
Kartica je read-only. Project/Shell izvorne datoteke nisu mijenjane.
