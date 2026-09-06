# Project freeze

Status: zamrznuto

Datum: 2026-09-06
Razlog: korisnik je zatrazio da se Project vise ne mijenja bez izricite dozvole.

## Zakljucavanje

Korisnik je 2026-09-06 nakon live potvrde navigacije izricito zatrazio:
"sada zakljucaj project aplikaciju". Prethodno odobrenje je zatvoreno.
Nema aktivnog odobrenja za promjene Projecta. Nastavak razvoja Ingesta ne
otkljucava Project ni njegove ugovore.

## Prethodno odobrenje

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
