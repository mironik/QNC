# Ingest Novi / Sve filter

Odobreno korisnickim zahtjevom 2026-09-08, prije implementacije.

## UI odstupanje od v4

Referenca: qnc_v4/qnc-app/src/qnc_source_dock.rs, import header, Reload.
Na istom mjestu umjesto Osvjezi dolazi dvopolozajni filter Novi / Sve
(engleski New / All). Ostali raspored, font, tema i kartice ostaju isti;
uklanja se prethodna oznaka Postojeci sa svake kartice. Pending/failed
oznake spremanja nisu povijest i ostaju.

Kontrola je pasivni javni qnc-ui-kit obrazac bez Ingest znanja.
Nazivi dolaze iz Ingest UI ugovora; trenutni UI koristi hrvatske nazive.
Ovaj korak ne uvodi novi sustav jezika ili projektne postavke.

Naknadno odobrena UI dopuna 2026-09-08: status s brojem klipova premjesta se
iz desne akcijske grupe uz naziv klipa lijevo. Novi je zelen, Sve plav;
RGB par dolazi iz Ingest layout ugovora, ne iz poslovne logike UI kita.
Korisnik je dodatno precizirao: neaktivni polozaj mora biti bez ispune.
Samo aktivni polozaj ima svoju boju i donju oznaku kako izbor ne bi ovisio
samo o razlikovanju boja.

Verifikacija te dopune: 7/7 UI-kit i 2/2 desktop testova prolaze, ukljucujuci
RGB po modu i razmak brojac/akcije na sirinama 960, 1280, 1920. Build i
conformance prolaze. Windows live prikaz potvrduje lijevi status, dva obojena
segmenta desno i podcrtani aktivni Novi. Pokrenuta je nova verzija Ingesta.
Poslovna komponenta, DB i probe kod nisu mijenjani u toj dopuni.

Zavrsna dorada aktivne boje: 9/9 UI testova potvrduje transparentnu ispunu
neaktivnog polozaja i odgovarajucu boju aktivnog, u oba smjera. Build prolazi.
Windows live prikaz potvrdjuje aktivni plavi Sve i Novi bez ispune. Nova
verzija je pokrenuta; automatizirani klik na Novi blokirao je korisnicki input.

## Ugovor ponasanja

- Sve je pocetni prikaz cijelog ucitanog kataloga.
- Novi koristi vec izracunati previously_seen podatak: prikazuje samo klipove
  koji nisu bili u bazi prije tekuceg Selecta. Nije isto sto i import_status.
- Nakon pokretanja ranije spremljeni katalog pripada postojecim klipovima.
- Filter ne mijenja katalog, preview, selekciju, probe podatke niti bazu.
- Filter radi i tijekom Selecta; novi klipovi dolaze u prikaz odmah.
- Odaberi sve / Ocisti zahvacaju samo vidljive klipove; skrivena selekcija
  ostaje. Status i Uvezi i dalje obuhvacaju cijelu selekciju.
- Klik salje ingest_set_clip_filter intent; komponenta mijenja projekciju.
- Host refresh zadrzava ingest_reload za citanje radnih postavki iz baze.
- Nema promjene transporta: jednak prikaz za Local/LAN/Intranet.

## Verifikacija

- Component filter testovi: 4/4; UI-kit: 7/7; desktop paint: 1/1.
- DB integracijski test potvrduje da Odaberi sve / Ocisti pod Novim cuvaju
  skrivenu staru selekciju i u bazi, te da novi projekt resetira filter na Sve.
- Build qnc-ingest i conformance prolaze. git diff --check prolazi.
- Windows live: novi executable prikazuje 98 spremljenih klipova bez oznaka
  Postojeci. Oba jednaka segmenta Novi / Sve vidljiva su bez prekrivanja.
  Klik Novi odmah prikazuje Nema novih klipova; selekcija Mironik 1483.MXF
  ostaje. Povratak na Sve automatskim klikom nije zatvoren zbog istodobnog
  korisnickog inputa; oba smjera pokrivena su component/paint testovima.
- Stvarni DB prije/poslije: 98 klipova, 196 probe acquisitions, 196 snapshots;
  ista selekcija. Projektne postavke i globalni registry nepromijenjeni.
- Sira provjera nije potpuno zelena: od 27 component testova prvi prolaz je
  prosao, dva ponavljanja imaju 26/27. Postojeci concurrent_select_jobs_do_not_
  duplicate_probe_calls pada u selection_tests.rs:140 (Neki klipovi nisu
  spremljeni u bazu), ne u filteru. Selektiranje/DB writer nije mijenjano ovim
  korakom. Taj nalaz ostaje otvoren, test nije zaobidjen ni uklonjen.
- Nisu izvedeni fizicki LAN/Intranet ni Linux/macOS GUI testovi. Filter nema
  transport ili OS granu; isti component test izvodi Local/Lan/Internet.
