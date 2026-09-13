# Freeze cijele QNC obitelji

Status: zamrznuto

Datum: 2026-09-13

Razlog: korisnik je izricito zatrazio da se zamrzne cijeli projekt sa svim
modulima i aplikacijama i da se to stanje posalje na GitHub.

## Odnos prema starijim freezeovima

- Project: `AGENTS.md` odjeljak 14 i `docs/11-project-freeze.md`.
- Ingest i moduli koje Ingest koristi: odjeljak 17 i `docs/85-ingest-freeze.md`.
- Ovaj zapis zatvara sve ostalo: shell, preostale crateove, alate, ugovore
  i razvojne dokumente. Ne slabi 14 ni 17.

## Opseg: kod i ugovori, ne radni podaci

Freeze se odnosi na izvorni kod, manifeste, module/application/DB/UI
ugovore, seed, conformance i razvojne dokumente. Ne odnosi se na radne
projektne direktorije, baze ni artefakte. Ti se i dalje smiju zapisivati
kroz vec zamrznute javne write putove.

## Ne smatra se dozvolom

- "nastavi", "idemo dalje", "sredi QNC", "dodaj Story"
- otvoreni `AGENTS.md` odjeljak 8.3
- preview/player/trzaj kao razlog za izmjenu bez imenovanog otkljucavanja

Dozvola mora imenovati tocnu aplikaciju, modul ili putanju i vrstu promjene.

## Zamrznuti scope

Sve ispod root-a `C:\Users\miron\Projects\QNC` sto je razvojni kod:

- `apps/**` ukljucujuci `qnc-app`, `qnc-project`, `qnc-ingest`
- `crates/**`
- `tools/**`
- `contracts/**`
- `seed/**`
- `docs/**` (osim novog freeze/unlock zapisa na izriciti zahtjev)
- root `Cargo.toml`, `Cargo.lock`, `AGENTS.md` (osim freeze/unlock zapisa
  na izriciti zahtjev)

`qnc-ingest-components` ne smije postojati.

## Dopusteno bez otkljucavanja

- citanje i audit
- pokretanje testova i live programa
- zapis poslovnih rezultata kroz postojeci javni write put

## Postupak otkljucavanja

1. Zaustaviti implementaciju.
2. Navesti tocne datoteke.
3. Tražiti izricitu korisnicku dozvolu.
4. Nakon odobrene promjene ponovno vratiti status na zamrznuto ovdje i u
   `AGENTS.md` odjeljku 18.
