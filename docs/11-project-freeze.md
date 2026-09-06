# Project freeze

Status: zamrznuto

Datum: 2026-09-06
Razlog: korisnik je zatrazio da se Project vise ne mijenja bez izricite dozvole.

## Zatvorena odobrenja za postavke i footer

Naknadno izricito odobrenje 2026-09-06: "isto treba biti i na Project .. treba
pisati naziv selektiranog projekta". Uski opseg: Project desktop povrsina i
javni adapter iznose samo prikazni naziv postojece selekcije u shell footeru.
Nema novog DB citanja/upisa, promjene selekcije, aktivacije ili workflowa.
Zatvoreno nakon ciljane i live provjere: Project u novom `qnc-app.exe`
prikazuje `novi-novi--x5` desno u shell baru. 23 ciljana testa prolaze,
conformance prolazi. UI naziv je samo prikaz postojece selekcije.

Korisnik je 2026-09-06 izricito potvrdio: "da" na dopunu javnog prikaza
postojecim postavkama. Odobrenje se odnosi samo na `settings_json` u
`public_project_settings`, verziju tog read ugovora i ciljani test.
Nema novih poslovnih podataka, promjene aktivacije, UI-ja niti workflowa.
Izvan gore navedenog odobrenja Project ostaje zamrznut.
Odobrenje za javni settings prikaz takoder je zatvoreno: test javnog payloada
prolazi, a reader je procitao novi stvarni projekt bez izmjene njegove baze.
Project je ponovno zamrznut; nema aktivnog odobrenja. Detalji: docs/21.

## Zakljucavanje

Korisnik je 2026-09-06 nakon live potvrde navigacije izricito zatrazio:
"sada zakljucaj project aplikaciju". Prethodno odobrenje je zatvoreno.
Naknadno odobrenje 2026-09-06: "otkljucaj projekt da bi dodao te mogucnosti,
ali ne u projekt, nego u javni modul"; zatim implementacija uz prikaz samo
naziva projekta i datuma. Opce zakljucavanje i dalje vrijedi izvan tog opsega.

Odobreni opseg: javni modul identiteta, njegova Cargo/manifest integracija u
Project owner store, nove tablice/javni prikazi porijekla u obje baze, pouzdan
jedinstveni ID novih projekata i datum u postojecem retku popisa. OS ocitavanje
nije dio forme ili Projecta. Nema migracija/backfilla starih projekata,
session routinga niti LAN prijenosa. Podaci starih projekata vec su uklonjeni
iz globalne baze na korisnikov zahtjev; njihovi direktoriji se ne diraju.

## Zatvoreno odobrenje za metapodatke

Korisnik je izricito potvrdio: otkljucavanje radi javnog modula identiteta i
zapisivanja metapodataka, uz prikaz samo naziva i datuma projekta.
Odobrenje se odnosi na gore navedeni ograniceni opseg i docs/20.
Izvan gore navedenog odobrenja Project ostaje zamrznut.
Verifikacija je zavrsena: 164 testa, conformance, Windows standalone/shell live
prikaz i provjera stvarnih zapisa u obje baze. Detalji i granice provjere su u
docs/20. Status je vracen na zamrznuto; ovo odobrenje vise nije aktivno.

## Prethodni zahvat

Korisnik je 2026-09-06 izricito potvrdio: "da odmrzni projects".
Odobrenje se odnosi na prethodno dogovorene popravke iz Projects audita i
povezivanje templatea s katalogom dostupnih aplikacija, uz zadrzavanje UI-ja.

Katalog kreira samostalan alat iz postojecih registracija. Po naknadnoj
korisnickoj uputi zapis smije biti baza ili JSON; prvi korak koristi JSON.
Projects nije generator kataloga i ne smije otkrivati druge aplikacije iz forme.
Integracija kataloga u Project i izbor po grupama opisani su u docs/17.
Shell automatizacija i njezina live potvrda zapisane su u docs/19.
Preostali nalazi audita nisu dozvola za izmjene zamrznutog Projecta.

## Pravilo

Project aplikacija i njezini ugovori su zamrznuti.
Nema izmjena Projecta bez
izricite korisnicke dozvole u najnovijem zahtjevu.

Ne smatra se dozvolom:

- "nastavi"
- "idemo dalje"
- "sredi QNC"
- rad na Ingest, Story, Media Assist ili drugim aplikacijama
- rad na opcem shellu, modulu ili conformanceu ako promjena dira Project scope

Dozvola mora izricito imenovati Project i promjenu, npr. "otkljucaj Project za
promjenu X".

## Zamrznuti scope

Bez izricite dozvole ne mijenjati:

- `apps/qnc-project/**`
- `apps/qnc-project/qnc-app.json`
- `crates/qnc-project-desktop/**`
- `crates/qnc-project-store/**`
- `crates/qnc-project-desktop-adapter/**`
- `contracts/applications/project.application.json`
- `contracts/databases/project-registry.database.json`
- `contracts/databases/project-workspace.database.json`
- `contracts/ui/project.layout.json`
- Project dijelove `contracts/qnc-keyboard-shortcuts.json`
- Project template/seed dijelove `seed/system_seed.json`
- Project conformance pravila u `tools/qnc-conformance/**`

## Dopusteno bez otkljucavanja

- citanje Project koda
- audit bez izmjena
- pokretanje testova
- pokretanje live `qnc-project.exe`
- pokretanje `qnc-app.exe` radi provjere shell-hosted Projecta
- razvoj drugih aplikacija i modula ako ne mijenjaju zamrznuti Project scope

## Postupak otkljucavanja

Ako buduci rad treba promjenu Projecta:

1. Zaustaviti implementaciju koja dira Project.
2. Navesti tocne Project datoteke koje bi se morale promijeniti.
3. Traziti izricitu korisnicku dozvolu za otkljucavanje.
4. Nakon odobrene promjene ponovno vratiti status na zamrznuto.
