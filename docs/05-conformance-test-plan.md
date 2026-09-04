# QNC conformance test plan

Status: pocetni testni ugovor  
Datum: 2026-09-04

## Cilj

Testovi moraju cuvati arhitekturu:

- aplikacije nisu povezane privatnim kodom
- moduli nisu aplikacije
- moduli su javni QNC resursi bez hardkodirane liste dozvoljenih korisnika
- baza je jedina poslovna veza izmedu aplikacija
- probe se radi samo u Ingest procesu preko Media Probe modula
- modulne zabrane znace sto modul sam ne smije pozvati
- QNC keyboard shortcuti dolaze iz vanjske datoteke i user overridea, ne iz
  hardkodiranog koda
- postojeci QNC v4 UI/layout mora se doslovno preslikati u novi UI
- raw OS path nije javni contract

## Test skupovi

| Test skup | Sto provjerava | Kada se uvodi |
| --- | --- | --- |
| manifest validation | application/module manifest schema, required fields, OS/CPU support | odmah uz manifest/capability modul |
| public module contract | module manifest nema hardkodirani popis aplikacija koje smiju koristiti modul | odmah uz manifest/capability modul |
| app/module classification | Project/Ingest/Media Assist/Story su aplikacije; Filmstrip/Wave/Player/Export itd. su moduli | odmah uz manifest/capability modul |
| forbidden imports | aplikacija ne linka privatni kod druge aplikacije | uz prvi source tree |
| DB ownership | aplikacija pise samo vlastitu bazu/shemu | uz prvi DB contract |
| QNC URI validation | javni DB/media identitet nije raw OS path | uz transport/resolver modul |
| keyboard shortcut catalog | obvezni JSON katalog postoji i UI/input kod koristi action id, ne hardkodirane chordove | uz keyboard/shortcut modul |
| UI/layout mirror guard | UI promjena je doslovna preslika relevantnog `qnc_v4` prikaza ili ima unaprijed odobreno minimalno odstupanje | uz prvu UI formu |
| UI layout contract JSON | `contracts/ui/*.layout.json` mora imati `literal_qnc_v4`, jedan font sustav i OS-neutralne qnc_v4 reference | uz prvu UI formu |
| probe boundary | aplikacijski workflow ne pokrece probe izvan Ingest procesa; Filmstrip/Wave/Player/Export ne pozivaju probe dependency | uz Media Probe modul |
| artifact boundary | Filmstrip/Wave ne rade probe i ne rade fallback sken/probe | uz Filmstrip/Wave modul |
| UI boundary | UI ne radi scan/probe/media obradu | uz prvu UI formu |
| local/LAN/intranet | isti contract radi za local, LAN i intranet URI | uz transport/resolver modul |
| qnc_v4 migration guard | kopirani kod mora biti preslozen u aplikaciju ili modul s ownerom | pri svakom prijenosu iz `qnc_v4` |
| Project app boundary | `apps/qnc-project` ne smije linkati niti pozivati media/probe/filmstrip/wave/player/export runtime | uz Project skeleton |

## Minimalni static checks

Prvi automatski testovi trebaju moci citati source tree i traziti zabranjene
uzorke.

Primjeri zabrana:

```text
module manifest -> hardkodirana lista aplikacija koje smiju koristiti modul
Story aplikacija -> qnc-host/src/ingest/*
Media Assist aplikacija -> qnc-host/src/story/* private workflow
Filmstrip modul -> ffprobe
Filmstrip modul -> Media Probe
Filmstrip modul -> scanner
Wave modul -> ffprobe
Wave modul -> Media Probe
Broadcast Player modul -> ffprobe
Broadcast Player modul -> Media Probe
Export modul -> ffprobe
Export modul -> Media Probe
UI modul -> ffprobe, scanner, render engine
transport/resolver modul -> story workflow, ingest workflow, media assist workflow
UI/input kod -> hardkodirani Ctrl/Cmd/Alt/Shift chord mimo keyboard catalog/user override modela
```

## DB ownership checks

Svaka DB shema mora imati ownera.

Test mora odbiti:

- write u bazu bez ownera
- write u tudu bazu
- app-to-app mutaciju mimo DB contracta
- dupliciranje tudeg kataloga/probea/media lokacije kao nove istine

## Transport checks

Svaki javni DB/media identitet mora biti QNC URI.

Validni primjeri:

```text
qnc://local/db/project_registry
qnc://local/db/ingest_content/source_123
qnc://lan/storage-a/media/source_123/clip_456/original
qnc://intranet/mam-a/db/story/news_story_001
```

Nevalidni javni contract primjeri:

```text
C:\media\clip001.mp4
/mnt/media/clip001.mp4
\\server\share\clip001.mp4
```

Raw OS path smije postojati samo kao privremeni resolver rezultat unutar
procesa.

## Probe checks

Dozvoljeno:

```text
Ingest aplikacija -> Media Probe modul -> Ingest DB
Story aplikacija -> javni Timeline modul -> Story DB
Media Assist aplikacija -> javni Timeline modul -> Media Assist DB
```

Zabranjeno:

```text
Story -> Media Probe
Media Assist -> Media Probe
Filmstrip -> ffprobe
Filmstrip -> Media Probe
Filmstrip -> scanner
Wave -> ffprobe
Wave -> Media Probe
Broadcast Player -> ffprobe
Broadcast Player -> Media Probe
Export -> ffprobe
Export -> Media Probe
```

Ako potreban probe podatak ne postoji u bazi, test mora tretirati to kao gresku
Ingest contracta.

## Live test pravilo

Nakon svakog vidljivog UI ili media koraka mora postojati live test:

- sto se pokrece
- na kojoj putanji
- koji input korisnik bira
- sto se mora vidjeti
- gdje se rezultat zapisuje
- kako se provjerava baza

Bez toga korak nije zavrsen.

Za UI/layout live test dodatno se mora navesti:

- koji `qnc_v4` ekran/layout je koristen kao referenca
- dokaz da su raspored, nazivi, font, poravnanja, razmaci, fokus i keyboard
  ponasanje doslovno preslikani
- sto je promijenjeno, samo ako postoji unaprijed odobreno odstupanje
- zasto je odstupanje bilo neizbjezno

## Prvi testni prioritet

Prvi implementacijski testni redoslijed:

1. manifest schema validation
2. module manifest public-contract validation
3. module/app classification validation
4. UI/layout mirror check
5. QNC URI parser/validator
6. keyboard shortcut catalog validation
7. forbidden app-to-app import scanner
8. forbidden dependency scanner za module
9. forbidden ffprobe/probe scanner
10. DB owner/write policy validator
11. Project app source/dependency boundary scanner
