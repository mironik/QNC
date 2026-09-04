# QNC architecture model

Status: pocetni ugovor  
Datum: 2026-09-04  
Root: `C:\Users\miron\Projects\QNC`

## Osnovni model

QNC se gradi kao obitelj aplikacija/formi koje pokrece shell. Aplikacije nisu
medusobno povezane aktivnim kodom. Jedina poslovna veza izmedu aplikacija je
baza kroz javni DB contract.

Moduli su gradivni dijelovi aplikacija. Modul je javno dobro unutar QNC
sustava: ne smije imati hardkodirani popis aplikacija koje ga smiju koristiti.
Isti modul smije koristiti vise aplikacija, ali modul ne smije biti tajna veza
izmedu aplikacija.

Zabrane se dijele na dvije razlicite stvari:

- aplikacijska workflow zabrana: sto aplikacija/forma ne smije pokrenuti ili
  koristiti u svojem workflowu
- modulna dependency zabrana: sto modul sam ne smije pozvati, ucitati ili
  pokrenuti

Ni jedna od tih zabrana ne smije postati lista dozvoljenih korisnika modula.

```text
QNC Shell
  -> pokrece Project aplikaciju
  -> pokrece Ingest aplikaciju
  -> pokrece Media Assist aplikaciju
  -> pokrece jednu ili vise Story aplikacija/varijanti

Project aplikacija
  -> koristi svoje module
  -> pise Project DB

Ingest aplikacija
  -> koristi Dir Browser, Media Browser, Media Probe, Filmstrip, Wave module
  -> pise Ingest DB

Story aplikacija / Story varijanta
  -> koristi Timeline, Media Browser, Broadcast Player, Export module
  -> cita Project/Ingest javne DB contracte
  -> pise svoju Story DB

Media Assist aplikacija
  -> koristi neutralne media/editorial/analysis module
  -> cita Project/Ingest javne DB contracte
  -> pise svoju Media Assist DB
```

## Primjer dijeljenog modula

```text
                 local / LAN / intranet transport
        +-----------------------------------------------+
        |                                               |
        |               Timeline modul                  |
        |                                               |
        | prima ulaz -> vraca rezultat                  |
        | ne posjeduje Story DB                         |
        | ne posjeduje Media Assist DB                  |
        | ne povezuje aplikacije medusobno              |
        +--------------------^------------------^-------+
                             |                  |
                             |                  |
                  request/response      request/response
                             |                  |
        +--------------------+--+            +--+--------------------+
        | Story aplikacija      |            | Media Assist aplikacija |
        | pise story.sqlite     |            | pise media_assist.sqlite |
        +-----------------------+            +-------------------------+
```

Primjer upotrebe javnog modula:

```text
Story aplikacija -> Timeline modul
Media Assist aplikacija -> Timeline modul
```

Zabranjena je direktna poslovna veza aplikacija mimo baze:

```text
Story aplikacija -> Media Assist aplikacija
Story aplikacija -> privatni Media Assist kod
Media Assist aplikacija -> privatni Story kod
```

## Aplikacije

Pocetne QNC aplikacije/forme:

- Project
- Ingest
- Media Assist
- Story varijanta 1
- Story varijanta 2
- buduce Story varijante
- buduce QNC aplikacije

Filmstrip, Wave, Broadcast Player, Export, Media Probe, Media Browser,
Timeline, Monitor i Dir Browser nisu aplikacije u ovom modelu. To su moduli.

## UI/layout referenca

Postojeci UI i layout iz `C:\Users\miron\Projects\qnc_v4` obvezna je referenca
za novi QNC i mora se doslovno preslikati.

Prije kodiranja bilo koje forme ili vidljivog modula treba provjeriti
relevantni stari UI:

- shell i tab/layout strukturu
- Project formu
- Ingest formu i source/browser layout
- Media Assist formu/template
- Story forme i njihove varijante
- media browser, timeline, player kontrole, filmstrip/wave prikaz
- fokus, keyboard ponasanje, font, razmake i poravnanja

UI/layout mora biti prenesen kao doslovni vizualni i strukturni baseline:
raspored, redoslijed elemenata, nazivi, font, razmaci, poravnanja, fokus,
keyboard ponasanje i stanja prikaza moraju ostati isti.

Redizajn, reinterpretacija ili uljepsavanje layouta nisu dio migracije. Ako je
odstupanje tehnicki neizbjezno zbog razdvajanja starog monolita na
aplikacije/module, odstupanje mora biti minimalno, zapisano prije implementacije
i potvrdeno live testom.

UI kod i layout kod iz starog projekta smiju se koristiti za doslovno
preslikavanje. Aktivna poslovna logika iz starog koda ne smije se prenijeti bez
razdvajanja na aplikaciju/modul i bez jasnog odobrenja.

## Moduli

Modul moze biti:

- neutralni shared modul koji koristi vise aplikacija
- unutarnji modul jedne aplikacije
- vanjski in-process plugin
- vanjski out-of-process helper/plugin

Modul mora imati:

- ime
- verziju
- capability popis
- javni capability contract bez popisa dozvoljenih korisnika modula
- input contract
- output contract
- dependency boundary / forbidden calls
- state/write policy
- OS/CPU support

Modul ne smije:

- imati hardkodirani popis aplikacija koje ga smiju koristiti
- pisati u baze aplikacija koje ga ne posjeduju
- dijeliti aktivno stanje izmedu aplikacija
- pozivati aplikacije kao workflow
- pozivati druge module ili alate izvan svojeg dependency boundaryja

Primjer: Filmstrip je javni modul, ali njegov dependency boundary zabranjuje
pozive prema `ffprobe`, Media Probe modulu, scanneru i Ingest workflowu.

## Granica odgovornosti

Aplikacija je vlasnik workflowa i baze.

Modul je alat koji aplikacija koristi.

Baza je jedina poslovna veza izmedu aplikacija.

Transport/resolver rjesava pristup lokacijama u local/LAN/intranet okruzenju,
ali nije izvor istine.
