# QNC - ROOT ZAKON

Ovaj file nije preporuka, podsjetnik ni audit. Ovo je obvezni projektni zakon
za novu QNC obitelj aplikacija. Svaki audit, plan, implementacija, test i
popravak mora prvo provjeriti i slijediti ovaj dokument.

Ako kod, plan ili prethodna odluka nisu u skladu s ovim dokumentom, vrijedi
`AGENTS.md`. Ne izmisljati zaobilazna rjesenja, ne uvoditi lokalne default
tokove i ne graditi male monolite pod drugim nazivom. Ako je za rad potrebna
promjena pravila, prije koda mora se traziti izricita korisnicka dozvola i
promjenu zapisati ovdje.

Forme su samo layout i UI. Kompletan aktivni kod mora biti u uskim javnim
modulima ili generickim javnim UI komponentama, s jasnim ugovorom i granicom
odgovornosti. Aplikacijski umbrella component crateovi nisu dozvoljeni.
Posebno: `qnc-ingest-components` ne smije postojati ni kao runtime crate ni
kao javni arhitekturni sloj. Naziv `components` ne smije biti izgovor za novi
centralni kontroler koji u sebi skuplja browser, settings, katalog, select,
player, filmstrip, wave ili druge workflowe.

Root projekta: `C:\Users\miron\Projects\QNC`  
Stari referentni projekt: `C:\Users\miron\Projects\qnc_v4`

Ako pojedina aplikacija kasnije dobije svoj `AGENTS.override.md`, taj override
smije dodati stroza lokalna pravila, ali ne smije oslabiti ova root pravila.

**Temeljna odrednica razvoja: sve QNC poslovne aplikacije rade u okviru
projekta i moraju poznavati njegove postavke kroz bazu. Project aplikacija
zapisuje opce projektne podatke, template izbor, lokacije i postavke u bazu
svakog projekta; ne zna za Ingest, Story, Media Assist, Filmstrip, Wave,
Broadcast Player ili njihove workflowe. Sve ostale aplikacije i moduli te
zapise citaju i primjenjuju kroz javni DB/transport ugovor.**
To ukljucuje odredista direktorija i baza te pravila rada i pohrane potrebna
pojedinoj aplikaciji. Pravilo vrijedi za sve postojece i buduce aplikacije,
ne samo za Ingest.
Jedina poslovna veza je DB zapis, ne Projects aplikacija, shell ni zajednicko
runtime stanje. Citanje je read-only kroz javni DB/transport ugovor, jednako
za Local/LAN/Intranet. Potrosaci ne uvode vlastite zamjenske projektne postavke.
Prije zahvata u bilo kojoj aplikaciji obvezna je provjera iz odjeljka 4.1.

## 0. STROGA PRAVILA (vrijede prije svega ostalog; korisnik ih je ponovio vise puta)

1. **Projekt i njegova baza su jedini izvor svih trajnih podataka i JEDINA VEZA izmedu procesa i formi.**
   Sve sto je trajno (klipovi, odabir, statusi, postavke, putanje, sličice, artefakti, slijed aplikacija)
   cita se iz baze aktivnog projekta i zapisuje u nju kroz javni write transport vlasnika. Sto forma ili
   proces treba znati o drugom procesu ili formi (rad, stanje, zauzetost, najam, rezultat), zapisano je u
   bazi. **Privremena memorija nije istina**: kes, memorija forme, zastavice, datoteke-oznake, zakljucavanja i
   zapisnici pored koda nikad ne nose stanje izmedu procesa i formi; smiju biti samo lokalni pomocnik unutar
   jednog procesa, a istinu uvijek potvrduje baza.
2. **Projekt odreduje putanje.** Direktorij projekta, izlazni direktoriji (`original`, `proxy`, `ingest`,
   `filmstrip`, ...), izvori i lokacije dolaze iz postavki projekta i baze. Kod, alat ni test ne smije
   pretpostaviti, sastaviti ni zapisati putanju, disk ili projekt "napamet" ni doci do njega mimo baze.
3. **Radi se po stanju aktivnog projekta iz baze, ne po namjeri ni pretpostavci.** Aktivni projekt je onaj u
   `app_settings.active_project_id`; prije svake provjere, mjerenja ili popravka pogledati koji je aktivan i
   u kakvom je stanju. Stanje se ponovno cita, ne pamti iz proslog koraka.
4. **Testovi, mjerenja i dijagnostika nikad ne pisu u stvarni projekt.** Rade nad kopijom u kojoj je putanja
   projekta preusmjerena na kopiju, a preusmjerenje se provjerava prije pokretanja. Ako se ne moze
   dokazati da je kopija izolirana, radi se samo citanje.
5. **Postupak (procedura) se uvijek uzima iz QNC v5 koda i tamo se cita do detalja; ne nagada se.**
   Ako v5 nesto ne pokriva, to se kaze korisniku, ne izmisljava. Korisnikove odluke imaju prednost pred v5.
6. **Postojeci UI i layout ostaju netaknuti** osim uz izricito odobrenje zapisano u odjeljku 18.
7. **Kartica (izvor medija) je uvijek, i softverski, SAMO ZA CITANJE.** Nijedna aplikacija, alat, test,
   mjerenje ni dijagnostika ne smije na kartici ili izvoru nista pisati, stvarati, brisati, preimenovati ni
   mijenjati. Izvori se citaju iskljucivo kroz citace koji nemaju operaciju pisanja. Pisanje je dopusteno samo
   u direktorije koje odreduje projekt (pravilo 2), a kod koji pise mora to provjeriti prije pisanja.
8. **Broadcast Player je centar playback sustava.** Isti javni Broadcast layer,
   isti javni preview monitor, isti javni media clip izbornik i isti javni
   source timeline moraju se koristiti u svim formama: Ingest, Story, Media
   Assist i buducim aplikacijama. Forma ih samo smjesta u layout i salje
   korisnicki intent kroz javni ugovor. Nijedna forma ne smije imati privatni
   player, privatni preview monitor, privatni media pool/clip picker, privatni
   timeline paint ili vlastito tumacenje player stanja.
9. **Sve playback-adjacent komponente su interfejsi oko Broadcast Playera.**
   Preview monitor, source/program monitor, timeline, keyboard, media clip
   izbornik, filmstrip, wave i buduci HDMI/SDI/NDI izlazi sluze Broadcast
   Playeru kao ulazni ili izlazni interfejsi. Ne smiju postati drugi player,
   drugi sat, drugi katalog istine ili aplikacijski monolit.
10. **Source timeline ima jedan javni univerzalni prikazni put.**
    Ingest, Story, Media Assist i buduce forme za source timeline koriste isti
    `qnc-source-dock::show_timeline_dock` ulaz i istu `qnc-timeline` UI
    komponentu. Filmstrip je iskljucivo pasivni background te source timeline
    komponente. Forma ne smije imati vlastiti source timeline paint, vlastito
    slaganje filmstripa, vlastitu geometriju slotova ni posebnu varijantu za
    pojedinu aplikaciju. Segment/program timeline nije predmet ovog pravila:
    postojeci slot API za segment timeline ostaje dopusten i ne smije se dirati
    dok se radi source timeline.

## 1. Tocne putanje

- Novi root je tocno: `C:\Users\miron\Projects\QNC`.
- Stari referentni projekt je tocno: `C:\Users\miron\Projects\qnc_v4`.
- `_` je dio imena direktorija, nije separator i ne smije se mijenjati ili
  tumaciti drukcije.
- Prije vecih copy/move/delete operacija obavezno provjeriti tocnu apsolutnu
  putanju.
- Kod mora biti OS-neutralan: Windows, Linux i macOS.
- Stabilni identiteti u ugovorima ne smiju biti raw Windows/Linux/macOS path.
  Za javne reference koristiti QNC transport URI / resolver model.

## 2. QNC kao obitelj aplikacija i modula

- QNC vise ne smije biti jedan monolit.
- QNC se gradi kao obitelj aplikacija/formi koje pokrece shell.
- Aplikacija/forma nije isto sto i modul.
- Aplikacija/forma ima vlastiti workflow, vlastite podatke, vlastiti lifecycle i
  vlastitu odgovornost.
- QNC aplikacije/forme su: Project, Ingest, Media Assist, Story i buduce
  aplikacije/forme.
- Shell / QNC.app nije QNC poslovna aplikacija/forma. Shell je desktop host.
- Shell nema vlastitu poslovnu bazu i ne smije biti owner Project, Ingest,
  Media Assist, Story ili drugog poslovnog workflowa.
- `qnc.shell` manifest smije postojati samo kao host/registry ugovor, ne kao
  poslovna aplikacija u ownership matrici.
- Filmstrip, Wave, Broadcast Player, Export, Media Probe, Media Browser,
  Timeline, Monitor, Dir Browser, Keyboard/Shortcut i slicni dijelovi nisu
  aplikacije. To su moduli.
- Modul je gradivni dio aplikacije: UI modul, DB modul, scanner modul, probe
  modul, generator modul, player modul, export modul, browser modul, resolver
  modul, keyboard/shortcut modul, codec adapter, test modul itd.
- Aplikacije mogu koristiti unutarnje module i vanjske module.
- Modul moze biti in-process ili out-of-process plugin/helper, ali i dalje nije
  QNC aplikacija ako nema vlastiti aplikacijski workflow.
- Moduli su javno dobro unutar QNC sustava. Ne smije se hardkodirati popis
  aplikacija koje smiju koristiti modul.
- Javni moduli koji predstavljaju osnovnu montaznu povrsinu moraju biti isti
  za sve aplikacije: Broadcast layer/player command-state ugovor, preview
  monitor, media clip izbornik i source timeline. Story, Media Assist, Ingest
  i buduce forme smiju mijenjati samo layout i domenski workflow oko tih
  komponenti, ne smiju stvarati njihove privatne varijante.
- Modul objavljuje capabilityje, input/output contract i state/write policy.
- Modul ne smije imati hardkodiranu zabranu tko ga smije koristiti.
- Modul smije imati dependency boundary: sto on sam ne smije pozvati, ucitati
  ili pokrenuti. Primjer: Filmstrip modul ne smije pozvati Media Probe,
  `ffprobe`, scanner ili Ingest workflow.
- Aplikacija/forma smije imati workflow zabrane: sto ona ne smije pokrenuti ili
  koristiti u svojem workflowu.
- Ni modul ni aplikacija ne smiju koristiti zabranu kao popis dozvoljenih
  korisnika modula.
- Isti modul smije se koristiti u vise aplikacija ako nema vlasnistvo nad tudim
  bazama i ne dijeli aktivno stanje izmedu aplikacija.
- Aktivni kod ne smije se gomilati u jednu opcu aplikacijsku komponentu.
  Svaka aktivna odgovornost mora imati uski javni modul ili javnu komponentu:
  browser, source select, probe, DB publish, player client, player-timeline
  projekcija, filmstrip, wave, export i slicno. Aplikacijska komponenta smije
  orkestrirati vlastiti workflow kroz te javne ugovore, ali ne smije postati
  novi monolit.
- Ako vise aplikacija koristi isti modul, svaka aplikacija ga koristi unutar
  vlastite granice i zapisuje samo vlastite rezultate u vlastitu bazu.
- Izmedu aplikacija ne smije postojati alat za suradnju, nasljedivanje,
  dijeljeni runtime context, shared workflow service, aplikacijski bridge ili
  slican posrednik. Zapis u bazi kroz DB contract je jedina poslovna veza.
- Aktivni kod iz stare aplikacije ne smije se kopirati bez jasnog razdvajanja
  odgovornosti i korisnickog odobrenja.

## 3. Opci model za sve QNC aplikacije

- Pravila koja su naglasena kroz Ingest vrijede kao arhitekturni obrazac za
  sve QNC aplikacije.
- Svaka QNC aplikacija mora biti samostalna jedinica s jasnim vlasnistvom nad
  svojim workflowom, bazom, statusom, greskama i lifecycleom.
- Svaka QNC aplikacija mora moci raditi bez direktnog pozivanja privatnog koda
  druge QNC aplikacije.
- Shell smije pokrenuti aplikaciju/formu, ali ne smije preuzeti njezin
  workflow.
- Shell je QNC kvazi desktop: aplikacije se otvaraju u njegovom desktop
  prostoru kao aplikacijske povrsine, a ne kao novi monolitni tabovi.
- Samo `qnc-app.exe` / `qnc.shell` smije biti QNC desktop host. Ne smije
  postojati drugi QNC desktop, drugi desktop host manifest niti druga
  aplikacija s `shell.desktop` capabilityjem.
- Project, Ingest, Media Assist, Story i buduce aplikacije nisu desktopi.
  One su aplikacijske forme/povrsine koje se mogu prikazati unutar jedinog
  shell desktopa ili pokrenuti kao samostalni aplikacijski prozor.
- Nazivi adaptera i registry polja tipa `desktop_entry` odnose se iskljucivo
  na ulaz za jedini shell desktop. Takav adapter ne smije biti drugi desktop,
  ne smije imati vlastiti app registry, footer, shell navigaciju ili hostati
  druge aplikacije.
- Svaka QNC aplikacija mora moci raditi i samostalno, bez shell desktopa.
- Standalone executable je obvezan za svaku QNC aplikaciju. Shell desktop nije
  vlasnik aplikacije i ne smije biti jedini nacin pokretanja.
- Shell-hosted oblik aplikacije smije koristiti samo javni aplikacijski entry
  point/adapter, ne privatne module aplikacije.
- Dodavanje ili uklanjanje aplikacije iz shella ide preko app registry
  manifesta `apps/*/qnc-app.json`, ne preko rucnog hardkodiranja tabova u
  shell kodu.
- App registry manifest je "kvazi include" za shell: opisuje aplikaciju,
  redoslijed taba, nacin hostanja i standalone executable, ali ne smije sadrzati
  poslovnu logiku.
- `apps/*/qnc-app.json` registrira tab, redoslijed, `host_mode` i
  `desktop_entry`.
- Katalog dostupnih aplikacija generira samostalan alat iz postojecih
  registracija i prisutnih izvrsnih datoteka. Katalog smije biti JSON ili
  baza; nije poslovna baza niti alat za suradnju izmedu aplikacija.
- Forma ne generira katalog i ne otkriva instalirane aplikacije. Izbornik
  dostupnih aplikacija cita samo provjereni popis iz kataloga, bez
  hardkodiranih zamjenskih aplikacija. Katalog se osvjezava nakon dodavanja,
  uklanjanja ili promjene registracije/instalacije.
- Svaka aplikacija pripada prioritetnoj grupi (`priority_group`) oznacenoj
  slovom a-z u registraciji. Vise aplikacija smije pripadati istoj grupi:
  to su alternativne izvedbe iste faze (skracena, standardna, prosirena itd.).
- U jednom templateu smije biti odabrana najvise jedna aplikacija iz svake
  grupe. Izbor jedne onemogucuje druge iz te grupe; uklanjanje izbora ponovno
  oslobada grupu. Zauzetost nije globalna niti zajednicko runtime stanje.
- Izbornik koristi option/radio dugmad po grupama. Odabir druge varijante
  zamjenjuje prethodnu, nikad ne dodaje drugu u istu grupu. `Bez odabira`
  preskace neobaveznu grupu. Grupa a mora imati jednu odabranu aplikaciju;
  ako postoji samo Project, ne moze se iskljuciti. Alternativa iz grupe a
  moze zamijeniti Project. Nepostojece aplikacije ne prikazuju se ni kao disabled stavke.
- Project ucitava katalog pri pokretanju. U formi nema dugmeta za rucno
  osvjezavanje niti generiranja kataloga.
- Odabrane grupe uvijek slijede abecedni prioritet, npr. a-c-d, nikad a-d-c.
  Neodabrane grupe se smiju preskociti. Grupe dolaze iz registracija, ne iz
  hardkodiranog popisa imena aplikacija. Numericki `order` nije podrzan;
  registracija, template i shell koriste samo `priority_group`.
- Izbor aplikacija iz templatea i njihov grupno sortiran slijed zapisuju
  se u projektnu bazu. Shell smije automatizirati navigaciju prema tom zapisu,
  ne poslovni rad aplikacija. Katalog ne sadrzi aktivni projekt ili njegovo
  poslovno stanje.
- U razvoju nema migracija ni kompatibilnog starog nacina. Nevaljani razvojni
  zapisi uklanjaju se, ne popravljaju niti pretvaraju u novi format.
- Uspjesno kreiranje ili otvaranje projekta salje shellu jednokratni
  `shell_next_group` UI okidac bez poslovnog payloada. Okidac se ne zapisuje.
  Shell cita navigacijski slijed iz javnog DB prikaza kroz desktop adapter;
  ne predaje radne postavke drugoj aplikaciji i ne pokrece njezin workflow.
- Shell `Close project` je samo poziv samostalne javne komponente/modula
  `qnc-project-close`. Shell smije poslati taj intent i prikazati rezultat.
  Sav stvarni rad zatvaranja aktivnog projekta pripada toj komponenti. Ona smije
  isprazniti `active_project_id` u Project registryju i vratiti rezultat. Shell
  nakon toga ne smije zatvarati hostane aplikacijske povrsine, prebacivati tab,
  pokretati Project workflow, brisati temp direktorije niti cistiti poslovno
  stanje. Ni shell ni `qnc-project-close` ne smiju brisati projekt, direktorij,
  bazu, media fileove ni artefakte. Brisanje projekta ostaje iskljucivo Project
  workflow kroz `X` u Project popisu i potvrdu korisnika.
- LAN/Intranet zatvaranje aktivnog projekta ide samo kroz uski javni
  `qnc-project-close` write adapter. Taj adapter prima samo
  `project.close_active`, provjerava `project_registry` URI i write ovlast, te
  ne smije izlagati genericki SQL, ProjectStore API, delete workflow ili temp
  cleanup.
- App registry manifest nije dovoljan za embedded hostanje. Shell ne smije imati
  hardkodirani `if desktop_entry == "qnc_project"` niti slican switch po imenu
  aplikacije u render/activate putu.
- Embedded hostanje ide samo preko javnog desktop adaptera aplikacije s ulazom
  `create` + `show_desktop`.
- Shell smije imati registry factory tablicu samo kao mapu
  `desktop_entry -> javni adapter`, bez znanja o privatnim modulima te
  aplikacije.
- Nova embedded aplikacija zahtijeva vlastiti app crate, `qnc-app.json`, javni
  desktop adapter i Cargo ovisnost shella samo na taj adapter crate, ne na puni
  aplikacijski crate koji sadrzi store, advanced ili privatni UI.
- Shell ne smije u jednom procesu sakupljati pune aplikacijske crateove
  (`qnc-project`, `qnc-ingest`, `qnc-story`...) kao novi monolit. Ako
  in-process hostanje to privremeno nalaze, adapter crate mora biti odvojen od
  owner/store cratea.
- Compile-time factory s jednim Project adapterom smije ostati samo dok je to
  zapisano kao privremeno odstupanje. Sljedeca aplikacija ne smije se dodati
  prosirenjem istog if/switch puta uz puni app crate.
- Shell smije citati app registry i prikazati aplikacijsku povrsinu, ali ne
  smije zvati scan/probe/media/render/player/workflow rutine aplikacije.
- Shell ne smije pokretati QNC aplikacije kao zasebne OS prozore kao finalni
  desktop model, osim ako korisnik izricito trazi standalone pokretanje.
- Forma ne smije nositi aktivni kod. Forma smije prikazati stanje, skupljati
  korisnicki input i poslati intent/naredbu, ali stvarni rad mora biti u
  komponenti aplikacije ili u javnom modulu.
- Razgovor izmedu javnih komponenti i modula ne smije ici preko forme. Forma
  ne smije prevoditi player stanje u timeline, browser state u transport,
  probe rezultate u katalog ili bilo koji drugi aktivni medukomponentni tok.
  Takav prijevod pripada specijaliziranom javnom modulu ili aplikacijskoj
  komponenti koja ne crta UI.
- Forma ne smije direktno pozivati store, filesystem, scanner, probe, player,
  render, export ili druge workflow operacije ako za to postoji komponenta ili
  modul.
- Komponenta aplikacije smije zvati vlastiti store. Forma ne smije.
- Neutralni intent mora imati `action_id` iz keyboard/shortcut ugovora ili isti
  `action_id` bez tipke kada dolazi iz klika. Forma ne smije zvati
  create/delete/open metode storea.
- Produkt svake QNC aplikacije mora biti zapisan u njezinu bazu ili u njezin
  javni DB contract.
- Druge aplikacije koriste taj rezultat citanjem baze, ne pozivanjem rutine
  aplikacije koja ga je napravila.
- Project aplikacija je vlasnik `project_registry` i `project_workspace`
  baza. Globalna baza i baza po projektu moraju biti odvojene kroz DB
  contract, iako implementacija fizicki koristi SQLite datoteke.
- Project aplikacija kreira projektnu bazu za svaki projekt posebno i u tu
  projektnu bazu zapisuje samo opce projektne podatke, template izbor, lokacije
  i postavke. Ne zapisuje pravila specijalizirana za Ingest, Story, Media
  Assist, Filmstrip, Wave, Broadcast Player ili buduce poslovne workflowe.
- Druge aplikacije ne smiju znati za Project aplikaciju, Project crateove,
  Project komponente, Project store ili Project workflow. Za rad nad projektom
  moraju iz baze aktivnog projekta procitati i primijeniti dogovorene postavke
  potrebne za svoj rad. To nije opcionalno niti pravilo samo za Ingest.
- Oznaka aktivnog projekta mora biti podatak u bazi, ne UI stanje i ne argument
  koji aplikacija izmisli. Svaka aplikacija koja radi nad projektom iz baze
  utvrdjuje aktivni projekt i iz njegove baze cita postavke za rad.
- QNC baza mora biti prenosivi poslovni artefakt. Na bilo kojem racunalu kojem
  korisnik da valjanu QNC bazu, svaka aplikacija koja koristi njezin javni
  ugovor mora moci raditi bez Project aplikacije, bez QNC.app shella i bez
  bilo kojeg drugog QNC aplikacijskog procesa.
- Aplikacija koja cita postavke za rad aktivnog projekta smije ih citati samo
  read-only. Ne smije pisati, migrirati, popravljati ni otvarati Project
  workflow.
- Project system template seed je obvezni lokalni QNC artefakt:
  `C:\Users\miron\Projects\QNC\seed\system_seed.json`, preuzet iz
  `C:\Users\miron\Projects\qnc_v4\seed\system_seed.json`.
- Project system templatei su ugrađeni/seedani templatei i smiju biti
  hardkodirani kao zaključani QNC baseline.
- Project system templatei se ne smiju brisati.
- Korisnički templatei nisu system templatei: zapisuju se u Project registry
  bazu, imaju `system = 0` i smiju se brisati kroz Project aplikaciju.
- Project registry smije imati privatne lokalne runtime tablice za resolver
  konfiguraciju i fizicku storage lokaciju projekta. Te putanje nisu javni
  identitet projekta; javni identitet ostaje QNC URI.
- Project direktoriji moraju biti zasticeni od slucajnog korisnickog brisanja
  OS-level delete lockom. Sam read-only atribut nije dovoljan na svim OS-ovima.
- Delete lock se odnosi na root direktorij projekta kao OS-level zastitu od
  brisanja. Ne smije se rekurzivno postavljati read-only na sadrzaj projekta,
  jer Ingest i ostale aplikacije moraju moci pisati vlastite baze i artefakte.
- Otvaranje postojeceg projekta ne smije ponovno prolaziti kroz projektno
  stablo niti pokretati masovni filesystem/ACL posao. Otvaranje projekta je
  kratki DB update aktivnog projekta i navigacijski okidac.
- Windows adapter koristi ACL deny delete/delete-child za trenutnog korisnika.
- macOS adapter koristi file flags gdje je dostupno.
- Linux/POSIX adapter mora zakljucati parent `projects_root`, jer POSIX delete
  direktorija kontrolira write permission na parent direktoriju.
- Hidden je samo dodatna zastita od slucajnog korisnickog diranja, nije glavna
  lock zastita.
- Potvrdeno brisanje cijelog projekta pripada Project workflowu. Zastita od
  slucajnog brisanja ne smije zabraniti drugim aplikacijama zapis vlastitih
  rezultata u projektne radne direktorije i vlastite DB tablice kroz javne
  storage/DB module. Za takav zapis nije potrebno pokretati Project aplikaciju.
- Za ovu zastitu ne uvoditi dodatni DB lease/storage-state model bez posebnog
  odobrenja.
- Batch aplikacija nakon zavrsetka posla prestaje raditi, osim ako je drukciji
  lifecycle eksplicitno dio njezina dizajna.
- Runtime modul, npr. Broadcast Player ili Monitor, smije ostati aktivan samo
  unutar aplikacije koja ga koristi ili kroz jasno ugovoreni module lifecycle.
- Isti princip vrijedi za Project, Ingest, Media Assist, Story
  aplikacije/varijante i buduce QNC aplikacije.

## 4. DB-first zakon

- Baza je izvor istine, ne UI.
- UI je pasivna forma.
- Jedina poslovna veza izmedu QNC aplikacija/formi je baza kroz javni DB
  contract.
- Sama baza mora biti dovoljna poslovna veza. Ako je baza kopirana na drugi
  racunar, aplikacija koja zna taj DB contract mora moci citati sto joj treba
  bez prisutnosti aplikacije koja je bazu prvotno stvorila.
- Aplikacije se ne smiju medusobno poznavati kao aplikacije. Ne smije postojati
  poslovna veza Ingest -> Project, Story -> Ingest ili slicno preko API-ja,
  cratea, procesa, klase ili privatne funkcije.
- Ne smije postojati poseban alat za suradnju, nasljedivanje ili shared runtime
  context koji aplikacijama prenosi poslovno stanje mimo baze.
- Aplikacija smije citati javne ili dogovorene podatke iz baze kroz DB/transport
  ugovor. Ona time poznaje ugovor baze, ne aplikaciju koja je tu bazu stvorila.
- Aplikacija smije pisati samo u vlastitu bazu ili vlastitu shemu.
- Vlastita shema moze biti skup tablica u istoj fizickoj projektnoj SQLite
  datoteci. Read-only citanje projektnih postavki ne znaci read-only za sve
  tablice te datoteke. Upis vlastitih rezultata ne daje pravo na izmjenu
  Project postavki, registra, identiteta ili aktivacije.
- Nijedna forma, aplikacijski UI sloj, worker, generator, scanner, probe,
  player, timeline, filmstrip, wave, export ili drugi potrosacki modul ne smije
  direktno pisati u bazu, direktno izvrsavati write SQL niti direktno otvarati
  javnu projektnu bazu u `ReadWrite` modu.
- Svaki DB write ide iskljucivo kroz javni DB owner/write adapter ili javni
  DB/transport writer definiran ugovorom te baze. Taj writer serijalizira
  kratke write operacije jednako za Local/LAN/Intranet i jedini smije imati
  stvarnu write ovlast nad javnim DB endpointom.
- Worker/generator/modul koji proizvodi rezultat smije vratiti gotov artefakt,
  zapis ili write naredbu, ali ne smije sam raditi DB publish. Ako treba zapis
  u bazu, predaje ga javnom write transportu.
- Direktan DB write helper smije postojati samo unutar implementacije DB ownera
  ili write transporta te u njegovim izoliranim testovima. Takav helper nije
  javni komunikacijski model i ne smije se pozivati iz formi ili potrosackih
  modula.
- Nema direktnih privatnih SQLite zaobilaznica kao javnog komunikacijskog
  modela.
- Nema pozivanja privatnih funkcija, klasa, skripti, workera ili procesa druge
  aplikacije kao poslovne komunikacije.
- Transport/proxy sluzi za pristup DB i media lokacijama u local/LAN/intranet
  okruzenju. Ne smije zamijeniti DB kao izvor istine.
- Ako standalone aplikacija i shell-hosted oblik iste aplikacije mogu otvoriti
  istu SQLite bazu, baza mora imati dogovoreni multi-process rezim: kratke
  transakcije, `busy_timeout`, WAL gdje je podrzan i bez dugog DB locka iz UI
  threada.
- Dugotrajni rad ne smije drzati write lock nad javnom bazom. Dugotrajni rad
  ide u modul/komponentu i pise kratkim batch transakcijama.

### 4.1. Projektne postavke za sve aplikacije

- Sve postojece i buduce poslovne aplikacije, ukljucujuci Ingest, Media Assist
  i sve Story varijante, primjenjuju postavke projekta iz baze. Samostalno
  pokretanje znaci neovisnost o drugim aplikacijama i shellu, ne neovisnost
  o zapisanim projektnim postavkama.
- Project aplikacija je vlasnik zapisa opcih projektnih postavki. Te postavke
  nisu opis Ingesta ili bilo koje druge aplikacije. Ostale aplikacije citaju
  potrebne vrijednosti read-only kroz javni DB/transport ugovor; ne prepisuju
  ih, ne nasljedjuju ih od druge aplikacije i ne zamjenjuju ih lokalnim
  defaultima. Poslovne rezultate i dalje zapisuju samo u vlastitu bazu ili
  shemu.
- Vrijednosti zapisane u projektnom templateu ili projektnim postavkama nisu
  hardkodiranje. One su korisnicka/projektna konfiguracija. Hardkodiranje je
  kada aplikacija, modul ili adapter koristi fiksnu vrijednost mimo zapisa u
  bazi, source zapisa ili eksplicitne host konfiguracije.
- Citanje i primjena postavki pripadaju komponentama/modulima, ne formi.
  Shell ne dostavlja poslovne postavke niti postaje njihov vlasnik.
  Isti ugovor vrijedi za standalone i shell-hosted rad, Local/LAN/Intranet
  te Windows/Linux/macOS.
- Prije audita, plana ili implementacije bilo koje aplikacije obavezno pratiti
  cijeli put: postojeci zapis u bazi aktivnog projekta -> javni read-only
  ugovor -> ucitane radne postavke -> njihova primjena u komponenti/modulu.
  Provjeriti stvarni zapis i kod koji ga cita; nedostatak u modelu potrosaca
  nije dokaz da podatak ne postoji u projektnoj bazi.
- Prvo utvrditi prekida li se postojeca veza citanja ili primjene postavki.
  Ne predlagati nove projektne postavke, nove izlazne baze ni izmjene Projectsa
  prije te provjere. Potvrdjeni nedostatak prijaviti s tocno navedenim zapisom
  ili ugovorom; ne zaobilaziti ga lokalnim defaultom. Ako obvezne postavke nisu
  dostupne, posao koji ih zahtijeva ne pokrece se; prijavljuje se kontrolirana
  greska. Project ostaje zamrznut prema odjeljku 14.

- Jedini put od projektne baze do potrosaca (Ingest, player, filmstrip, wave,
  buduci Story). Ne smije se preskociti korak, zamijeniti lokalnim JSON-om
  niti krenuti od forme, shella ili Project cratea. Detalj: `docs/83-project-db-to-module-path.md`.

  1. `project_registry.public_app_settings.active_project_id`
  2. javni identitet projekta (`public_projects.project_uri`), ne raw path
  3. `public_project_settings.settings_json` aktivnog `project_id`
  4. `qnc-work-settings` read-only (`SettingsReader`) -> `WorkSettings`
  5. layout/odredista kroz javni work-plan/storage iz tog snapshota
  6. clip/media snapshot kroz javni content read port (Ingest owner store)
  7. `qnc-player-input::InputReader.load(workspace_db_uri, clip_id)`
  8. launcher -> `qnc-broadcast-player` dobiva samo `PreparedInput`

  Iz `settings_json` player slike bira samo `playback.input`. Izlazni audio
  broj kanala i sample rate dolaze iz `audio.channels` / `audio.sample_rate`.
  Source timebase, trajanje i inventar kanala dolaze iz spremljenog media
  snapshota, ne iz `video.fps`. Nedostaje li korak 1–4, posao staje.

  Zabranjeni precaci: `qnc-project*` crate, shell payload, `player-output.json`,
  forma cita SQLite, path-join na `project.db` kao javni korak, novi probe,
  izmisljeni default, HTTP/URL kao decoder ulaz, Project/export FPS kao sat.

## 5. Ingest kao prvi konkretni primjer

- Ponovni Select istog izvora uskladjuje razlike, ne prazni i ponovo puni cijeli
  prikaz. Postojeci klipovi i selekcija ostaju; novi se dodaju. Nedostajuci se
  uklanjaju iz detektiranog kataloga samo nakon potvrdenog nedostatka na izvoru,
  nikad zbog prekida ili greske transporta. Izvorne datoteke se ne brisu.
- Klip se smije prikazati cim je ucitan u memoriju, dok javna DB komponenta
  sprema u pozadini. Prikaz jasno razlikuje nepotvrdjen/neuspjesan upis od
  spremljenog rezultata; kasnije radnje ne koriste nepotvrdjen zapis.
  To ne odgadja obvezne trajne probe claim/evidence zapise prije probea.
- Filter `Novi / Sve` (`New / All`) zamjenjuje oznaku `Postojeci` i dugme
  `Osvjezi`. Novi su klipovi koji nisu postojali u bazi prije tekuceg Selecta;
  to nije oznaka dovrsenog importa. Usporedbu radi komponenta kroz javni DB
  ugovor, ne forma. Filter mijenja samo prikaz, bez scana, probea ili DB upisa.
  Skrivena selekcija ostaje sacuvana; skupne akcije odabira vrijede za vidljive
  klipove. Pocetni prikaz je Sve.
- Podaci o klipovima tijekom Select/Ingest faze primarno se citaju iz
  proizvodjackih metadata/index datoteka na kartici kada postoje, npr. camera
  XML/JSON/binary katalozi i povezani sidecar zapisi. To citanje ide kroz
  javne source-reader/camera-reader module i transport, ne kroz formu.
- Skeniranje ekstenzija i stablo direktorija sluze za pronalazak kandidata i
  potvrdu fizicke prisutnosti datoteka, ali nisu zamjena za camera metadata
  zapis kada ga kartica ima.
- `ffprobe` u Ingestu smije samo nadopuniti ili potvrditi obvezne podatke koje
  camera metadata ne daje ili daje nepotpuno. I dalje vrijedi jedan probe
  prolaz u Select/Ingest fazi; kasnije aplikacije i moduli citaju finalni DB
  zapis, ne karticu, camera datoteke ili novi probe.
- Ucitavanje postojeceg Ingest kataloga iz baze aktivnog projekta ne smije
  cekati thumbnail/poster decode, citanje kartice ili mrezu. Lista klipova i
  statusi dolaze odmah iz DB zapisa; slike se ucitavaju naknadno u pozadinskoj
  komponenti kao pasivni prikaz. Nedostupan izvor ne smije sakriti vec
  spremljeni katalog.
- Pocetni prikaz Ingest kataloga smije koristiti samo lagani public DB sazetak
  iz materijaliziranih kolona (`clip_id`, `name`, trajanje, status, source/card
  podaci, thumbnail URI ako postoji). Ne smije deserijalizirati puni
  `catalog_json`, `probe_json`, media snapshot ili thumbnail payload za obican
  prikaz liste. Puni zapis cita se tek preko `read(clip_id)` kada ga aktivni
  modul stvarno treba, npr. Broadcast Player ili import worker.
- Pri prelasku izmedu aplikacija ili povratku na Ingest ne smije se preskociti
  provjera stanja kataloga. Komponenta mora kroz javni DB/transport ugovor
  procitati lagani signature kataloga (minimalno broj klipova i revizijski
  fingerprint) i usporediti ga s prikazanim stanjem. Ako je signature isti,
  prikaz ostaje u memoriji; ako se razlikuje, ucitava se novi lagani sazetak.
  Ova provjera ne smije skenirati karticu, citati thumbnaile, otvarati media
  datoteke niti pokretati probe.

- Ingest je samostalna zatvorena aplikacija.
- Korisnik mora moci pokrenuti Ingest bez QNC.app i bez drugih QNC aplikacija.
- QNC.app ne mora postojati na istom racunalu i ne mora biti pokrenut.
- Ingest ne zna i ne smije znati za Project aplikaciju. Ingest ne smije imati
  dependency na Project app crate, Project desktop adapter, Project component,
  Project store, Project action_id ili Project workflow.
- Ako Ingest radi nad aktivnim projektom, mora iz baze procitati koja projektna
  baza ima oznaku aktivnog projekta i iz te baze procitati samo postavke za rad.
- Ingest mora moci raditi na racunalu na kojem ne postoji Project aplikacija,
  ako mu je dostupna valjana baza s oznakom aktivnog projekta i postavkama za
  rad.
- Ingest ne smije imati rucno postavljanje projektnih radnih postavki. Ne smije
  imati svoj projektni settings editor, lokalni override za project workflow,
  niti default koji zamjenjuje nepostojece projektne postavke.
- Postavke po kojima Ingest radi dolaze iz baze aktivnog projekta. Ingest ih
  samo cita i primjenjuje kao ulazni ugovor.
- Ako ne postoji aktivni projekt ili postavke za rad nisu citljive, Ingest mora
  stati u kontrolirano stanje greske. Ne smije kreirati projekt, traziti Project
  aplikaciju, popravljati Project bazu niti izmisljati default projekt.
- Ingest nakon zavrsetka ingest procesa prestaje raditi.
- Produkt Ingest aplikacije je baza i pripadajuci artefakti zapisani kroz
  ugovor.
- Ingest moze koristiti module kao sto su Dir Browser, Media Browser, Media
  Probe, Filmstrip, Wave i resolver.
- Druge aplikacije ne smiju pokretati Ingest workflow, Ingest scanner ni Ingest
  probe.
- Druge aplikacije smiju samo citati rezultate koje je Ingest zapisao u bazu.

### 5.1. Primjena projektnih odredista u Ingestu i media modulima

- Ovo je konkretna primjena opceg pravila iz odjeljka 4.1, ne ogranicenje tog
  pravila na Ingest. Sve druge aplikacije i njihovi moduli jednako su obvezni
  koristiti zapisane projektne postavke relevantne za svoj rad.
- Project aplikacija zapisuje lokaciju konkretnog projekta, template izbor i
  opce radne postavke te kreira standardni projektni raspored. Ona ne zna tko
  ce te podatke koristiti. Ingest, Filmstrip, Wave i drugi potrosaci samo citaju
  taj DB zapis i ne mijenjaju ga.
- Ingest iz baze cita aktivni projekt i njegove zapisane radne postavke.
  Ne dobiva ih pozivom Project aplikacije ni iz shell/UI memorije. Project
  aplikacija ne mora biti prisutna ni pokrenuta; dovoljan je zapis dostupan
  kroz javni DB/transport ugovor.
- Standardna relativna odredista razrjesava javni storage modul unutar
  lokacije konkretnog projekta procitane iz baze. Kao u v4, svaka podmapa i
  naziv DB datoteke ne moraju biti zasebno polje projektnih postavki.
  Primjena postojeceg rasporeda nije izmisljanje novih postavki. Nije dopusteno
  samostalno izabrati drugi projektni korijen, novu bazu ili zamjensko odrediste
  kada stvarni projektni zapis nedostaje. Forma i generator ne sastavljaju
  privatne putanje; koriste javni DB/storage ugovor.
- V4 raspored (`original`, `proxy`, `audio`, `incoming/card`, `incoming/ftp`,
  `ingest/thumbnails`, `filmstrip`) jest standardni referentni raspored koji
  javni storage modul primjenjuje, ne mijenja prema pojedinoj aplikaciji.
  Koji medij treba kopirati ili samo povezati odredjuju Projectove zapisane
  radne postavke; postojanje podmape samo po sebi nije naredba za kopiranje.
- Izbor kartice u browseru odredjuje IZVOR, ne izlaznu lokaciju. Izvorna kartica
  ostaje read-only. Direktorij aplikacije, `target`, cache i temp nisu zamjenska
  odredista za trajne projektne rezultate.
- Filmstrip slicice spremaju se kao JPEG datoteke na odrediste izvedeno iz
  opceg projektnog DB zapisa i standardnog rasporeda (v4 referenca:
  `filmstrip/<clip_id>/`); baza cuva vezu s klipom, redoslijed, vremenske
  polozaje, status i reference na artefakte. Ne zamjenjivati ovaj raspored JPEG
  BLOB-ovima u novoj `filmstrip.db` bez izricitog odobrenja.
- Filmstrip DB veza ostaje `project_id + clip_id`, ali javni direktorij
  artefakta mora koristiti naziv klipa zapisan u `clips.name`, OS-neutralno
  sanitiziran za QNC URI i filesystem. Interni `clip-*` smije biti samo fallback
  kada naziv klipa nije dostupan ili nije valjan.
- Wave se sprema kao niz amplituda/peaks po kanalu u projektnu bazu kroz javni
  DB/transport ugovor. Ne zahtijeva posebnu wave mapu, PNG datoteke niti novu
  `wave.db` samo zato sto je generator zaseban modul. U v4 su to `a1_peaks` i
  `a2_peaks` u tablici `audio_waveforms`.
- Referentni v4 `project.db` sadrzi tablice `filmstrips`,
  `filmstrip_frames` i `audio_waveforms`. To je dokaz nacina pohrane, ne dozvola
  za vracanje monolita ili pisanje u tudju shemu: u novom QNC-u upis mora ici
  kroz javni DB/transport ugovor i ownera rezultata, uz postovanje odjeljka 4.
- Javni resolver/storage modul samo tehnicki razrjesava zadano odrediste;
  ne odlucuje gdje rezultat pripada. Potrosaci koriste QNC URI -> resolver ->
  endpoint. Privatne fizicke putanje ostaju u storage adapteru; ista pravila
  vrijede Local/LAN/Intranet i na Windows/Linux/macOS, bez ovisnosti o
  prisutnosti Project aplikacije.
- Prije implementacije provjeriti referentni kod u `qnc_v4`:
  `qnc-host/src/project/db.rs` (`project_dir_from_conn`, `ensure_project_dirs_at`),
  `qnc-host/src/filmstrip/store.rs`, `qnc-host/src/waveform/store.rs` i
  `qnc-host/src/ingest/db.rs`. Ne prenositi njihove privatne Project pozive,
  migracije ili fallbacke u nove module. Ova uputa ne odmrzava Project.

## 6. Probe zakon

- `ffprobe` i svaki drugi media probe smiju se izvrsiti samo jednom: tijekom
  `Odaberi` / `Ingest` procesa.
- Nema `filmstrip probe`.
- Nema `player probe`.
- Nema `waveform probe`.
- Nema `export probe`.
- Nema naknadnog probe fallbacka.
- Ako kasnija aplikacija treba probe podatak koji ne postoji u bazi, to je
  greska ingest ugovora, a ne razlog za novi probe.
- Svi podaci potrebni za player, filmstrip, waveform, export i buduce module
  moraju biti zapisani u bazu tijekom jedinog ingest probe prolaza.

## 7. Original/proxy zakon

- Original je pravi clip i uvijek postoji.
- Proxy postoji samo kod nekih kamera/source struktura.
- Proxy nije zaseban clip.
- Proxy se ne smije prikazati kao odvojeni clip.
- Proxy se ne smije probati kao odvojeni clip.
- Proxy metadata mora biti vezana uz originalni clip.
- Za proxy zapisati container, codec i druge potrebne media podatke u istom
  ingest probe prolazu.

## 8. Filmstrip zakon

- Filmstrip u UI-ju je pasivna pozadina za vizualni pregled. Nije izvor playera,
  vremenski sat, osnova za seek ni dio playback/timeline racunanja.
  Referenca je `qnc_v4/qnc-app/src/qnc_filmstrip_background.rs` i njezina
  upotreba kao `video_background` u `qnc_source_dock.rs`.
- Filmstrip nema `cue` intent, `seek` intent, klik handler, selection handler ni
  vlastitu playback akciju. Ako timeline dopusta cue/scrub, taj intent pripada
  javnom timeline/video-row ugovoru, a filmstrip ostaje samo nacrtana pozadinska
  raster slika ispod tog sloja.
- Wave prikaz je takodjer pasivan: crta vec pripremljene amplitude. Ni jedan
  prikaz ne generira artefakte, ne radi probe, ne odredjuje pohranu i ne pise DB.
- Generator filmstripa i generator wavea odvojeni su javni moduli; pohrana ide
  kroz javnu DB/storage komponentu. Prikaz, generator i pohrana nisu jedna
  komponenta niti poslovna logika forme.
- Wave generator se pokrece automatski na temelju zapisa u projektnoj bazi,
  istim lifecycle okidacem kao filmstrip: nakon sto je aktivni projekt ucitan
  ili nakon sto je Select zavrsio zapis u bazu. Ne smije cekati klik na clip i
  ne smije slati peakove formi kao paralelno UI stanje.
- Wave izvor i broj laneova odredjuju samo projektne postavke zapisane u bazi
  i spremljeni media/proxy snapshot klipa. Ako projektni `audio.channels` trazi
  dvokanalni rad i clip ima proxy s audio streamom, wave se moze graditi iz tog
  proxyja. Proxy ne mora imati tocno dva kanala; koristi se onoliko kanala
  koliko proxy stvarno ima. Ako se wave gradi iz originala, koriste se stvarni
  spremljeni kanali originala, redom u javne timeline laneove. Forma, lokalni
  JSON ili hardkodirani fallback ne smiju izmisljati broj kanala.
- Ingest forma i Ingest aplikacijski sloj smiju samo osvjeziti javni wave
  worker/service i citati objavljeni artefakt kroz javni `timeline-assets`
  reader. Wave worker sam odlucuje koji `wave_artifacts` zapisi nedostaju, a
  write ide kroz javni DB/transport writer.
- Wave generiranje smije biti rasporedjeno u vise pozadinskih worker slotova,
  po istom obrascu kao filmstrip, ali samo izvan UI forme i bez dodatnog probea.
  Worker smije citati/dekodirati spremljeni source/proxy prema javnom decoder
  catalogu, izracunati lane peakove i vratiti gotov artefakt. Zapis u bazu
  radi zaseban javni content write transport, ne pojedinacni generator worker.
- Wave worker ne smije konkurirati Broadcast Playeru. Kada player ima prioritet,
  aktivni wave poslovi se moraju zaustaviti ili vratiti u red; nastavak ide
  automatski nakon sto player vise ne trazi prioritet.
- Filmstrip worker smije generirati JPEG artefakte i vratiti opis gotovog
  artefakta, ali ne smije sam otvarati content bazu u `ReadWrite` modu niti
  direktno raditi DB publish. Zapis URI-ja, statusa, redoslijeda i vremenskih
  polozaja u bazu radi javni DB/transport writer koji serijalizira write
  operacije prema Local/LAN/Intranet endpointu.
- Sljedeca pravila odnose se na GENERIRANJE filmstripa. Generator je modul za
  izradu artefakta, ne aplikacija, ne scanner i ne probe.
- Filmstrip generator se pokrece automatski na temelju zapisa u projektnoj bazi.
  Ne smije cekati klik na clip, jer bi korisnik tada cekao generiranje pri
  odabiru.
- Ingest forma i Ingest aplikacijska komponenta ne smiju rasporedjivati
  pojedinacne filmstrip poslove po UI dogadjajima. Smiju samo osvjeziti javni
  filmstrip worker/service nakon sto je aktivni projekt ucitan ili nakon sto je
  Select zavrsio zapis u bazu. Filmstrip worker tada sam cita `ingest_content`
  i sam odlucuje koji artefakti nedostaju.
- Filmstrip cita samo podatke iz baze i iz Projectom zadanog artifact
  direktorija.
- Filmstrip koristi proxy ako postoji, a original ako proxy ne postoji.
- Filmstrip mora generirati stvarne frameove, ne ponavljati poster.
- Filmstrip ima 13 sličica.
- Filmstrip generator trazi 13 sličica i rasporedjuje ih ravnomjerno kroz
  trajanje prikaza.
- Za kratke i duge clipove koristi se keyframe/intra seek najblize ciljnoj
  poziciji kroz javni decoder adapter. Ne smije se linearno citati cijeli klip
  samo da bi se izvuklo 13 sličica.
- Ako keyframe/intra adapter vrati manje stvarnih slika, filmstrip prikaz ih
  ravnomjerno rasporedjuje kroz cijelu sirinu. Ne popunjavati praznine
  ponavljanjem postera.
- Filmstrip ne smije raditi `ffprobe` niti drugi probe fallback.
- Filmstrip ne smije hardkodirati `ffmpeg` niti bilo koji drugi dekompresor.
  Dekoder se bira iskljucivo preko javnog `qnc-decoder-catalog` ugovora ili
  eksplicitnog adaptera. Ako katalog nije dostupan, to je kontrolirana greska,
  ne povod za tihi fallback.
- Izbor novog klipa ne smije resetirati, brisati ni zamijeniti filmstrip podatke
  prethodnog klipa. Smije se promijeniti samo aktivni prikaz; artefakti i cache
  moraju biti vezani uz `project_id + clip_id`, a zakasnjeli worker rezultat smije
  se prikazati samo ako i dalje pripada trenutno aktivnom klipu.
- Ponavljanje postera u starom Ingest UI-ju nije generirani filmstrip i ne moze
  zamijeniti zahtjev za stvarnim frameovima. Pasivni UI obrazac i stvarni
  generirani sadrzaj moraju se promatrati odvojeno.

### 8.1. Timeline je pasivni prikaz i UI remote

- Timeline je samostalna javna pasivna UI komponenta. Prikazuje stanje
  Broadcast Playera i sluzi kao njegov UI daljinski upravljac; nije player,
  playback engine, vlasnik vremena niti vlasnik prikazanog stanja.
- Timeline-engine je jedna jedinstvena javna komponenta s vise UI layera, po
  obrascu `qnc_v4/qnc-app/src/qnc_timeline.rs`. Ne rade se zasebni timeline
  enginei za Ingest, Story, Program ili buduce aplikacije.
- Aplikacija ili njezina komponenta samo priprema neutralni frame/range/layer
  model i ukljucuje/iskljucuje layer flagove koji su joj potrebni. Timeline ne
  smije znati za aplikaciju koja ga koristi niti imati app-specific grane.
- Timeline smije imati javne projection helpere za mapiranje program/source
  frame osi u lokalne UI redove, po uzoru na v4 segment timeline. To je samo
  geometrija prikaza i povratni UI intent; nije playlist owner, player client,
  DB citac niti aktivni workflow kod.
- Timeline nema nista svoje u smislu poslovnog ili playback stanja: nema
  vlastiti playhead, tekuci frame, FPS/timebase, play/pause status, IN/OUT,
  trajanje, playlistu ni odabrani izvor kao neovisnu ili zamjensku istinu.
  Dobiva pripremljene read-only prikazne podatke; ne stvara niti odrzava
  paralelno stanje u formi ili timeline adapteru.
- Runtime frame, pozicija i transport status dolaze iskljucivo od Broadcast
  Playera kroz javni ugovor. Trajni rangeovi i ostali projektni podaci ostaju
  u nadleznosti svojih DB/modula; timeline ih samo prikazuje kroz zadani model.
- Timeline nema vlastiti sat, timer, playback tick, napredovanje frameova,
  interpolaciju vremena, FPS fallback ni lokalnu fallback poziciju. Ako
  valjano player stanje nije dostupno, prikazuje nespremno/prazno stanje;
  ne izmislja vrijeme niti preuzima upravljanje od playera.
- Klik, povlacenje, scrub i keyboard unos proizvode samo neutralni intent s
  vanjskim `action_id`. Javni command/transport modul predaje zahtjev
  Broadcast Playeru ili nadleznom modulu; timeline ne izvrsava naredbu.
  Slanje seek zahtjeva nije potvrda da je player stigao na trazeni frame:
  prikaz stvarnog playheada mijenja se prema povratnom player stanju.
- Geometrija frame -> piksel i pokazivac -> trazeni frame sluzi iskljucivo
  crtanju i korisnickom zahtjevu. Ne smije postati playback matematika,
  source/program vremenska istina ili odluka o sljedecem klipu/segmentu.
- Timeline ne radi decode, probe, scan, generiranje filmstripa/wavea, DB/FS
  pristup ni izbor medija ili odredista. Aktivni kod pripada zasebnim javnim
  modulima/komponentama, ne timelineu niti formi koja ga prikazuje.
- Filmstrip je samo pasivna pozadina V trake, a Wave samo pasivni prikaz
  pripremljenih amplituda audio traka. Ne upravljaju timelineom ili playerom.
  Generatori i pohrana ostaju odvojeni od tih UI prikaza.
- Source, Segment i Program koriste isti javni timeline paint/intent ugovor
  s pripremljenim slojevima, ne zasebne playback modele. Timeline ne smije
  poznavati aplikaciju, aktivni shell tab ili workflow koji ga koristi.
- Ista granica vrijedi standalone i u shellu, Local/LAN/Intranet te na
  Windows/Linux/macOS. Modulni command/event ugovor nije veza za poslovnu
  suradnju izmedu aplikacija; njihova jedina poslovna veza ostaje baza.
- Obvezna v4 referenca: `C:\Users\miron\Projects\qnc_v4\AGENTS.md`
  (posebno odjeljak `Jedinstveni model`), `docs/qnc-timeline.md`,
  `qnc-app/src/qnc_timeline.rs`, `qnc_timeline_progress.rs`,
  `qnc_segment_timeline.rs`, `carrier_sync.rs` i `playback_stack.rs`.
  Zateceni pending/fallback prikaz ili app-specific routing u starom kodu
  nije dozvola za odstupanje od ovih pravila u novom QNC-u.
- Testovi granice moraju potvrditi da crtanje ne mijenja player/DB stanje,
  korisnicki unos samo emitira intent, a bez novih player podataka timeline
  samostalno ne pomice vrijeme ni playhead.

### 8.2. Broadcast Player je prvi playback modul

- Korisnicki redoslijed 2026-09-08: sljedeci razvojni korak je Broadcast
  Player kao samostalni javni out-of-process modul. Njegov stvarni runtime
  i javni command/event ugovor prethode integraciji timelinea, monitora,
  filmstripa i wave prikaza. Ne graditi playback pocevsi od UI komponente.
- Player posjeduje playback sat, ritam, play/pause lifecycle, frame-precizni
  seek, decode i sinkronizaciju video/audio izlaza kroz odvojene neutralne
  adaptere. Potvrdjeni runtime status i prezentirani frame dolaze od playera,
  ne iz forme, timelinea, monitora, client adaptera ili shella.
- Broadcast Player se gradi po profesionalnom NLE obrascu kakav koriste
  Premiere, Final Cut i Resolve: jedan playback engine je vlasnik sata,
  dekodiranja, frame redoslijeda i A/V sinkronizacije; Source/Program monitor,
  timeline, tipkovnica i UI dugmad su samo pasivni prikaz ili daljinski
  upravljac. Ne smiju brojati frameove, drzati vlastiti sat ni popravljati
  ritam reprodukcije.
- UI monitor nije broadcast signal. QNC mora razlikovati UI preview,
  clean/program output i buduce vanjske izlaze kao SDI/HDMI/NDI. Svi izlazi
  slusaju isti engine clock i isti source timebase; nijedan izlaz ne smije
  postati drugi player ili drugi vlasnik playback stanja.
- Pasivni UI preview je javni modul `qnc-monitor`. Svaka aplikacija ga smije
  ugraditi. Forma predaje samo potvrdjeni frame ili poruku; komponenta ne
  poznaje host aplikaciju, ne drzi sat, ne dekodira i ne cita bazu.
- Glavni playback put ne smije ovisiti o tome da se svaki frame salje kroz UI
  kao CPU RGBA tekstura. To je samo preview adapter. Profesionalni cilj je
  engine -> video output/GPU surface/clean output adapter, uz pasivni UI
  monitor koji prikazuje potvrdjeno stanje bez upravljanja satom.
- Korisnicki kriterij 2026-09-08: Play mora odmah pokrenuti vec pripremljenu
  reprodukciju. Odabir/ucitavanje klipa pokrece pripremu unutar player modula,
  izvan UI threada: otvaranje medija, dekodera i izlaza te ograniceni pocetni
  video/audio buffer. `Ready` se ne objavljuje prije dovrsene pripreme.
- Play nad spremnim klipom ne smije pokretati proces, otvarati medij ili audio
  uredaj, citati DB niti cekati pocetni decode/preroll. Pokrece postojeci sat
  i spremni izlaz. Pause cuva pripremljene resurse za nastavak. Promjena klipa
  ili seek koji ponisti buffer zahtijeva novu pripremu, ne lazni `Ready`.
- Mjeri se Play naredba -> prvi stvarni video/audio izlaz, odvojeno od vremena
  pripreme. Brzo otvaranje dekodera ili test s laznim izlazom nije dokaz
  trenutnog Playa. Isti kriterij vrijedi Local/LAN/Intranet; ako medij ili
  izlaz nije spreman, player to jasno prijavljuje umjesto laznog `Playing`.
- Javni player modul moze koristiti svaka aplikacija, bez caller allowliste
  i bez poznavanja njezina workflowa. Svaka sesija ima izolirano playback
  stanje; zajednicki modul ne smije postati shared workflow svih aplikacija.
- Broadcast Player radi samo playback. Nije QNC poslovna aplikacija i ne
  implementira Select, katalog, filmstrip, wave, import, formu ni shell.
  Okruzenje se prilagodava njegovom protokolu (`docs/84-broadcast-player-protocol.md`);
  player se ne prilagodava aplikaciji. `NotReady` pauzira sat, ne gasi proces
  ni command socket. Ingest `action_id` nije dio player protokola.
- Ulaz se priprema kroz javne read-only DB module iz postojecih projektnih
  postavki i spremljenog opisa konkretnog original/proxy medija. Player ne
  poznaje Projects ili Ingest aplikaciju i ne odredjuje aktivni projekt.
  QNC media URI razrjesava javni resolver/transport adapter, ne forma.
- `playback.input` iz baze odredjuje original/proxy izbor. Source frame rate,
  timebase, trajanje, scan/field mode, color opis i mapa video/audio streamova
  dolaze iz spremljenih media podataka. Project/export FPS, field order, color
  ili format nisu zamjena za source podatke tijekom play/montaze. Nedostajuci
  ili nepodrzani source podaci daju kontroliranu gresku, nikad novi probe,
  izmisljeni format ili hardkodirani default.
- Zabranjeno je uvoditi fiksni monitor refresh ili OS repaint kao playback
  pravilo. Player cadence mora dolaziti iz spremljenog source timebasea
  konkretnog klipa i player/audio clocka. Podrska za bilo koji stvarni source
  fps dopustena je samo kao source timebase, ne kao globalna pretpostavka
  aplikacije.
- Svaki javni player/monitor frame zapis mora nositi source timebase uz frame
  broj i identitet sourcea. Monitor, timeline i UI remote smiju prikazivati ili
  preskakati stale slike, ali ne smiju mijenjati cadence, izmisljati FPS niti
  zamijeniti source timebase postavkom ekrana, prozora ili projekta.
- Ingest sam cita postavke iz baze aktivnog projekta kroz javni read-only
  modul. Projects ni shell mu ih ne salju. DB-first vrijedi za cijelu
  aplikaciju i njene module, ne samo za katalog medija.
- Broj audio izlaza i zadana frekvencija uzorkovanja citaju se iz postojecih
  projektnih audio postavki. Player ih primjenjuje, ne zamjenjuje lokalnim
  JSON-om, UI izborom, brojem kanala snimke ili izmisljenim defaultom.
- Izvorni media zapis zadrzava sve odvojene kanale. Source preview cuva
  numeraciju kanala do broja zadanog projektom, bez automatskog stereo miksa.
  To ne odredjuje uloge montaze: u zadanom broadcast postupku A1 je OFF i
  izjava, A2 ambijent B-rolla. Te uloge nisu stereo par kamere. A1/A2 se u
  playeru moraju tretirati kao dva odvojena mono lanea iz spremljenog source
  inventara, i kad fizicki dolaze iz istog visekanalnog streama.
- Izbor proxy SLIKE ne smije zamijeniti originalni audio reduciranim proxy
  audiom. Izvorni identiteti, sample rate i timing ostaju iz spremljenog
  originala; nema novog probea. Razlicit projektni sample rate zahtijeva
  stvarnu pretvorbu ili jasnu gresku nepodrzanog formata, nikad preimenovanje
  izvornih uzoraka. Nepodrzan fizicki izlaz nije dozvola za tihi fallback.
  Docs/66 ispravlja prethodnu interpretaciju docs/64 i docs/65.
- Player nema scanner, Media Probe, filmstrip/wave generator, export, Project
  workflow niti vlasnistvo nad poslovnim DB zapisima. Generatori artefakata
  ostaju zasebni moduli; ovaj redoslijed nije naredba da ovise o playeru.
- Isti verzionirani ugovor mora vrijediti Local/LAN/Intranet. Lokalni pipe
  ili shared-memory adapter nije sam po sebi implementacija mreznog rada.
  Stanje, media pristup i prijenos video/audio izlaza moraju imati definirane
  transportne granice; fizicke putanje ostaju privatne adapteru.
- Monitor prikaz ne smije vuci velike RGBA frameove request/response pollingom
  preko istog kanala koji nosi player komande. Komande, state i frame transport
  moraju biti odvojeni.
- Lokalni monitor output Broadcast Playera koristi `qnc-player-frame-map`
  shared-memory/mmap ring kao obvezni pixel handoff. Ako frame-map ne postoji
  ili pukne, to je greska sesije; nije dopusten tihi fallback na socket slanje
  RGBA frameova.
- Player command/state socket smije nositi samo male kontrolne poruke i javno
  stanje. Ne smije postati skriveni monitor pixel kanal.
- Dekoder ne smije dobiti HTTP storage endpoint, credentials ili LAN/Intranet
  URL kao svoj ulaz. Storage adapter otvara `MediaStream` ili ga, kada owner
  ima privatni lokalni/montirani path, pretvara u seekable `CodecEndpoint`.
  FFmpeg CLI adapter smije dobiti samo seekable file endpoint. Session-private
  byte stream preko TCP/named-pipe/shared-memory adaptera smije koristiti samo
  decoder adapter koji eksplicitno deklarira da podrzava takav ne-javni procesni
  kanal i njegovu seek semantiku. Taj kanal nije poslovni media identitet, ne
  radi probe i ne uvodi fallback dekoder.
- Za stvarni playback lokalni monitor handoff mora biti ogranicen, sekvenciran
  i oznacen session/source/frame generacijom. Latest-only prikaz smije postojati
  samo kao eksplicitno degradirani preview ili thumbnail put; ne smije biti
  dokaz stabilnog broadcast playa jer skriva preskocene frameove i narusava
  1-frame preciznost.
- Spori UI ne smije blokirati playback sat, audio punjenje, decode ni player
  proces, ali ne smije ni silently gutati gubitak frameova bez dijagnostike.
  Lokalni mmap je samo lokalni adapter; LAN/Intranet izlaz mora imati svoj
  jednako pasivan transportni adapter s istim command/state/timebase ugovorom.
- V4 referenca je aktivni player model; u novom QNC-u taj sloj se zove
  `qnc-broadcast-engine` i koristi ga proces `qnc-broadcast-player`.
  Ne vracati
  arhivirani app player niti cijeli `qnc-media-ffmpeg` paket s probeom i
  generatorima kao player ovisnost. Prijenos aktivnog koda zahtijeva unaprijed
  naveden opseg i korisnicku potvrdu prema odjeljcima 2 i 10.
- Player nije zavrsen kada samo prihvaca naredbe ili pomice brojac. Potrebna
  je provjera stvarnog videa i audija, seek/pause/granica, izolacije sesija i
  rada bez probea. UI se spaja tek na provjereni javni playback put.
- Klik na drugi klip obvezno prekida reprodukciju prethodnog klipa.
  Prethodna player sesija mora biti ugasena prije pripreme nove, cak i ako
  novi klip nije spreman ili njegov zapis nije valjan. Nova selekcija ne
  nasljedjuje Play; stara slika, zvuk i zakasnjeli odgovori ne prelaze u nju.
- Dekoder je zamjenjiva implementacija javnog verzioniranog QNC ugovora.
  FFmpeg naredbe i njegovi interni formati ostaju u zasebnom adapteru, ne u
  engineu, formi ili neutralnom ugovoru. Engine ostaje jedini vlasnik sata.
- Konacni cilj decode sloja je capability lanac: prvo hardverski/GPU decode
  adapter ako host i source zapis to stvarno podrzavaju, zatim system decoder
  adapter, zatim external/software adapter. NVIDIA NVDEC/CUVID, Intel
  Quick Sync/QSV/oneVPL, AMD AMF/VCN/VAAPI, Apple VideoToolbox i buduci
  adapteri smiju se dodati samo kao javni adapteri preko kataloga. Ni jedan
  od njih ne smije biti hardkodiran u Ingest, player engine, filmstrip,
  timeline, formu ili shell.
- Dok QNC ne dobije funkcionalnu stabilnu aplikaciju, FFmpeg ostaje odobreni
  privremeni external/software adapter unutar istog zamjenjivog decode lanca.
  To nije dozvola da FFmpeg postane monolit, da se vrati v4 media paket, da se
  uvede novi probe ili da se zaobidje decoder catalog. Kasnije dodavanje GPU
  i system adaptera ne smije mijenjati javni player/filmstrip ugovor.
- Decoder capability odluka koristi spremljene source podatke iz baze:
  container, codec, profile, pixel format, bit depth, chroma, scan/field
  opis, timebase, trajanje i stream mapu. Ako adapter ne moze eksplicitno
  podrzati taj zapis, odbija request prije playbacka ili filmstrip posla.
  Nema tihog fallbacka koji pokusava drugi dekoder nakon djelomicnog playa.
- Local/LAN/Intranet storage smije biti privatno montiran na hostu koji dekodira.
  Javni zapis i dalje ostaje `qnc://...` URI; privatni path je samo resolver
  binding hosta. Ako udaljeni storage nije montiran i nema QNC decoder service
  adapter, player/filmstrip/wave moraju odbiti posao jasnom greskom, ne prelaziti
  na HTTP ili raw TCP decoder ulaz.
- Instalirani dekoderi i eksplicitni odabir dolaze iz konfiguracije hosta
  na kojem se dekodiranje izvrsava. To nisu nove projektne postavke niti
  poslovna veza aplikacija. Nema automatskog fallbacka na drugi dekoder.
- Vanjski dekoderski adapter mora potvrditi verziju procesnog ugovora,
  identitet i format izlaza te postovati spremljene podatke, ogranicenu
  memoriju i otkazivanje. Instalacija drugog adaptera ne dopusta novi probe.
  Isti media URI/transport ugovor vrijedi Local/LAN/Intranet; lokalni
  procesni kanal sam po sebi nije udaljeni decode servis.
- Klik na thumbnail odmah bira novi klip i monitor smije prikazati thumbnail
  samo dok Broadcast Player ne pripremi stvarni prvi frame. Kad javni player
  isporuci prvi frame, taj frame je kvalitetniji preview i smije zamijeniti
  thumbnail bez pokretanja Playa. Play je i dalje jedina naredba koja pokrece
  reprodukciju; Pause zadrzava video frame.
- Kad Play dodje do kraja aktivnog rangea ili klipa, player ne ostaje na
  zadnjem frameu. Mora prekinuti kretanje, vratiti carrier na pocetak rangea
  i pripremiti prvi frame kao preview za sljedeci Play.

### 8.3. Plan dovrsetka Broadcast Playera

Ovaj plan je obvezni redoslijed za dovrsetak stvarnog Broadcast Playera.
Ne preskakati ga zbog Filmstripa, Wavea, Timelinea, Monitora, Storyja ili
drugih aplikacija.

1. Live acceptance single-source Playa kroz stvarni Ingest:
   Mironik 2002 do kraja, Mironik 2679 do kraja, vise puta Pause/Play,
   bez zastoja, bez vidljivog trzaja i bez neprihvatljivog A/V pomaka.
2. Frame-precizne komande:
   step jedan frame natrag/naprijed, cue/seek na zadani frame, IN/OUT granice
   i kraj klipa s povratkom na start. Player smije potvrditi novu poziciju
   samo iz vlastitog javnog stanja, ne iz UI pretpostavke.
3. Promjena klipa:
   klik na drugi klip obvezno prekida staru sesiju i stari zvuk 100%.
   Novi thumbnail se prikazuje odmah samo dok prvi stvarni frame novog klipa
   nije spreman. Zakasnjeli frame, stanje ili odgovor stare sesije ne smije
   se prikazati u novoj selekciji.
4. Output ugovor:
   razdvojiti UI preview, buduci clean/program output i buduce SDI/HDMI/NDI
   adaptere. Svi izlazi slusaju isti player clock/source timebase ugovor;
   nijedan izlaz ne smije postati drugi player.
5. Audio ugovor:
   broj kanala i sample rate dolaze iz projektne baze, a source kanali ostaju
   odvojeni. Broadcast A1/A2 nisu stereo fallback nego dva mono lanea po
   spremljenom source inventaru. Svaki nepodrzani format mora dati jasnu
   gresku ili stvarnu dogovorenu konverziju u javnom adapteru.
6. Diagnostics i mjerenje:
   acceptance mora mjeriti Play naredbu do prvog stvarnog outputa, razmak
   prezentiranih frameova, queue/buffer stanje, A/V offset u vise tocaka klipa
   i razlog svakog prekida. Test s pomocnim fake rutinama nije dovoljan.
7. Local/LAN/Intranet:
   lokalni mmap/latest-frame adapter nije dokaz mreznog rada. Isti command,
   state, timebase i output ugovor mora dobiti LAN/Intranet transport adapter
   prije tvrdnje da je Broadcast Player gotov za sve QNC okoline.
8. Tek nakon ovoga smiju ici Timeline, Filmstrip i Wave integracije koje ovise
   o stabilnom player stanju. Timeline ostaje pasivni UI remote, Filmstrip i
   Wave ostaju pasivni artefakti/prikazi iz baze.

## 9. Local/LAN/Intranet

- Sve mora raditi na laptopu, lokalnoj mrezi i intranetu.
- Fizička lokacija medija nije workflow istina.
- Player, export, timeline i druge aplikacije ne smiju pretpostaviti lokalni
  filesystem.
- Pristup DB i media lokacijama ide preko resolvera/transporta.
- Isti ugovor mora vrijediti za lokalni disk, LAN server i intranet.
- Aplikacija ne smije otvoriti DB ili media file preko sirovog path join-a kao
  javnog koraka. Javni pristup mora proci kroz QNC URI -> resolver -> endpoint.
- Resolver binding `URI -> privatna datoteka` smije postaviti samo owner
  aplikacija. Binding nije javni identitet.
- LAN/Intranet authority dolazi iz owner registry/runtime konfiguracije, ne iz
  hardkodiranog URL-a u UI-ju.
- Dir Browser javni output mora biti QNC URI odabrane lokacije, ne raw OS path.
- Dir Browser mora biti samostalni javni modul, ne dio Project/Ingest/Story
  forme.
- Aplikacijska forma ne smije imati vlastiti browser state koji zna za OS
  filesystem. Forma smije samo prikazati browser view/state koji je dosao iz
  Dir Browser modula i poslati `action_id` natrag komponenti.
- Potvrdna/odustajna dugmad oko browsera, npr. `Odaberi`, `U redu` i
  `Odustani`, nisu dio javnog `qnc-dir-browser` modula i nisu dio browser
  session statea. Ta dugmad pripadaju aplikacijskoj UI akcijskoj traci ili
  zajednickom UI paint modulu.
- Standardna potvrdna akcijska traka za forme mora ici kroz javni UI modul
  `qnc-ui-kit`, ne kroz lokalno duplicirane helper funkcije u pojedinim
  aplikacijama.
- Ako jedna forma koristi isti javni browser vise puta, npr. za lokaciju
  projekta i export lokaciju, forma/komponenta mora upravljati vlastitim
  panelima tako da je otvoren samo jedan browser prikaz odjednom. Otvaranje
  jednog browser panela automatski zatvara drugi.
- To ekskluzivno otvaranje nije odgovornost `qnc-dir-browser` modula.
  `qnc-dir-browser` ne smije znati koliko browser instanci neka aplikacija
  prikazuje.
- Browser navigacija, npr. source tabovi, `Gore`, `Diskovi`, breadcrumb i rows,
  smije biti UI paint komponenta, ali stvarni state/listing/URI mora doci iz
  Dir Browser modula.
- OS-specific display detalji, npr. Windows extended path prefix `\\?\`, ne
  smiju izlaziti u UI. Dir Browser modul mora vratiti OS-neutralan prikaz i
  QNC URI kao javni identitet.
- Raw path smije postojati samo unutar Dir Browser/resolver procesa ili u
  privatnim runtime tablicama owner aplikacije.
- `rfd` i OS dialog smiju zivjeti samo unutar Dir Browser modula ili jednakog
  modula za OS izbor lokacije, ne u aplikacijskoj formi.
- Dir Browser `dir.list` i `dir.select` capabilityji moraju postojati prije
  nego se modul smatra implementiranim.

## 10. UI pravila

- Postojeci native egui UI iz starog QNC-a mora se doslovno preslikati kada se
  prenosi u novi QNC.
- Prije kodiranja bilo koje forme, panela, layouta, dialoga, browsera,
  timelinea, player kontrola ili shell prikaza obavezno se mora uzeti kao
  referenca postojeci UI i layout iz starog projekta:
  `C:\Users\miron\Projects\qnc_v4`.
- Referenca ukljucuje postojece ekrane, raspored elemenata, poravnanja,
  razmake, panel strukturu, navigaciju, fokus, keyboard ponasanje, font i
  vizualni stil.
- UI/layout iz `qnc_v4` je obvezni vizualni i strukturni izvor za novi QNC i
  mora se preslikati doslovno: isti raspored, isti redoslijed elemenata, isti
  nazivi, isti font sustav, isti razmaci, ista poravnanja, isti fokus i isto
  keyboard ponasanje.
- Redizajn, reinterpretacija, uljepsavanje ili promjena layouta nisu dozvoljeni
  bez izricitog odobrenja.
- Ako je odstupanje tehnicki neizbjezno zbog razdvajanja monolita na
  aplikacije/module, odstupanje mora biti minimalno, zapisano prije
  implementacije i potvrdeno live testom.
- UI kod i layout kod smiju se koristiti kao izvor za doslovno preslikavanje.
  Aktivna poslovna logika iz starog koda ne smije se prenijeti bez razdvajanja
  na aplikaciju/modul i bez jasnog odobrenja.
- Svako odstupanje od postojeceg UI/layout ponasanja mora biti eksplicitno
  zabiljezeno prije implementacije i ne smije se tretirati kao slobodna
  dizajnerska odluka.
- Jedan osnovni font sustav koristi se kroz aplikaciju.
- QNC keyboard shortcut ugovor je obvezna vanjska datoteka:
  `C:\Users\miron\Projects\QNC\contracts\qnc-keyboard-shortcuts.json`.
- Referentni izvor za pocetni QNC keyboard shortcut contract je stari QNC v4
  katalog: `C:\Users\miron\Projects\qnc_v4\seed\keyboard-shortcuts.json`.
- Tijekom kodiranja UI-ja, akcija, menija, playera, timelinea, browsera ili
  bilo kojeg input handlinga prvo se mora provjeriti i koristiti QNC keyboard
  shortcut ugovor.
- Shortcuti se ne smiju hardkodirati direktno u aplikacijski ili modulni kod.
- Kod smije ucitati shortcut preko keymap/shortcut modula i action id-a.
- OS specificne razlike tipki smiju postojati samo u vanjskoj shortcut
  datoteci, ne razbacane po kodu.
- Shortcut dispatch je obavezan: input -> shortcut modul -> `action_id` ->
  intent. Ucitani katalog bez dispatcha nije dovoljan.
- Mapiranje egui/OS tipke na catalog `key`/`code` ne smije ovisiti o Debug
  ispisu widgeta. Mora biti eksplicitna tablica ili catalog polje.
- Svaka nova tipkovna akcija mora imati `action_id` u
  `contracts/qnc-keyboard-shortcuts.json` prije koda.
- Theme boje, font i chrome mjere dolaze iz UI layout contracta.
- Soft/HighContrast i slicne teme ne smiju imati hardkodirane RGB vrijednosti u
  aplikacijskom kodu ako nisu u contractu.
- Shell footer tabovi dolaze iz registryja, a geometrija iz
  `contracts/ui/shell.layout.json`.
- Close project i workspace status smiju izostati samo uz zapis u
  `docs/10-shell-ui-implementation-note.md` ili noviji UI note prije
  implementacije.
- Forme ne sadrze poslovnu logiku.
- Forma i njezin desktop/UI crate ne smiju sadrzavati unit/integration testove
  niti dijagnosticki panel. Analiza runtime ponašanja ide kroz javni
  `qnc-dev-diagnostics` i samostalnu diagnostic tool aplikaciju koja cita
  logove. Ciljani testovi zive u modulima, conformance alatu ili diagnostic
  checkovima, ne u formi.
- UI smije prikazati stanje, korisnicki izbor, gresku i napredak.
- UI smije poslati neutralnu naredbu.
- UI ne smije raditi scan, probe, filmstrip, waveform, render, player decode ili
  drugu media obradu. UI smije koristiti pasivne UI module za prikaz.

## 11. Media Assist i Story varijante

- Media Assist je valjana zasebna aplikacija/template za daljnji razvoj.
- Media Assist nije dio jedne Story aplikacije.
- Moze postojati neogranicen broj Story aplikacija ili Story varijanti za razne
  korisnicke potrebe.
- Ne smiju se sve Story potrebe zbijati u jednu aplikaciju.
- Story i Media Assist smiju koristiti iste neutralne module ako to ne krsi
  DB-first zakon.
- Dijeljeni kod je dozvoljen; monolit i direktna poslovna veza izmedu aplikacija
  nisu dozvoljeni.

## 12. Modulna klasifikacija

- Aplikacije/forme: Project, Ingest, Media Assist, Story i buduce aplikacije.
- Shell nije stavka u listi aplikacija/formi. Shell je host.
- Moduli: Filmstrip, Wave, Broadcast Player, Export, Media Probe, Media Browser,
  Timeline, Monitor, Dir Browser, Keyboard/Shortcut, resolver, scanner, codec
  adapter, UI widget, DB adapter, test adapter i slicni gradivni dijelovi.
- Modul je javno dostupan svim aplikacijama kroz svoj contract.
- Dir Browser, resolver i keyboard/shortcut ostaju javni moduli za sve
  aplikacije, bez liste dozvoljenih korisnika.
- Modul ne nosi hardkodiranu listu dozvoljenih aplikacija.
- Aplikacija nosi workflow zabrane: sto ne smije pokrenuti ili koristiti u
  svojem workflowu.
- Modul nosi dependency zabrane: sto on sam ne smije pozvati, ucitati ili
  pokrenuti.
- Zabrane nisu lista dozvoljenih korisnika modula.
- Modul moze biti zajednicki ako nema vlasnistvo nad tudim workflowom.
- Modul ne smije postati centralni host koji zna poslovnu logiku svih
  aplikacija.
- Modul ne smije biti tajna veza izmedu aplikacija. Ako aplikacije razmjenjuju
  poslovne podatke, to ide kroz bazu.
- Sve sto se ponavlja u vise aplikacija i moze imati uski contract treba
  izdvojiti kao javni modul ili genericku javnu UI komponentu, npr. `qnc-ui-kit`,
  `qnc-dir-browser`, keyboard, resolver, media browser ili timeline paint.
- Javni UI modul, ukljucujuci `qnc-ui-kit`, smije sadrzavati samo pasivne
  paint/layout/intent obrasce i genericke UI-state pomocnike, npr.
  ekskluzivno otvaranje panela. Ne smije imati aplikacijski workflow, DB ownera,
  filesystem ownership, browser session ownership, media scan/probe/player/
  render/export logiku niti centralni registry poslovnog stanja.
- Javni modul ili komponenta ne smije se siriti dodavanjem posebnih pravila za
  Project, Ingest, Story ili Media Assist. Ako mu treba takvo znanje, modul je
  postao monolit i mora se razbiti na uzi contract.
- Aplikacijska komponenta koja pocne sadrzavati vise nepovezanih aktivnih
  odgovornosti mora se razbiti. Nije dozvoljeno sakriti monolit pod nazivom
  `components`. Ingest nema svoj `components` sloj; Ingest aplikacijski sloj
  smije biti samo composition/root za javne module i treba se dalje smanjivati
  izdvajanjem svake aktivne odgovornosti u uski javni modul.

## 13. Razvojni redoslijed

- Ne razvijaju se svi moduli odmah.
- Prvo se definiraju ugovori: application/form contract, module contract, DB
  contract, transport URI contract, workflow boundary i dependency boundary.
- Pocetni ugovorni dokumenti nalaze se u `C:\Users\miron\Projects\QNC\docs`.
- Pocetna modulna contract matrica je
  `C:\Users\miron\Projects\QNC\docs\04-module-contract-matrix.md`.
- Pocetni conformance test plan je
  `C:\Users\miron\Projects\QNC\docs\05-conformance-test-plan.md`.
- Obvezni vanjski QNC keyboard shortcut ugovor je
  `C:\Users\miron\Projects\QNC\contracts\qnc-keyboard-shortcuts.json`.
- Obvezni UI/layout referentni ugovor je
  `C:\Users\miron\Projects\QNC\docs\07-ui-layout-reference.md`.
- Konkretni UI layout contracti nalaze se u
  `C:\Users\miron\Projects\QNC\contracts\ui`.
- Obvezni Project system seed nalazi se u
  `C:\Users\miron\Projects\QNC\seed\system_seed.json`.
- Project UI layout contract je
  `C:\Users\miron\Projects\QNC\contracts\ui\project.layout.json`, nastao iz
  audita `qnc_v4` Project forme.
- Nakon ugovora razvija se najmanji skup neutralnih zajednickih modula bez
  poslovne logike aplikacija.
- Prvi zajednicki moduli su: manifest/capability modul, transport/resolver
  modul, DB contract/validation modul, keyboard/shortcut modul i
  frame/timebase modul.
- Nakon osnovnih ugovora i neutralnih modula razvija se Project aplikacija.
- Nakon Project aplikacije razvija se Ingest aplikacija, jer ona puni bazu
  izvornim media podacima.
- Dir Browser smije se razviti uz Project jer Project bira lokacije. To ne
  otvara Ingest/probe/filmstrip workflow.
- Tek kada postoji baza koju moduli mogu citati, razvijaju se media moduli:
  Media Browser, Media Probe, Filmstrip, Wave, Broadcast Player, Export,
  Timeline, Monitor i drugi potrebni moduli.
- Gornji popis nije redoslijed implementacije. Nakon postojeceg Ingest DB
  puta korisnik je odredio Broadcast Player kao prvi sljedeci runtime modul;
  daljnji playback/UI razvoj slijedi odjeljak 8.2.
- Story aplikacije/varijante i Media Assist razvijaju se tek nakon sto postoje
  stabilni Project/Ingest DB contracti i potrebni media moduli.
- Modul bez ugovora se ne razvija.
- Aplikacija bez definirane baze, ownera i workflow boundaryja se ne razvija.
- Application manifest `module_dependencies` mora imati odgovarajuci crate ili
  runtime komponentu i stvarnu upotrebu u runtimeu. Conformance to mora odbiti
  kada manifest i kod nisu uskladeni.
- Prvi prakticni kodni rez smije biti samo neutralni contract/manifest/URI
  sloj ili test koji cuva ta pravila.

## 14. Project freeze

Zamrznuto na korisnikov zahtjev 2026-09-04.

Pojasnjenje korisnika 2026-09-08: zamrznut je Project programski kod i dolje
navedeni razvojni ugovori, NE projektni radni direktoriji i podaci. Ingest i
drugi owneri smiju zapisivati vlastite rezultate u njih kroz javne module.
To ne dopusta promjenu Project postavki niti ukida zastitu od slucajnog
brisanja. Izvorna kartica i dalje ostaje read-only. Ne traziti odmrzavanje
Project koda samo zato sto se posao zapisuje u direktorij ili bazu projekta.

Ponovno zamrznuto na izriciti korisnikov zahtjev 2026-09-06, nakon live
potvrde navigacije na sljedecu odabranu grupu. Prethodno ograniceno
odmrzavanje za popravke i katalog je zatvoreno. Nema aktivnog odobrenja za
promjene Projecta. Razvoj Ingesta nije dozvola za njegovo otkljucavanje.
Detalji su u `docs/11-project-freeze.md`.

Zatvoreno ograniceno odobrenje 2026-09-06: korisnik je izricito otkljucao Project za
integraciju javnog modula identiteta radne stanice/korisnika i zapis metapodataka
porijekla novog projekta. Ocitavanje OS podataka mora ostati u javnom modulu.
Na popisu projekata prikazuju se samo naziv i datum; ostali podaci su samo u
bazi. Ovo nije odobrenje za session routing, LAN sinkronizaciju ili druge
promjene Projecta. Zahvat je verificiran (docs/20), odobrenje je zatvoreno i
Project je ponovno zamrznut. Nova promjena zahtijeva novu izricitu dozvolu.

- Project aplikacija je zavrsena za trenutni razvojni korak i ne smije se
  mijenjati bez izricite korisnicke dozvole.
- Opci zahtjevi poput "nastavi", "idemo dalje", "sredi QNC", "dodaj Ingest",
  "dodaj Story" ili slicni nastavci ne daju dozvolu za promjene Projecta.
- Dozvola mora izricito imenovati Project i vrstu promjene, npr.
  "otkljucaj Project za promjenu export lokacije" ili "promijeni Project UI".
- Bez takve dozvole zabranjeno je mijenjati:
  `apps/qnc-project/**`,
  `apps/qnc-project/qnc-app.json`,
  `crates/qnc-project-desktop/**`,
  `crates/qnc-project-store/**`,
  `crates/qnc-project-desktop-adapter/**`,
  `contracts/applications/project.application.json`,
  `contracts/databases/project-registry.database.json`,
  `contracts/databases/project-workspace.database.json`,
  `contracts/ui/project.layout.json`,
  Project dijelove `contracts/qnc-keyboard-shortcuts.json`,
  Project template/seed dijelove `seed/system_seed.json`,
  i Project conformance pravila u `tools/qnc-conformance/**`.
- Dopušteno je bez izmjene Projecta citati kod, raditi audit, pokretati
  testove, pokretati live aplikaciju i razvijati druge aplikacije/module ako ne
  mijenjaju zamrznuti Project scope.
- Ako buduca promjena druge aplikacije zahtijeva promjenu Projecta, rad treba
  zaustaviti i prvo traziti izricitu dozvolu za otkljucavanje Projecta.
- Detaljni freeze zapis mora postojati u
  `C:\Users\miron\Projects\QNC\docs\11-project-freeze.md`.
- Ingest i moduli koje koristi zamrznuti su odvojeno, odjeljak 17 i
  `docs/85-ingest-freeze.md`. Razvoj Ingesta vise nije otvoren korak.

## 15. Verifikacija

Zatvoreno ograniceno odobrenje 2026-09-06: korisnik je potvrdio dopunu
`public_project_settings` postojecim `settings_json` radi read-only citanja
radnih postavki. Nema novih Project podataka, aktivacije ni UI promjena.
Opseg i zatvaranje odobrenja vode se u docs/11 i docs/21.

Zatvoreno naknadno ograniceno odobrenje 2026-09-06: Project u shell baru prikazuje naziv
selektiranog projekta kroz isti javni desktop status kao Ingest. Samo prikaz
postojece selekcije, bez promjene DB-a, aktivacije ili workflowa; docs/11 i docs/21.
Ciljani testovi i Windows live prikaz potvrdjeni. Project je ponovno zamrznut.

- Nakon svakog implementacijskog koraka ide ciljani test.
- Za vidljivo ponasanje ide live test prije nastavka.
- Za UI/layout promjene obavezna je usporedba s relevantnim postojecim UI-jem
  iz `C:\Users\miron\Projects\qnc_v4`.
- UI/layout test prolazi samo ako je novi prikaz doslovna preslika relevantnog
  starog prikaza ili ako je svako odstupanje unaprijed zabiljezeno i odobreno.
- UI/layout live test iz `docs/07-ui-layout-reference.md` i
  `docs/05-conformance-test-plan.md` je uvjet zatvaranja koraka, ne opcionalni
  dodatak.
- Pravila treba kasnije pretvoriti u automatske testove gdje god je moguce.
- Audit nije zavrsen ako nije navedeno sto je provjereno, sto nije provjereno i
  koji je sljedeci rizik.

## 16. Trenutna odobrena odstupanja

Zapisano 2026-09-04, azurirano 2026-09-12. Vrijedi samo dok se odstupanja ne
zatvore. Ovaj odjeljak ne smije proturjeciti strogim pravilima iznad. Ako se
neka stavka u ovom odjeljku pokaze zastarjelom, ne koristiti je kao dozvolu za
novi kod; prvo je ispraviti prema stvarnom stablu i ovom zakonu.

- Shell runtime footer prikazuje samo aplikacije s `apps/*/qnc-app.json`.
  Layout contract i dalje nabraja `project`, `ingest`, `media_assist` i
  `storyboard`.
- Close project postoji u shell footeru samo kao poziv javnog modula
  `qnc-project-close`, prema odjeljku 3. To nije Project workflow i nije
  dozvola shellu da brise, cisti, zatvara aplikacije ili preuzima poslovno
  stanje.
- Shell embedded factory trenutno ima `qnc_project` i `qnc_ingest` javne
  adaptere. To nije dozvola za uvodenje privatnih app/store ovisnosti u shell i
  nije uzor za monolit.
- In-process shell adapter smije tranzitivno povuci desktop/component/store
  sloj iste aplikacije samo dok je to javni adapter te iste aplikacije.
  Adapter ne smije povuci privatni workflow druge aplikacije.
- Project je 2026-09-06 ponovno zamrznut nakon odobrenog zahvata.
  Prethodno odmrzavanje vise ne daje dozvolu za izmjene; vrijedi odjeljak 14.
- Project `Odaberi...` ne smije koristiti OS folder dialog; mora koristiti
  ugradeni Dir Browser prikaz s `Racunalo / LAN / Internet`, `Gore`, `Diskovi`,
  `U redu` i `Odustani` akcijama.
- Dir Browser prvi rez za Project smije privremeno potvrditi privatni lokalni
  path u owner Project postavke. Javni identitet lokacije mora ostati QNC URI.
- Ingest `Odaberi` za konfigurirane izvore pokrece source scan, potvrdeno
  original/proxy grupiranje, citanje camera zapisa, jedini potrebni probe prolaz
  i source/media DB upis kroz javni `qnc-ingest-select` modul i njegove javne
  dependency module (docs/34). Postojeci zavrseni zapisi citaju se bez novog
  probea. Prikaz karticnih slicica koristi eksplicitne DB veze i read-only
  transport (docs/35). To ne znaci da su kopiranje medija, filmstrip, waveform,
  playback ili sve camera sheme implementirani.
- Ingest application manifest smije deklarirati samo module i capabilityje koji
  imaju stvarnu runtime ovisnost ili implementirani javni adapter u trenutnom
  rezu. Scanner, camera detector, Media Probe, Media Browser, Filmstrip, Wave,
  Timeline, Broadcast Player i njihovi adapteri smiju biti u manifestu samo ako
  stvarno postoje u runtimeu i koriste se kroz javni ugovor. Manifest ne smije
  najaviti nepostojeci modul kao zavrsenu funkciju.
- Broadcast Player nije zatvoren dok ne prodje prihvat iz odjeljka 8.3. Od
  2026-09-13 taj nastavak je zamrznut zajedno s Ingestom (odjeljak 17). Vec
  spojeni pasivni Timeline/Filmstrip/Wave prikazi smiju ostati kao razvojni
  prikaz samo ako ne preuzimaju player sat, ne rade novi probe, ne pisu direktno
  u bazu i ne konkuriraju playeru za vrijeme `Preparing` ili `Playing`. Ne
  siriti Filmstrip/Wave/Timeline/player funkcije bez izricitog otkljucavanja.
- Ingest player sloj (`prepare_preview`, lokalni Guard) se ne siri. Zamrznuto
  odjeljkom 17. Convert-queue krpice nisu nova arhitektura. Daljnji spoj je
  samo daljinski i samo nakon otkljucavanja.
- Project keyboard dispatch smije krenuti od `project_open_selected`. Nove tipke
  ne smiju ici mimo kataloga.
- Ingest klik akcije moraju imati action_id u
  `contracts/qnc-keyboard-shortcuts.json` prije nego ih forma posalje. Tipke za
  Ingest smiju se dodati samo kroz taj katalog.
- Soft/HighContrast theme varijante smiju ostati samo dok se ne prebace u UI
  contract.

Sve ostalo iz ovog filea ostaje na snazi. Ova lista nije dozvola za redizajn
ili novi monolit.

## 17. Ingest freeze

Zamrznuto na izriciti korisnikov zahtjev 2026-09-13: kompletan `qnc-ingest` i
javne komponente/moduli koje Ingest koristi. Detalj: `docs/85-ingest-freeze.md`.
Zatvoreno 2026-09-13: uklonjeno host GPU vezanje. Monitor ostaje javni pasivni
modul; shell i Ingest forma ne vežu GPU. Preview ide kroz egui adapter.
Broadcast Player nije diran.

Zatvoreno 2026-09-13: Ingest startup vise ne skenira diskove. Povrsina ide od
`shell_next_group`; aktivni projekt se cita iz baze. Freeze ponovno vrijedi.

- Ingest aplikacija/forma i njezini owner crateovi, ugovori i Ingest dijelovi
  conformancea ne smiju se mijenjati bez izricite dozvole.
- Isti freeze vrijedi za javne module koje Ingest stvarno koristi, ukljucujuci
  Dir Browser, keyboard/shortcut, `qnc-ui-kit`, work-settings, Select/katalog/
  work-plan, store, scanner/camera/probe lanac, thumbnail/image, Timeline,
  Monitor, Filmstrip, Wave, Timeline-assets, player-input/launcher/client/
  contract/frame-transport, Broadcast Player proces i `qnc-broadcast-engine`,
  te decoder/media/audio/video/transport crateove na tom putu.
- `qnc-ingest-components` i dalje ne smije postojati. Freeze nije dozvola za
  novi umbrella.
- Opci zahtjevi i otvoreni §8.3 ne daju dozvolu. Dozvola mora imenovati Ingest
  ili tocno ime zamrznutog modula i vrstu promjene.
- Bez takve dozvole zabranjeno je mijenjati:
  `apps/qnc-ingest/**`,
  `crates/qnc-ingest-desktop/**`,
  `crates/qnc-ingest-desktop-adapter/**`,
  `crates/qnc-ingest-application/**`,
  `crates/qnc-ingest-store/**`,
  `crates/qnc-ingest-select/**`,
  `crates/qnc-ingest-catalog/**`,
  `crates/qnc-ingest-work-plan/**`,
  `contracts/applications/ingest.application.json`,
  `contracts/databases/ingest-registry.database.json`,
  `contracts/databases/ingest-content.database.json`,
  `contracts/databases/source-index.database.json`,
  `contracts/databases/media-records.database.json`,
  `contracts/ui/ingest.layout.json`,
  Ingest dijelove `contracts/qnc-keyboard-shortcuts.json`,
  Ingest conformance u `tools/qnc-conformance/**`,
  `crates/qnc-dir-browser/**`,
  `crates/qnc-keyboard-shortcut/**`,
  `crates/qnc-ui-kit/**`,
  `crates/qnc-work-settings/**`,
  `crates/qnc-monitor/**`,
  `crates/qnc-timeline/**`,
  `crates/qnc-timeline-assets/**`,
  `crates/qnc-player-timeline/**`,
  `crates/qnc-filmstrip/**`,
  `crates/qnc-filmstrip-worker/**`,
  `crates/qnc-wave/**`,
  `crates/qnc-wave-worker/**`,
  `crates/qnc-wave-view/**`,
  `crates/qnc-player-client/**`,
  `crates/qnc-player-input/**`,
  `crates/qnc-player-launcher/**`,
  `crates/qnc-player-contract/**`,
  `crates/qnc-player-frame-transport/**`,
  `crates/qnc-broadcast-player/**`,
  `crates/qnc-broadcast-engine/**`,
  `tools/qnc-player-runner/**`,
  `crates/qnc-decoder-catalog/**`,
  `crates/qnc-media-decode/**`,
  `crates/qnc-media-stream/**`,
  `crates/qnc-media-probe/**`,
  `crates/qnc-media-thumbnail/**`,
  `crates/qnc-media-metadata/**`,
  `crates/qnc-media-metadata-compose/**`,
  `crates/qnc-media-record-db/**`,
  `crates/qnc-media-records/**`,
  `crates/qnc-ffprobe-metadata/**`,
  `crates/qnc-image-assets/**`,
  `crates/qnc-source-reader/**`,
  `crates/qnc-source-groups/**`,
  `crates/qnc-source-index-db/**`,
  `crates/qnc-scanner/**`,
  `crates/qnc-camera-detector/**`,
  `crates/qnc-camera-patterns/**`,
  `crates/qnc-sony-metadata/**`,
  `crates/qnc-audio-output/**`,
  `crates/qnc-video-output/**`,
  `crates/qnc-ffmpeg-decode/**`,
  `crates/qnc-gpu-raster/**`,
  `crates/qnc-pixel-convert/**`,
  `crates/qnc-transport-resolver/**`,
  `crates/qnc-json-transport/**`,
  `crates/qnc-db-contract/**`,
  `crates/qnc-dev-diagnostics/**`,
  i odgovarajuce `contracts/modules/*.module.json` tih modula.
- Shell host i svi preostali crateovi/alati spadaju u odjeljak 18.
- Dopusteno je citati kod, auditirati, pokretati testove i live Ingest.
- Ako Story ili druga aplikacija treba izmjenu zamrznutog javnog modula, rad
  stati i traziti otkljucavanje tog modula. Ne praviti privatnu kopiju.

## 18. Freeze cijele QNC obitelji

Zamrznuto na izriciti korisnikov zahtjev 2026-09-13: cijeli QNC projekt,
sve aplikacije/forme, shell, svi javni moduli, alati, ugovori i dokumenti
razvoja. Detalj: `docs/86-family-freeze.md`. Odjeljci 14 i 17 ostaju na snazi
i strozi su za svoj opseg; ovaj odjeljak zatvara sve sto oni nisu imenovali.

Zatvoreno ograniceno odobrenje 2026-09-19 (dvadeseto): korisnik je izricito otkljucao
sav kod potreban za rjesavanje nalaza vanjskog audita Ingesta (monoliti
`qnc-ingest-application` i `qnc-ingest-select`; uske javne host komponente; jedan
preview host; osvjezavanje artefakata istog klipa; determinsticki cancel Selecta;
uvoz izvan applicationa), do rjesenja svih nalaza. Otkljucano: crateovi obitelji Ingest
(`qnc-ingest-application`, `qnc-ingest-select`, `qnc-ingest-store`,
`qnc-ingest-import-worker`, `qnc-ingest-catalog`, `qnc-ingest-work-plan`), `qnc-camera-*`,
`qnc-source-preview`, `qnc-content-read`, `qnc-timeline-assets`, `qnc-media-thumbnail`,
`qnc-source-browse`, `qnc-dir-browser`, novi crateovi i ugovori modula, testovi,
`Cargo.toml`/`Cargo.lock`, dokumenti, `tools/qnc-conformance`
samo ako nova granica to trazi. **Layout i UI ostaju zakljucani**: `qnc-ingest-desktop`,
`qnc-project-desktop`, `qnc-editorial-desktop`, `qnc-media-card`, `qnc-media-pool-head`,
`qnc-source-dock`, `qnc-ui-kit`, `contracts/ui`. Pravilo: `qnc-ingest-application` i
`qnc-ingest-select` se ne prosiruju novom logikom, samo se smanjuju; nova logika ide u uske
javne crateove. Sve ostalo ostaje zamrznuto.
Izvedeno (dvadeseto): `qnc-ingest-select` podijeljen u module `scan`, `records`, `metadata`,
`publish` s tankim vozacem; Select cancel ceka nit ogranicen broj ms i baca kasne evente; isti klip
osvjezava artefakte; kamere u `qnc-camera-*` i `qnc-ingest-cameras`; uvoz u
`qnc-ingest-import-worker` (`Importer`, najam s otkucajima za zapete `Processing` klipove);
novi javni crateovi: `qnc-timeline-artifacts` i `qnc-ingest-artifacts`,
`qnc-ingest-preview-source` (Ingest source adapter; preview je neutralni
`qnc-source-preview` s citacima kroz traitove), `qnc-ingest-clip-list`,
`qnc-ingest-selection-write`, `qnc-source-browse`, `qnc-playback-priority`,
`CatalogLoader` u `qnc-ingest-catalog`.
`qnc-ingest-application` vise ne drzi player, filmstrip/wave radnike, import, pisanje odabira,
browsing ni ucitavanje kataloga; ostaje dispatch, mapiranje u view i `confirm_source_selection`.
Nije izvedeno: kopija postera (traži odluku o DB ugovoru), izbor citaca indeksa po
pattern-id u `qnc-scanner` (nije otkljucan), test kroz cijelu aplikaciju i na stvarnoj kartici.
Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-19 (dvadeset prvo): korisnik je izricito odobrio
(a) kopiju postera pri uvozu, varijanta A: uvoz kopira poster u direktorij projekta samo kad
kopira proxy ili original, `FinishImport` nosi `thumbnail_uri`, store upisuje URI kopije u
`thumbnail_uri` klipa, a ponovni Select ne vraca URI postera klipa u statusu `imported` na
karticu; (b) odmrzavanje `qnc-scanner` (i po potrebi `qnc-source-groups`) za izbor citaca
indeksa po pattern-id, tako da dva adaptera s istim XML namespaceom ne daju
`ReaderAmbiguous`. Otkljucano: `qnc-ingest-store`, `qnc-ingest-import-worker`,
`qnc-ingest-application`, `qnc-ingest-catalog`, `qnc-ingest-select`, `qnc-camera-adapter`,
`qnc-scanner`, `qnc-source-groups`, testovi, `Cargo.toml`/`Cargo.lock`, ugovori modula,
dokumenti. UI i layout ostaju zakljucani (`qnc-ingest-desktop`, `qnc-media-card` i ostali
popisani u dvadesetom). Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-20 (dvadeset drugo): korisnik je izricito odkljucao
(a) `qnc-ingest-desktop` samo za podjelu `widgets.rs` u module bez ikakve promjene izgleda,
rasporeda ili ponasanja (cisto premjestanje koda); (b) prijenos shellu nakon radnje Uvezi:
`qnc-shell-desktop-api`, `crates/qnc-ingest-desktop-adapter`, `apps/qnc-app` i `apps/qnc-ingest`
samo koliko je nuzno da aplikacija nakon Uvezi javi shellu da je radnja gotova i da shell
pokrene sljedecu aplikaciju (kao `go_workflow(Next{from:"ingest"})` u v5); (c)
`qnc-ingest-application`: odabir klipova je lokalna oznaka u prikazu (bez upisa u bazu pri
kliku), a odabir se provjerava i upisuje u bazu tek na Uvezi; podjela `lib.rs` u module. Sve
ostalo ostaje zamrznuto, a izgled i raspored (`contracts/ui`, `qnc-ui-kit`, `qnc-media-card`,
`qnc-source-dock`, `qnc-media-pool-head`, `qnc-project-desktop`, `qnc-editorial-desktop`) ostaju
zakljucani.

Zatvoreno ograniceno odobrenje 2026-09-20 (dvadeset trece): korisnik je izricito odkljucao
`crates/qnc-filmstrip-worker` (i po potrebi `crates/qnc-ffmpeg-decode` te `crates/qnc-filmstrip`)
samo radi brzine stvaranja filmstripa: broj istodobnih radnika, nacin ffmpeg dohvata kadrova i
suvisni rad po klipu, bez promjene izgleda filmstripa, ugovora ni baze. Probe se ne uvodi:
filmstrip koristi metapodatke iz baze (XML kartice). Sve ostalo ostaje zamrznuto.
Izvedeno (21.-23.): poster se kopira uz medij (`ingest/thumbnails/<clip>/poster.jpg`, URI u `thumbnail_uri`,
ponovni Select ga ne vraca na karticu); odabir klipova je lokalna oznaka, upis u bazu i red uvoza tek
na Uvezi (`qnc-ingest-selection-write`); Uvezi pokrece pozadinsku aplikaciju `tools/qnc-ingest-worker`
(postavke projekta se citaju jednom, link ne kopira, jedan primjerak, ceka dok radi player kroz
`qnc-playback-marker`) i javlja shellu prijelaz (`take_navigation_request`, slijed iz javnog prikaza
`public_project_application_sequence` kroz `qnc-application-sequence`); `qnc-ingest-application` i
`qnc-ingest-desktop` podijeljeni na jednu datoteku po akciji/tipu/widgetu (cisto premjestanje),
`IngestApp` posuduje view umjesto kopije u svakom frameu; sync artefakata s 3,2 s na 0,26 s
(jedna dijeljena veza za citanje); filmstrip: `-threads 1` po ulazu (`qnc-ffmpeg-decode`).
Nije izvedeno: izbor citaca indeksa po pattern-id u `qnc-scanner`; generiranje postera koji kartica nema;
pracenje uvoza u shellu; test uvoza s pravom karticom kroz sucelje. Odobrenja su zatvorena.

Otvoreno ograniceno odobrenje 2026-09-20 (dvadeset cetvrto): korisnik je izricito odkljucao
`crates/qnc-editorial-application` i `crates/qnc-editorial-desktop` (Media Assist) samo za sličice klipova po
v5 postupku: adresa postera iz javnog prikaza baze (`qnc-content-read`), ucitavanje u pozadini kroz
`qnc-media-thumbnail` (odabrani klip prvi, ostali redom; poster iz projekta ili s kartice), a kartica klipa
crta sliku umjesto "..." kad slika postoji. Popis klipova prikazuje samo uvezene. Izgled i raspored ostaju
isti; `qnc-media-card`, `qnc-ui-kit` i `contracts/ui` ostaju zakljucani. Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-21 (dvadeset peto): korisnik je izricito
otkljucao samo procesni popravak prikaza postera i source timeline artefakata iz aktivne
projektne baze u view model, bez citanja u formi i bez promjene layouta. Otkljucano:
`crates/qnc-ingest-application`, `crates/qnc-media-thumbnail`, `crates/qnc-ingest-catalog`,
`crates/qnc-ingest-artifacts`, `crates/qnc-timeline-assets` i testovi samo koliko je potrebno.
Izvedeno: `qnc-ingest-application` vise ne zahtijeva `selection_config` za ucitavanje postera
koji su vec u direktoriju projekta; projektni poster URI iz aktivne baze moze se ucitati kroz
`qnc-media-thumbnail` i upisati u `ClipView.thumb_image` dok forma ostaje pasivna. Verificirano:
`cargo test -p qnc-ingest-application`, `cargo test -p qnc-media-thumbnail`,
`cargo test -p qnc-timeline-assets -p qnc-ingest-artifacts`, `cargo check -p qnc-app`.
`cargo build -p qnc-app` nije mogao zamijeniti pokrenuti `qnc-app.exe` jer ga Windows drzi
zakljucanim. Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-21 (dvadeset sesto): korisnik je izricito
otkljucao popravak filmstrip/wave artefaktnog procesa: worker ne smije biti hranjen
Ingest cacheom ni Ingest adapterom, nego aktivnom projektnom bazom i projektnim
postavkama. Otkljucano: novi javni neutralni moduli `crates/qnc-content-store` i
`crates/qnc-content-artifacts`, ugovori `contracts/modules/content-store.module.json` i
`contracts/modules/content-artifacts.module.json`, root `Cargo.toml`/`Cargo.lock`,
uklanjanje starog `crates/qnc-ingest-artifacts` i njegovog ugovora, te minimalne veze u
`crates/qnc-ingest-application` i `crates/qnc-ingest-preview-source`. Izvedeno:
`qnc-content-store` je neutralni owner project content DB transporta; `qnc-content-artifacts`
je javni adapter za citanje/pisanje timeline artefakata iz aktivnog project content DB-a;
filmstrip/wave workeri i dalje vide samo svoje javne read/write traitove; `qnc-content-artifacts`
sam cita host source bindinge iz javne transport konfiguracije. Ingest samo poziva sync za
aktivni projekt nakon upisa/importa i ne predaje selection config, cache ni listu klipova.
`tools/qnc-ingest-worker` je prosiren tako da nakon import reda sam pokrene artifact sync
za aktivni projekt iz `public_app_settings.active_project_id` -> `public_project_settings` ->
`qnc-work-settings` -> `qnc-content-store`; worker se moze pokrenuti samostalno i nastaviti
filmstrip/wave generiranje nakon sto su Ingest ili cijeli `qnc-app` ugaseni. Ingest smije
pokrenuti/wakeati worker, ali worker ne smije ovisiti o Ingest objektima, UI cacheu,
selection configu ni trenutno otvorenoj formi.
`focus`/preview put vise ne konfigurira niti pokrece generatore, nego samo cita vec objavljene artefakte za
odabrani klip. Bez source bindinga nema generiranja. Forma ostaje pasivna. Verificirano:
`cargo test -p qnc-content-store -p qnc-content-artifacts -p qnc-timeline-artifacts -p
qnc-filmstrip-worker`, `cargo test -p qnc-ingest-application -p qnc-ingest-preview-source`,
`cargo test -p qnc-conformance`, `cargo run -p qnc-conformance`, `cargo build -p qnc-app`;
aktivni artifact/worker put vise nema runtime dependency na
`qnc-ingest-artifacts` ni `qnc-ingest-store`. Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-22 (dvadeset sedmo): korisnik je trazio
da Broadcast Player bude vlasnik vremena. Otkljucano samo
`crates/qnc-player-client/src/lib.rs` za javni read-only helper
`View::confirmed_source_frame()`, koji vraca frame iskljucivo iz zadnjeg
`CarrierPositionChanged` eventa s potvrdjenim rangeom i timebaseom. `PlaybackReadinessChanged`
frame nije timeline/playhead vrijeme. Izveden je samo test koji zakljucava ovo
pravilo. Timeline, source preview, forme, Project, Ingest workflow i layout nisu mijenjani.

Zatvoreno ograniceno odobrenje 2026-09-19 (devetnaesto): korisnik je izricito
odobrio izvrsitelja uvoza u obitelji Ingest, OS-neutralno i za lokalno, LAN i
intranet. Otkljucano: novi crate `crates/qnc-ingest-import-worker` (javni modul:
preuzima klip iz reda u bazi sadrzaja `claim_next`, radi po postavkama projekta
`storage.ingest_media` i `playback.input`: link, kopija proxyja ili originala;
izvorni medij cita kroz `qnc-media-stream` (lokalno ili udaljeno), kopira u
komadima kroz privremenu datoteku i preimenovanje u direktorij projekta, upisuje
ishod `finish_import`); ugovor modula `contracts/modules/ingest-import-worker.module.json`;
root `Cargo.toml`/`Cargo.lock`; `crates/qnc-ingest-application` (samo radnja
`ingest_import_selected`: stavlja odabrane u red i pokrece izvrsitelja) i po
potrebi `crates/qnc-ingest-store` (samo ako je nuzno); dokumenti. Ne dira se Ingest UI.
Izvan opsega: generiranje proxyja (transcode), kopija postera (traži prosirenje
`FinishImport` o `thumbnail_uri`, zasebno odobrenje), mrezni upis medija u projekt bez
lokalnog pristupa direktoriju projekta. Sve ostalo ostaje zamrznuto.
Izvedeno: `qnc-ingest-import-worker` (plan po postavkama, kopija u komadima kroz
`.partial`, `ImportSession` u pozadini, `TransportQueue`); `qnc-ingest-store`:
`ContentWriteTransport` dobiva `queue_selected`, `claim_next`, `finish_import` i
`ContentWriteData::Claimed` (izvrsitelj pise samo kroz serijalizirani write transport,
kako trazi conformance), `qnc-ingest-select` prilagodjen novoj varijanti;
`qnc-ingest-application`: radnja `ingest_import_selected` stavlja odabrane u red i
pokrece uvoz (widgeti se ne diraju). Testovi: import-worker 10, application 27, select
16, store 26; conformance prolazi. Nije izvedeno: generiranje proxyja, kopija postera
(traži `thumbnail_uri` u `FinishImport`), test uvoza kroz cijelu aplikaciju i na
stvarnoj kartici, nastavak prekinutog uvoza za klip u stanju `Processing`.
Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-19 (osamnaesto): korisnik je izricito
odobrio pravilo probea po zapisu: klip ciji zapis ima zapisane probe podatke
(dimenzije, frame rate, tocan broj frameova, odnosno audio format) nikad se ne
probe-a; klip ciji zapis nema te podatke (npr. Sony bez sidecara) probe-a se jednom.
Odluka vise nije svojstvo cijele kamere nego zapisa: `CameraAdapter::sufficiency`
prima metapodatke klipa. Otkljucano: `crates/qnc-camera-adapter` (sucelje i
pomocna funkcija `has_probe_facts`), `crates/qnc-camera-sony-fx6-v6`,
`crates/qnc-ingest-select` (odluka po zapisu, testovi), dokumenti i zapis odobrenja.
Genericki put za kamere bez indeksa (grupiranje bez indeksa i probe prije snimke
kamere) nije dio ovog odobrenja: trazi izmjenu `qnc-scanner`, `qnc-source-groups` i
ugovora `qnc-media-record-db` (akvizicija trazi snimku kamere). Sve ostalo ostaje
zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-19 (sedamnaesto): korisnik je izricito
odobrio (a) pravilo probea: klip ciji zapis kartice (npr. Sony XML za original i proxy)
daje metapodatke nikad se ne probe-a, a klip bez takvog zapisa probe-a se jednom u
Ingestu; (b) kameru kao samostalnu komponentu po uzorku kataloga (kamera i vrsta
zapisa), ne po proizvodacu. Otkljucano: novi crateovi `crates/qnc-camera-adapter`
(sucelje `CameraAdapter`, `CameraRegistry`, `MetadataSufficiency`) i
`crates/qnc-camera-sony-fx6-v6` (uzorak `sony-fx6-v6`, ovija postojeci
`qnc-sony-metadata` bez izmjene); `crates/qnc-ingest-select` (popis adaptera
dolazi izvana umjesto ugradjenog `adapters()`, probe samo za adapter s
`NeedsProbe`, kamera s `Declared` postaje Final bez probea, testovi);
`crates/qnc-ingest-application` (samo composition root: sastavlja registar i
predaje ga Selectu) i po potrebi `crates/qnc-ingest-desktop`,
`apps/qnc-ingest`; ugovori modula i dokumenti; root `Cargo.toml`/`Cargo.lock`.
Odobreno je i za `qnc-media-records`, `qnc-media-metadata`,
`qnc-media-metadata-compose`, ali se diraju samo ako se pokaze da je nuzno.
Ingest UI se ne mijenja. Sve ostalo ostaje zamrznuto.
Izvedeno: `qnc-camera-adapter`, `qnc-camera-sony-fx6-v6`; `qnc-ingest-select` prima
registar i za adapter s `Declared` upisuje Final bez probea (14 testova prolazi,
uz nove: Declared se ne probe-a, NeedsProbe se probe-a jednom, prazan registar je
kontrolirana greska); `qnc-ingest-application` sastavlja registar
(`default_camera_registry`); 27 testova prolazi; conformance prolazi.
`qnc-media-records`, `qnc-media-metadata`, `qnc-media-metadata-compose` nisu mijenjani:
pokusaj da Final bez probea bude Complete lomi ugovorni test
`qnc-media-record-db` (Final s nedostajucim poljima je Partial) i vracen je.
Korisnik je zatim izricito odobrio i `crates/qnc-ingest-store`: `content/mod.rs`
`ready()` sada dopusta uvoz kad je zapis Final i (Complete ili bez ffprobe dokaza);
probani, a nepotpun zapis ostaje blokiran, Camera faza ostaje blokirana (3 nova testa,
26 testova store prolazi; nastavak end-to-end test u select: Declared klip se odabire,
stavlja u red i preuzima bez probea, 15 testova select). Conformance prolazi.
Nije izvedeno (traze zasebna odobrenja): odabir citaca po `pattern_id` u `qnc-scanner`,
genericki adapter za kamere bez indeksa.

Povuceno ograniceno odobrenje 2026-09-19 (petnaesto): predlozeni prvi korak kartice
klipa (crate `qnc-clip-status` i izmjene `qnc-content-read`, `qnc-editorial-*`,
`editorial.layout.json`) nije izvrsen. Nakon audita (docs/v5-book/08) opseg se
prepisuje u novo odobrenje uz pravilo da je UI zakljucan i da se smije samo
dodavati. Nista nije otkljucano, nista nije promijenjeno.

Zatvoreno ograniceno odobrenje 2026-09-19 (sesnaesto): korisnik je izricito
odobrio izmjenu pravila provjere "business app DB-only isolation": dev-ovisnosti
(`[dev-dependencies]`) tranzitivnih crateova vise se ne broje, jer se ne povezuju
u aplikaciju. I dalje se broje sve runtime ovisnosti tranzitivnih crateova te
dev-ovisnosti samog provjeravanog cratea. Otkljucano samo
`tools/qnc-conformance/src/main.rs` (`cargo_dependency_names`,
`collect_transitive_cargo_dependencies` i tri nova testa). Verificirano: testovi
conformancea (20), pun conformance prolazi bez greske. Odobrenje zatvoreno. Sve
ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-19 (cetrnaesto): korisnik je izricito
zatrazio da preview monitor i source timeline u aplikacijama e, g, l, o budu
stvarno spojeni na Broadcast Player, uz popis klipova ispod monitora (lijevo
tijelo, mjesto izbora direktorija). Novi crateovi: `crates/qnc-editorial-application`
(tanki composition root: citanje aktivnog projekta, popis klipova iz projektne baze
samo za citanje, player klijent, timeline artefakti samo za citanje) i
`crates/qnc-source-bindings` (javni modul: veze `qnc://local/source/...` na lokalne
korijene iz host konfiguracije; ugovor `contracts/modules/source-bindings.module.json`).
Izmjene: `crates/qnc-editorial-desktop` (forma prikazuje popis, koristi view iz
application cratea), `contracts/ui/editorial.layout.json` (popis klipova),
`tools/qnc-conformance` (granica forme), root `Cargo.toml`/`Cargo.lock`. Ingest se
ne dira. Bez skeniranja, probea i pisanja u tude baze. Verificirano nakon sesnaestog
odobrenja: pun conformance prolazi. Zatvoreno; grana `wip/editorial-player-monitor`
ceka spajanje. Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-19 (trinaesto): korisnik je izricito
otkljucao Ingest za popravak nalaza 2 iz auditâ: playback guard blokira odabir
klipa tijekom pripreme ili reprodukcije playera. Otkljucano samo
`crates/qnc-ingest-application/src/lib.rs` (`select_clips` i test) i
`crates/qnc-ingest-application/src/playback_guard.rs` (`blocks_action`). Promjena:
akcije odabira (`ingest_clip_toggle`, `ingest_select_all`, `ingest_clear_selection`)
vise ne blokira guard; ostaje blokirano ponovno citanje, promjena izvora,
direktoriji i generiranje postera. Guard se ne siri, nego sužava. Verificirano:
testovi (27, novi `clip_selection_is_not_blocked_by_the_playback_guard` pada na
starom ponasanju), conformance, gradnja `qnc-ingest`. `qnc-app` (shell) nije
ponovno izgradjen jer je njegov exe bio zauzet pokrenutim procesom; treba ga
izgraditi nakon zatvaranja shella. Odobrenje
zatvoreno, Ingest je ponovno zamrznut. Sve ostalo
ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-18 (dvanaesto): korisnik je trazio da se
aplikacije grupa e, g, l, o dobiju u shell desktopu. Za svaku (Media Assist Audio
AI, Media Assist Audio, Media Assist Video, Story): novi app crate
`apps/qnc-media-assist-audio-ai`, `apps/qnc-media-assist-audio`,
`apps/qnc-media-assist-video`, `apps/qnc-story` s `qnc-app.json` i samostalnim exe-om,
javni desktop adapter crate (`crates/*-desktop-adapter`), ugovor aplikacije u
`contracts/applications/` (tri nova; Story vec postoji) i deklarativni ugovor baze u
`contracts/databases/` (tri nova, bez sheme: tablice se definiraju kasnije), dopuna
`crates/qnc-editorial-desktop` (`EditorialApp`), u shellu samo Cargo ovisnost na te
adaptere i njihovi unosi u tablicu tvornica (`apps/qnc-app/Cargo.toml`,
`apps/qnc-app/src/main.rs`), root `Cargo.toml`/`Cargo.lock`, i eventualne
prilagodbe `tools/qnc-conformance` samo ako pravila trebaju znati za nove
aplikacije. Ingest i Project se ne diraju. Verificirano: cijeli workspace se
kompajlira, conformance prolazi bez izmjene pravila, testovi shella (12), svaka
samostalna aplikacija prolazi `--check-contracts`, katalog osvjezen (a, b, e, g, l, o),
smoke shella. Ugovori baze za e, g, l su deklarativni (tablice prazne, politika
`owner_only`). Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (jedanaesto): korisnik je odredio da se
Ingest layout i UI kopiraju 100% vjerno kao osnova za Media Assist i Story, uz
izostavljanje desnog prikaza klipova; mjesto izbora direktorija ostaje prazno
(kasnije izbor klipova). Novi crate forme `crates/qnc-editorial-desktop`
(kopija koda Ingest forme: shell, preview monitor, glava pool-a, dock s
timelineom; bez ovisnosti o Ingestu i Projectu), dopuna `contracts/ui/editorial.layout.json`
(`pool_head`, `clip_label_fallback`), jedna dodana provjera granice u
`tools/qnc-conformance/src/main.rs`, root `Cargo.toml`/`Cargo.lock` samo za taj
crate. Ingest se ne dira. Tipkovnicki prečaci nisu dio ovog koraka. Verificirano:
kompajlira se bez upozorenja, conformance (nova provjera `Editorial form boundary`),
smoke za grupe e, g, l, o. Odobrenje zatvoreno, obitelj je zamrznuta. Sve ostalo
ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-18 (deseto): korisnik je trazio izvlacenje
tocnih layouta iz qnc_v4. Samo novi dokument `docs/88-qnc-v4-layout-extract.md`
(tocne mjere, boje i razlike prema novom QNC-u). Nema izmjena koda ni ugovora.
Odobrenje zatvoreno, obitelj je zamrznuta.
Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-18 (deveto): korisnik je ocijenio da probni
prozor nije kompletan UI (nedostaje donji dock). Novi javni pasivni modul
`crates/qnc-source-dock` (header s nazivom klipa, IN/OUT/Trajanje i gumbima,
mjesto za timeline; iz v4 `qnc_source_dock.rs`, samo varijanta s akcijama
uredjivanja, ne ingest varijanta), ugovor `contracts/modules/source-dock.module.json`,
dopuna bloka `source_dock` u `contracts/ui/editorial.layout.json`, proširenje
probnog prozora, root `Cargo.toml`/`Cargo.lock` samo za taj crate. Ingest se ne
dira; referenca za raspored docka je Ingest layout (odluka korisnika). Verificirano:
test modula, conformance (usporedba mjera docka s Ingest ugovorom), smoke probnog
prozora za grupe e i o. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (osmo): korisnik je potvrdio nastavak.
Samo probni prozor (`example`) u `crates/qnc-editorial-shell/examples/` s
dev-ovisnostima na `qnc-media-pool-head`, `qnc-media-card` i `serde_json`, da se
tri nova modula vide nacrtana s lazniim podacima prema `editorial.layout.json`.
Nema aplikacije, baze ni manifesta (aplikacije e, g, l traze prvo definiranu
bazu, §13). Verificirano: kompajlira se bez upozorenja, pokrece se za grupe e i o
(Responding, bez stderr-a). Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (sedmo): korisnik je potvrdio korak 3
(uz odluku "ne Ingest"). Novi javni pasivni moduli iz v4 reference, bez diranja
Ingesta: `crates/qnc-editorial-shell`, `crates/qnc-media-pool-head`,
`crates/qnc-media-card`, s ugovorima u `contracts/modules/`. Otkljucano: ti novi
crateovi, novi `*.module.json`, root `Cargo.toml`/`Cargo.lock` samo za njih.
Moduli su pasivni: boje i mjere dobivaju od forme (iz ugovora), ne drze stanje,
ne citaju bazu, ne rade probe/scan. Verificirano: testovi modula (shell 4, glava
pool-a 4, kartica 7), cijeli workspace se kompajlira, conformance prolazi. Moduli
nisu spojeni ni u jednu formu. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (sesto): korisnik je potvrdio da je
layout isti kao u Ingestu, a razlikuju se samo komponente u slotovima. Korak 2:
novi ugovor `contracts/ui/editorial.layout.json` (zajednicka geometrija +
kompozicija po grupama e, g, l, o) i jedna dodana provjera u
`tools/qnc-conformance/src/main.rs`. Ingest i `ingest.layout.json` se ne dira.
Desni panel za e, g, l ostaje prazan. Verificirano: conformance prolazi, a provjera
hvata odstupanje geometrije od Ingest ugovora i neprazan desni panel (mutacijski
test). Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-22 (dvadeset osmo): korisnik je
izricito trazio da se IN/OUT oznake vide na source timelineu i da nema
hardkodiranog znacenja gumba iz forme. Otkljucano samo:
`crates/qnc-timeline`, `crates/qnc-ingest-application`,
`crates/qnc-editorial-application`, `crates/qnc-ingest-desktop`,
`crates/qnc-editorial-desktop`, `contracts/ui/ingest.layout.json`,
`contracts/ui/editorial.layout.json`, `contracts/qnc-keyboard-shortcuts.json`.
Source timeline ostaje jedina javna timeline komponenta; `source-preview` ne
postaje vlasnik IN/OUT. Markeri se postavljaju iz potvrdenog Broadcast Player
framea, cuvaju se kroz refresh projekcije za isti klip i crtaju se u
`qnc-timeline`. Desktop forme ne tumace labelu `[`/`]`; layout nosi action_id,
a tipkovnica ide kroz `qnc-keyboard-shortcuts.json`. Verificirano ciljanim
testovima i conformanceom. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-22 (dvadeset deveto): korisnik je
izricito trazio kreiranje virtualnog short klipa. Otkljucano: novi
`crates/qnc-virtual-shots`, `contracts/modules/virtual-shots.module.json`,
`contracts/databases/virtual-shots.database.json`,
`contracts/databases/story.database.json` (tablica `virtual_shots` vise nije
u Story ugovoru), `crates/qnc-content-read` (putanja vec razrijesene projektne
baze), `crates/qnc-editorial-application`, `crates/qnc-editorial-desktop`,
`crates/qnc-keyboard-shortcut` (Enter), `tools/qnc-conformance`,
root `Cargo.toml`. Add virtual clip cita IN/OUT koje source timeline vec crta
i upisuje jedan red klase `short`. Naslovnice, source root, Talking Head,
Voice over i B-roll nisu dio ovog koraka. Layout ugovori nisu dirani.
Verificirano: `cargo test -p qnc-virtual-shots -p qnc-editorial-application`
i `cargo run -p qnc-conformance`. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-22 (trideseto): korisnik je trazio
da tab Virtual prikazuje short virtualne klipove po v5. Otkljucano:
`crates/qnc-virtual-shots` (`list_shorts` iz `public_short_clips`),
`crates/qnc-editorial-application`, `crates/qnc-editorial-desktop`.
All ostaje popis uvezenih klipova. Virtual crta samo shortove, najstariji
prvi, naziv i trajanje `sekunde:frameovi`. Nakon Add virtual clip tab
prelazi na Virtual. Klik na short otvara maticni klip i, kad player potvrdi
trajanje, postavlja IN/OUT tog shorta. Vlastita poster slika (`cover.jpg`)
nije dio ovog koraka. Layout ugovori nisu dirani.
Verificirano: `cargo test -p qnc-virtual-shots -p qnc-editorial-application`
i `cargo check -p qnc-editorial-desktop`. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23: korisnik je trazio ispravak gresaka
iz vanjskog audita oko DB vlasnistva i javnih komponenti. Otkljucano:
`crates/qnc-content-store`, `crates/qnc-ingest-store/src/content/**`,
`crates/qnc-virtual-shots`, `crates/qnc-editorial-application`,
`crates/qnc-application-sequence`, `crates/qnc-ingest-select` i pripadajuce
`Cargo.toml`/`Cargo.lock` promjene. Odluka: jedini owner project content baze je
`qnc-content-store`.
`qnc-ingest-store::content` smije ostati samo kompatibilni re-export i ne smije
ponovno dobiti SQL implementaciju. `qnc-virtual-shots` ne smije otvarati
projektnu bazu u write modu niti imati vlastiti SQLite schema/write path; save
shorta i status IN/OUT slicica idu kroz `ContentWriteTransport`. Slijed
aplikacija cita `workspace_db_uri` iz aktivnih projektnih postavki i resolver
binding, bez hardkodiranja naziva projektne DB datoteke u komponenti. Select
publish put smije prihvatiti samo svoje ocekivane write odgovore; novi
`SavedShort` odgovor iz javnog transporta tretira kao neispravan odgovor u tom
workflowu, ne kao Ingest logiku.
Verificirano: `cargo test -p qnc-editorial-application -p qnc-virtual-shots -p
qnc-content-store -p qnc-ingest-store -p qnc-application-sequence`, fokusirani
`cargo check` istih crateova, `cargo build -p qnc-app` i `cargo run -p
qnc-conformance`. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K1 forma-lego audit): korisnik je
dostavio audit i odobrio nastavak prvog koraka: javni read-only port aktivnog
projekta. Otkljucano samo: novi `crates/qnc-active-project-read`, novi
`contracts/modules/active-project-read.module.json` i root `Cargo.toml` /
`Cargo.lock`. Modul omata postojece `qnc-work-settings` i `qnc-content-read`,
svaki `read`/`compare` ponovno cita aktivni projekt iz baze, ne drzi projektno
stanje kao istinu, ne pise u bazu, ne otvara medij, ne radi probe i ne ovisi o
Project/Ingest/Editorial/shell/form crateovima. API daje snapshot aktivnog
projekta, `CatalogSignature` i usporedbu prikazanog stanja
(`Same`/`ProjectChanged`/`SignatureChanged`/`NoActiveProject`). Nije izvedeno:
K2 spajanje potrosaca na port, promjena formi, UI/layout promjene, ni
semanticko preimenovanje fizickih DB datoteka. Verificirano: `cargo test -p
qnc-active-project-read -p qnc-work-settings -p qnc-content-read`, `cargo build
-p qnc-app` i `cargo run -p qnc-conformance`. Odobrenje je zatvoreno, obitelj je
zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K2 forma-lego audit): korisnik je
odobrio nastavak nakon K1. Otkljucano samo za spajanje potrosaca na javni
read-only port aktivnog projekta: `crates/qnc-editorial-application`,
`crates/qnc-ingest-application`, `crates/qnc-ingest-catalog`,
`crates/qnc-ingest-import-worker`, `tools/qnc-ingest-worker`,
`tools/qnc-dev-diagnostics-app` i pripadajuci `Cargo.toml`/`Cargo.lock`.
Izvedeno: MA/Story katalog koristi `qnc-active-project-read::compare()` za
`Same`/`ProjectChanged`/`SignatureChanged`/`NoActiveProject`; Ingest application
i `qnc-ingest-catalog` citaju aktivni projekt kroz `ActiveProjectReader`, a
`SettingsReader` ostaje samo transport/binding objekt za postojece javne
module; import worker, ingest-worker i diagnostics citaju aktivni projekt kroz
isti javni port. Nije izvedeno: promjena UI/formi, K3 action/tab model, K4
timeline ulaz, preimenovanje fizickih DB datoteka ni promjena Projecta.
Verificirano: `cargo test -p qnc-active-project-read -p qnc-ingest-catalog -p
qnc-editorial-application -p qnc-ingest-application -p qnc-ingest-import-worker
-p qnc-ingest-worker`, `cargo check -p qnc-dev-diagnostics-app`, `cargo run -p
qnc-conformance` i `cargo build -p qnc-app`. Odobrenje je zatvoreno, obitelj je
zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K3 forma-lego audit): korisnik je
odobrio nastavak nakon K2. Otkljucano samo za uklanjanje tekstualnih odluka iz
Ingest i Editorial formi: `contracts/ui/ingest.layout.json`,
`contracts/ui/editorial.layout.json`, `crates/qnc-ingest-application`,
`crates/qnc-ingest-desktop`, `crates/qnc-editorial-application` i
`crates/qnc-editorial-desktop`. Izvedeno: layout ugovori nose `action_id` uz
postojeci tekst gumba/tabova gdje forma prije toga odlucuje po tekstu; Ingest
enabled stanje dolazi iz `IngestViewModel::action_enabled`; Editorial
enabled/selected stanje dolazi iz `EditorialView`; Ingest i Editorial desktop
keyboard dispatch vise ne drze privatne popise dopustenih akcija nego salju
akcije iz javnog QNC keyboard kataloga. Izgled nije namjerno mijenjan. Nije
izvedeno: K4 gotov source timeline ulaz, K5 prelazak Ingest kartica na
`qnc-media-card`, promjena source previewa, promjena DB sheme ili Projecta.
Verificirano: `cargo test -p qnc-ingest-application -p
qnc-editorial-application -p qnc-ingest-desktop -p qnc-editorial-desktop`,
`cargo run -p qnc-conformance` i `cargo build -p qnc-app`. Odobrenje je
zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K4 forma-lego audit): korisnik je
odobrio nastavak nakon K3. Otkljucano samo za to da javna source timeline dock
komponenta primi gotov timeline/artifact ulaz, bez promjene izgleda:
`crates/qnc-source-dock`, `crates/qnc-ingest-desktop`,
`crates/qnc-editorial-desktop`, `crates/qnc-ingest-application` i pripadajuci
`Cargo.toml`/`Cargo.lock`. Izvedeno: `qnc-source-dock::SourceTimeline` dobio je
`from_assets(&TimelineProjection, TimelineTheme, &SourceTimelineAssets)`; Ingest
i Editorial forme vise ne slazu filmstrip i cetiri wave trake same, nego
predaju projection/assets javnoj komponenti; stari Ingest helperi za pojedine
timeline laneove su uklonjeni da ne ostane paralelni put. Nije izvedeno:
promjena timeline izgleda, DB sheme, source previewa, playera, Projecta, K5
media-card rez ili bilo kakav novi artifact worker. Verificirano: `cargo test
-p qnc-source-dock -p qnc-ingest-application -p qnc-ingest-desktop -p
qnc-editorial-application -p qnc-editorial-desktop`, pretraga koja pokazuje da
`filmstrip_background()`/`a1_peaks()`... spajanje ostaje samo u
`qnc-source-dock`, `cargo run -p qnc-conformance` i `cargo build -p qnc-app`.
Odobrenje je zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K5 forma-lego audit): korisnik je
odobrio nastavak nakon K4. Otkljucano samo za prelazak Ingest clip grida na
javnu univerzalnu komponentu `qnc-media-card`, uz ocuvanje postojeceg izgleda i
bez izmjena DB-a, Projecta, previewa, timelinea ili ingest workflowa:
`crates/qnc-media-card`, `crates/qnc-ingest-desktop` i pripadajuci
`Cargo.toml`/`Cargo.lock`. Izvedeno: `qnc-media-card` je aditivno prosiren
opcionalnim top-right markerom i overlay labelom; MA/Story eksplicitno ostaju bez
tih opcija; Ingest `clip_grid` je tanki adapter `ClipView -> CardRow`, koristi
javni `show_card_grid`, javni checkbox hit-test i javnu geometriju; privatni
Ingest `clip_card` i `selection_check` aktivni put uklonjeni su. Nije izvedeno:
promjena layout dimenzija, promjena semantike odabira/preview fokusa, K6 redovi
kartica iz application sloja, live UI test ili commit. Verificirano: `cargo test
-p qnc-media-card -p qnc-ingest-desktop -p qnc-editorial-desktop`, pretraga koja
ne nalazi privatni Ingest card/selection painter, `cargo run -p qnc-conformance`
i `cargo build -p qnc-app`. Odobrenje je zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (K6/N8 forma-lego audit): korisnik je
odobrio nastavak nakon K5. Otkljucano samo za ukidanje drugog parsera
`ingest-transport.json`: `crates/qnc-source-bindings`,
`crates/qnc-ingest-select` i pripadajuci `Cargo.toml`/`Cargo.lock`. Izvedeno:
`qnc-source-bindings` je jedini runtime parser host transport konfiguracije
(`catalog`, `source_index`, `media_records`, registrirani izvori, probe i
parallelism); `qnc-ingest-select::SelectionConfig::load` vise ne cita JSON
datoteku, nego konvertira javni model u svoj workflow model i provodi Select
validaciju. Nepotpuni `probe` podaci se odbijaju, ne dopunjuju se defaultima.
Nije izvedeno: promjena formi, DB sheme, Projecta, previewa, timelinea,
workera, scanner/probe logike ili UI izgleda. Verificirano: `cargo test -p
qnc-source-bindings -p qnc-ingest-select -p qnc-ingest-application -p
qnc-ingest-import-worker`, pretraga koja pokazuje da runtime reference na
`ingest-transport.json` ostaju u `qnc-source-bindings` (test fixture reference
ostaju u testovima), `cargo run -p qnc-conformance` i `cargo build -p qnc-app`.
Odobrenje je zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-22 (trideset prvo): korisnik je
trazio novu ideju za slicice virtualnog shorta: MARK_IN uhvati potvrdenu
sliku u memoriju, MARK_OUT uhvati potvrdenu sliku u memoriju, a nakon upisa
virtualnog shorta u bazu te se dvije slike spremaju u projektni
`virtual_shorts/<shot_id>/in.jpg` i `out.jpg`. Memorija nije izvor istine:
DB red nastaje prvi, a slike su artefakti koji se zatim objave u istom DB
owneru. Otkljucano: novi javni modul `crates/qnc-virtual-short-stills`
(`contracts/modules/virtual-short-stills.module.json`), `crates/qnc-virtual-shots`
(`still_status`, `in_still_uri`, `out_still_uri`, `still_error` i publish API),
`contracts/databases/virtual-shots.database.json`,
`contracts/modules/virtual-shots.module.json`, `crates/qnc-editorial-application`
kao composition root i root `Cargo.toml`/`Cargo.lock`. Forma, source preview,
timeline, Ingest i Project se ne diraju. Background retry worker nije dio ovog
reza; status `pending/failed` ga samo omogucuje u kasnijem otkljucavanju.

Zatvoreno ograniceno odobrenje 2026-09-22 (trideset drugo): korisnik je
trazio da se IN slicica koristi kao poster za short virtual kadar, ali kao
javna komponenta, ne kao privatno sirenje forme/aplikacije. Otkljucano:
novi javni modul `crates/qnc-virtual-short-cards`
(`contracts/modules/virtual-short-cards.module.json`), `Cargo.toml`/`Cargo.lock`,
te tanko spajanje u `crates/qnc-editorial-application` i
`crates/qnc-editorial-desktop`. Pravilo "IN still je poster virtual short
kartice" zivi u `qnc-virtual-short-cards`; desktop forma samo crta dobiveni
card row, a application root samo predaje DB redove i ucitane slike.

Zatvoreno ograniceno odobrenje 2026-09-18 (peto): korisnik je izricito trazio
pocetak UI sheme za Media Assist grupe (e, g, l) i Story (o). Korak 1 je samo
novi dokument `docs/87-editorial-ui-reference-audit.md` (snimka v4 rasporeda i
mapa komponenti). Nema izmjena koda. Odluke korisnika: desni panel ostaje prazan
(prostor za funkcije pojedine grupe) kao u v4 Media Assistu; Ingest se NE dira
(nove komponente grade se izravno iz v4 reference, ne izdvajaju iz Ingest forme).
Sve ostalo ostaje zamrznuto.

Zatvoreno ograniceno odobrenje 2026-09-23 (products/work-settings lokacije):
korisnik je trazio da javne komponente ostanu neutralne i da lokacije dolaze iz
aktivnog projekta/projektne baze, ne iz forme ni iz hardkodiranog Ingest puta.
Otkljucano samo: `seed/system_seed.json`, `crates/qnc-work-settings/**`,
`crates/qnc-project-store/src/lib.rs`, `crates/qnc-ingest-work-plan/**`,
`crates/qnc-source-preview/**`, `crates/qnc-content-artifacts/**`,
`crates/qnc-media-thumbnail/**`, `crates/qnc-virtual-short-stills/**`,
`crates/qnc-editorial-application/**`, `crates/qnc-content-store/src/tests.rs`
i pripadni ugovorni tekstovi. Izvedeno: radne postavke imaju javni `products`
blok (`products/thumbnails`, `products/filmstrip`, `products/virtual_shorts`,
`products/virtual_segments`, `products/b_roll_virtual_clips`); novi projekt
kreira te direktorije; filmstrip, thumbnail i virtual-short still potrosaci vise
ne smiju sami pretpostavljati root `filmstrip`, root `virtual_shorts` ni
`ingest/thumbnails`. Forma/layout nisu dirani. Nije izvedeno: migracija starih
projekata, promjena UI-ja.

Zatvoreno ograniceno odobrenje 2026-09-23 (DB datoteke aktivnog projekta):
korisnik je odredio novu semantiku baza. Globalna baza projekata je
`data/qnc-projects.db`; lokalna baza pojedinog projekta je `project.db` u
direktoriju projekta. Globalno nosi `qnc` prefiks, lokalno ga ne nosi. Put
citanja ostaje baza -> `active_project_id` -> `project_uri`/binding ->
`project.db` -> `public_project_settings.settings_json` -> `WorkSettings`.
Otkljucano samo: ugovori baza, `qnc-project-store`, `qnc-project-close`,
`qnc-work-settings`, `qnc-active-project-read`, `qnc-player-input` testni
fixture, app/alati/testovi koji su imali stare nazive. Izvedeno: stari nazivi
globalne i projektne baze uklonjeni su iz koda, ugovora i alata. Nije izvedeno:
migracija starih projekata, promjena UI-ja, promjena fizickog modela
`project.db`.

Zatvoreno zakljucavanje 2026-09-23 (Project/Ingest/Player/Monitor/Worker):
korisnik je nakon live provjere trazio da se zakljucaju Project, Ingest,
Broadcast Player, preview monitor i ingest-worker. Zamrznuto je bez daljnjeg
izricitog otkljucavanja: `crates/qnc-project-*`, `crates/qnc-project-store`,
`crates/qnc-project-close`, `apps/qnc-app` samo u dijelu Project hostanja,
`crates/qnc-ingest-*`, `tools/qnc-ingest-worker`, `crates/qnc-broadcast-player`,
`crates/qnc-broadcast-engine`, `tools/qnc-player-runner`, `crates/qnc-player-*`,
`crates/qnc-source-preview`, `crates/qnc-monitor`, `crates/qnc-source-dock` u
dijelu preview/source monitora, i povezani ugovori koji mijenjaju taj runtime.
Dopusteno bez otkljucavanja ostaje samo citanje, audit, pokretanje testova,
build i live pokretanje vec sastavljenih aplikacija/helpera, te zapis poslovnih
rezultata kroz postojece javne write putove. Nedostajuci runtime exe smije se
izgraditi istim profilom, ali to nije dozvola za promjenu koda. Sljedeca
izmjena bilo kojeg od tih dijelova mora prvo imenovati tocan modul/putanju i
razlog otkljucavanja.

Zatvoreno ograniceno odobrenje 2026-09-23 (Paket A ciljne slike): korisnik je
odobrio Paket A iz `docs/89-qnc-target-lego-catalog.md`: forma je samo ploca
javnih univerzalnih lego kockica. Otkljucano samo: `tools/qnc-conformance/**`,
novi `docs/89-qnc-target-lego-catalog.md` i ovaj zapis. Izvedeno: nova provjera
„target picture C1-C13 (forms are boards of public pieces)“ u
`tools/qnc-conformance/src/lego.rs` s poznatim iznimkama u
`tools/qnc-conformance/lego-baseline.json` (51 kljuc, stanje commita `9fd8b07`).
Veci broj ili novi kljuc je greska; manji broj je upozorenje da se baseline
spusti. Baseline se nikad ne podize. Trenutni brojevi:
`cargo run -p qnc-conformance -- --print-lego-counts`. Nije izvedeno: nijedan
popravak koda, ugovora, formi ni UI-ja; pakete B-H treba zasebno otkljucati po
cjelinama iz dokumenta. Ciljna slika ne ukida zakljucavanje Project/Ingest/
Player/Monitor/Worker iz prethodnog zapisa. Verificirano: `cargo test -p
qnc-conformance` (25), `cargo run -p qnc-conformance` prolazi, i mutacijska
provjera (spustena granica C10 obara conformance). Odobrenje je zatvoreno,
obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-23 (zastoj playera dok radi pozadina):
korisnik je live potvrdio da player stane nakon desetak sekundi dok pozadinski
proces radi i otkljucao cetiri popravka iz dijagnoze: worker samo jedan primjerak,
worker se sklanja dok bilo koji player radi, skrivena povrsina gasi svoj player,
player se oporavlja nakon ispraznjenog audio reda. Dokaz iz `player.log`:
`player-rebuffer reason=audio_underrun` pa `audio_rearm_failed ... initial PCM
queue is not ready`, player ostaje stajati; istovremeno dva `qnc-ingest-worker`
procesa rade iste filmstripove i dva playera citaju karticu. Otkljucano i izvedeno
samo: `crates/qnc-broadcast-player` (`finish_audio_rearm`, test),
`crates/qnc-broadcast-engine` (ponavlja commit/start dok audio red ceka PCM),
novi javni `crates/qnc-playback-activity` + `contracts/modules/playback-activity.module.json`
(neutralni zapis "player radi" i najam workera u projektnoj bazi; kod premjesten iz
`qnc-ingest-runtime`, koji ostaje samo re-export), `crates/qnc-source-preview`
(svaki preview javlja rad playera, C9), `crates/qnc-ingest-import-worker`
(`run_import` bez najma), `tools/qnc-ingest-worker` (jedan najam za uvoz i
artefakte; filmstrip/wave se pauziraju dok player radi, C8),
`crates/qnc-shell-desktop-api` (`on_deactivated`), `apps/qnc-app` (poziva ga pri
promjeni povrsine, test), `crates/qnc-ingest-desktop-adapter` i cetiri editorial
adaptera, `crates/qnc-ingest-desktop`/`qnc-editorial-desktop` (`set_active`
umjesto `on_activated`, bez rasta), `crates/qnc-ingest-application` (`set_active`,
uklonjen vlastiti reporter jer to radi preview) i `crates/qnc-editorial-application`
(`set_active`; dvije petlje cekanja upisa spojene u jednu radi C10), Cargo.
Baseline ciljne slike spusten: C8 x2 i C9 uklonjeni, C10 Ingest 1758, Editorial 757.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story korak 1 i 1b, v5 Story): korisnik je
odobrio Story plan (docs/93, inventar v5 Storyja) i rekao "odkljucaj sve sto trebas";
odluke: zapisi Storyja u bazi projekta, mijesani fps zabranjen jasnom greskom, Off bez
slike je crn. Otkljucano i izvedeno samo: `crates/qnc-content-store` (tablica
`program_segments` + `public_program_segments`, operacije CreateSegment, DeleteSegment,
MoveSegment, ListSegments; pravila R8-R13: vrsta ton/off, OUT >= IN+1, samo uvezeni klip,
jedan fps po prici, tvrdo brisanje uz zatvaranje rupe, zamjena sa susjedom; testovi),
`crates/qnc-ingest-select/src/publish.rs` (samo novi write odgovor tretiran kao
neispravan u Select toku), novi javni `crates/qnc-program-segments` (model programa,
upis bez blokiranja UI-ja, naredbe Select/Step/Move/Delete), novi javni
`crates/qnc-segment-panel` (v5 panel "Segmenti" nad `qnc-timeline` i lista taba Segment
s Up/Down/Del), novi javni `crates/qnc-editorial-layout` (model editorial layout
ugovora premjesten iz forme; grupa smije imati vlastite tabove i gumbe glave poola),
`crates/qnc-virtual-shots` (upis shorta i slicica s cekanjem premjesten iz
`qnc-editorial-application`), `crates/qnc-editorial-application` (Talking Head/Voice
over, tabovi All/Virtual/B-roll/Segment, tipke strelice gore/dolje i Delete, naredbe
segmenata), `crates/qnc-editorial-desktop` (desni panel Segmenti za grupu o, lista
taba Segment, prazan B-roll tab), `contracts/ui/editorial.layout.json` aditivno (grupa
o: action_id za Talking Head/Voice over, tabovi B-roll/Segment, gumbi B i Export
HI-res; poruka praznog B-roll taba), ugovori `program-segments`, `segment-panel`,
`editorial-layout` (moduli) i `program-segments` (baza), Cargo. Nije izvedeno: Wrap
pogled, M markeri i slotovi, pustanje price, pokrivalice (B, Cover slot, Overwrite,
Pokrivalice, sadrzaj B-roll taba), Sync/B-roll, Export HI-res (gumbi B i Export su
vidljivi, ali nisu omoguceni). C10 granice spustene: Editorial application 737, forma
735 (forma je prije bila 920). Verificirano: `cargo build --workspace`, testovi
`qnc-content-store` (32), `qnc-program-segments` (6), `qnc-segment-panel`,
`qnc-editorial-application` (11), `qnc-virtual-shots`, `qnc-editorial-layout`,
`cargo run -p qnc-conformance` bez upozorenja. Odobrenje je zatvoreno.
Zatvoreno ograniceno odobrenje 2026-09-24 (Story korak 2, v5 Story): isti Story plan i
"odkljucaj sve sto trebas"; odluke korisnika: M markeri pripadaju segmentu (sloj iznad
base videa), svaki segment ima prekidac 1 (markeri prate sliku, zadano) ili 2 (markeri
ostaju na frameu programa, kao v5); Talking Head i Voice over idu na A1, pokrivalice na
A2; cijela montaza je frame-precizna. Otkljucano i izvedeno samo: `crates/qnc-content-store`
(tablica `program_markers` + `public_program_markers`, stupac `marker_mode`, operacije
TrimSegment, SetSegmentMarkerMode, Create/Move/DeleteMarker, ListMarkers; brisanje
segmenta brise njegove markere i pomice markere prekidaca 2; testovi),
`crates/qnc-ingest-select/src/publish.rs` (samo preimenovani write odgovor `Created`),
`crates/qnc-program-segments` (markeri, M-M slotovi sa stalnim imenima, provjere
zakljucanih krajeva, navigacija marker/slot/segment/pocetak, Wrap: naredba samo trazi
klip i source frame od playera, playhead programa dolazi samo iz potvrdjenog framea
playera; mapiranje tipki kataloga), `crates/qnc-segment-panel` (redovi s markerima i
slotovima, prekidac 1|2, traka M marker/navigacija iz v5, pregledni red programa;
Cover slot, Overwrite i Sync/B-roll vidljivi, ali nisu omoguceni),
`crates/qnc-editorial-application` (Wrap otvaranje klipa i cue, tipke M, Delete, Home,
I/O skracuju segment samo u Wrap pogledu), `crates/qnc-editorial-desktop` (samo poziv
novog panela), ugovori `program-segments` (modul i baza) i `segment-panel`. Layout
ugovori nisu dirani; C10 granice nisu podignute. Nije izvedeno: pustanje price kroz
segmente, pokrivalice, Sync/B-roll, undo/redo, Export HI-res. Verificirano: testovi
`qnc-content-store` (35), `qnc-program-segments` (13), `qnc-editorial-application` (11),
`cargo build --workspace`, `cargo run -p qnc-conformance` prolazi. Odobrenje je zatvoreno.
Zatvoreno ograniceno odobrenje 2026-09-24 (Story korak 3a: markeri na osi programa):
korisnik je nakon analize v5 (`novi13`: slot 16-160 preko granice Off->Ton) odredio:
segmenti su dijelovi programske trake, programska traka nastaje slaganjem segmenata i
to je prica; zadani markeri postoje samo na pocetku prvog i na kraju zadnjeg segmenta;
rekao "kreni". Prekidac 1|2 i skracivanje segmenta (nisu v5 postupak) uklonjeni su.
Otkljucano i izvedeno samo: `crates/qnc-content-store` (`program_markers` nosi samo
`program_frame`; stari razvojni zapisi markera po segmentu i stupac `marker_mode` se
brisu pri otvaranju, bez migracije; marker samo strogo unutar programa i jedan po frameu;
brisanje segmenta brise markere unutar njega i pomice kasnije ulijevo, kao v5
`shift_markers_after_part_removal_frames`; premjestanje segmenta ne dira markere;
uklonjeni TrimSegment i SetSegmentMarkerMode; testovi), `crates/qnc-program-segments`
(markeri i slotovi samo iz frameova programa; uklonjeni MarkerMode, TrimIn/TrimOut),
`crates/qnc-segment-panel` (bez prekidaca), ugovori `program-segments` (baza, modul) i
`segment-panel`. Nije izvedeno: playlista programa i pustanje price (korak 3b),
pokrivalice Cover slot/Overwrite (3c), Sync/B-roll (3d). Verificirano: testovi
`qnc-content-store` (36), `qnc-program-segments` (13), `qnc-editorial-application`,
`cargo build --workspace`, `cargo run -p qnc-conformance`. Odobrenje je zatvoreno.
Verificirano: `cargo build --workspace`, testovi `qnc-broadcast-player` (78),
`qnc-broadcast-engine`, `qnc-playback-activity`, `qnc-ingest-import-worker`,
`qnc-ingest-worker`, `qnc-ingest-application`, `qnc-editorial-application`,
`qnc-source-preview`, `qnc-app` (13, novi test deaktivacije), `cargo run -p
qnc-conformance` bez upozorenja. Live provjera slijedi na release buildu.
Odobrenje je zatvoreno, obitelj je zamrznuta.

Zatvoreno (korak 2) ograniceno odobrenje 2026-09-18 (cetvrto): korisnik je
izricito potvrdio nastavak. Poslovna logika export presetova i JSON putanja
postavki izlazi iz `crates/qnc-project-desktop/src/project_advanced.rs` u dva
uska javna modula (novi crateovi `crates/qnc-settings-path` i
`crates/qnc-export-preset`, s ugovorima u `contracts/modules/`); 8 testova izlazi
iz desktop cratea (2 uz module, 1 uz `ProjectsState` u `qnc-project-application`,
2 provjere ugovora u `tools/qnc-conformance`, 3 tautoloska se uklanjaju).
Otkljucano: `crates/qnc-project-desktop/**`, `crates/qnc-project-application/**`,
dva nova crate-a, dva nova `*.module.json`, root `Cargo.toml`/`Cargo.lock` samo za
njih, `tools/qnc-conformance/src/main.rs` samo za dodane provjere. Verificirano:
conformance (nove provjere `Project form has no tests` i `Project layout and
shortcut reference`), cijeli workspace se kompajlira, testovi modula prolaze, zivi
smoke `qnc-project`. Obitelj je zamrznuta. Bez promjene
ponasanja. Sve ostalo ostaje zamrznuto.

Zatvoreno (korak 1) ograniceno odobrenje 2026-09-18 (cetvrto): korisnik je izricito
otkljucao sve sto je potrebno da forma Project ne sadrzi store ni poslovnu
logiku (§3, §10). Korak 1: `ProjectComponent` i `ApplicationSelection` selje
iz `crates/qnc-project-desktop` u novi crate `crates/qnc-project-application`
(po uzoru na `qnc-ingest-application`); desktop crate gubi ovisnost o
`qnc-project-store`. Otkljucano: `crates/qnc-project-desktop/**`,
`crates/qnc-project-application/**` (novo), root `Cargo.toml`/`Cargo.lock` samo
za taj crate, i samo pravila Project granice u `tools/qnc-conformance/src/main.rs`
(`scan_project_app_boundary`) koja trenutno traze suprotno. Bez promjene
ponasanja. Izdvojen je i `crates/qnc-application-selection` (uski javni modul,
ugovor `contracts/modules/application-selection.module.json`); `qnc-project-application`
je samo tanki composition root i ne smije rasti. Verificirano: conformance,
testovi, gradnja, zivi smoke `qnc-project`. Ostaje otvoreno (korak 2, treba
novo otkljucavanje): poslovna logika export presetova u `project_advanced.rs`
i 8 testova u desktop crateu. Obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (trece): korisnik je izricito otkljucao
samo `crates/qnc-dev-diagnostics/src/lib.rs`, funkciju `log_line`: cijela linija
se sastavlja u jedan string i pise jednim `write_all`, jer `writeln!` na
neuslojenom `File` salje vise zapisa pa se linije vise procesa isprepletu u
`player.log`. Bez promjene formata. Verificirano stresom: 4 procesa x 4 niti,
48000 linija, stara verzija 37259 neispravnih, nova 0; testovi i conformance
prolaze. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18 (drugo): korisnik je izricito otkljucao
samo `crates/qnc-broadcast-engine/src/av_sync.rs` za jednu promjenu: test
`recorded_player_log_picture_must_not_lag_sound` dobiva `#[ignore]` s
razlogom, jer cita lokalni, netrackirani `data/diagnostics/player.log` koji
skuplja stare sesije i zato nije deterministicki. Test ostaje pokretljiv s
`--ignored`. Verificirano: `qnc-broadcast-engine` lib testovi prolaze,
`qnc-conformance` prolazi. Odobrenje zatvoreno, obitelj je zamrznuta.

Zatvoreno ograniceno odobrenje 2026-09-18: korisnik je izricito otkljucao samo
`crates/qnc-broadcast-engine/src/av_sync.rs` za jednu promjenu: oznaka
`#![cfg(test)]` na pocetku datoteke, da `qnc-conformance` (player boundary)
prepozna test-only kod. Bez promjene ponasanja. Verificirano: `qnc-conformance`
prolazi sve provjere. Odobrenje je zatvoreno i cijela obitelj je ponovno
zamrznuta.

- Nema izmjena koda, ugovora, manifesta, seeda, conformancea ni razvojnih
  dokumenata bez izricite dozvole koja imenuje tocnu putanju ili modul i
  vrstu promjene.
- Opci zahtjevi, otvoreni §8.3 i "sredi preview" ne daju dozvolu.
- Zamrznuto ukljucuje, bez ogranicenja na ovaj popis:
  `apps/**`, `crates/**`, `tools/**`, `contracts/**`, `seed/**`,
  `docs/**` osim novog freeze zapisa kad korisnik trazi freeze/unlock,
  root `Cargo.toml` / `Cargo.lock` / `AGENTS.md` (osim ovog freeze zapisa
  kad korisnik trazi freeze/unlock).
- Posebno su zamrznuti i prije otvoreni dijelovi: `apps/qnc-app/**`,
  `crates/qnc-shell-desktop-api/**`, `crates/qnc-application-catalog/**`,
  `crates/qnc-contracts/**`, `tools/qnc-conformance/**`,
  `tools/qnc-app-catalog/**`, `tools/qnc-camera-catalog/**`,
  `tools/qnc-dev-diagnostics-app/**`, te svi moduli koji nisu navedeni
  u odjeljku 17.
- `qnc-ingest-components` i dalje ne smije postojati.
- Dopusteno bez otkljucavanja: citanje, audit, testovi, live pokretanje
  vec sastavljenih programa, te zapis poslovnih rezultata kroz vec
  zamrznute javne write putove. Radni projektni direktoriji i baze nisu
  freeze koda.
- Ako treba bilo kakva izmjena, prvo otkljucavanje. Ne granati privatnu
  kopiju zamrznutog modula.

Zatvoreno ograniceno odobrenje 2026-09-23 (source input adapteri): korisnik je
odobrio da se vise ulaznih adaptera (Racunalo, LAN, Intranet) modelira kao
samostalna javna lego komponenta, bez patchanja formi i bez aktivnog koda u
layoutu. Dodan je `crates/qnc-source-input` i ugovor
`contracts/modules/source-input.module.json`. Komponenta cita javne source
transport bindinge i izlozi eksplicitne adaptere `ComputerInputAdapter`,
`LanInputAdapter`, `IntranetInputAdapter`; ne skenira, ne probea, ne pise u DB i
ne zna za Ingest/MA/Story forme. Ingest browser smije kasnije birati adapter po
tabu kroz javni ugovor, ali ovaj rez ne mijenja `qnc-ingest-application` ni
desktop layout. Projektni sadrzaj i dalje ostaje iskljucivo u aktivnom
`project.db`/`WorkSettings`.

Zatvoreno ograniceno odobrenje 2026-09-23 (timeline asset read): nakon
preispitivanja odbacen je async/refresh smjer jer je uvodio petlju i dva izvora
za prikaz. Otkljucano samo: `crates/qnc-content-artifacts`,
`crates/qnc-source-preview` i Ingest kopiranje preview statea. Zadrzano je
racionalno pravilo: filmstrip putanje se citaju iz aktivnog `WorkSettings`
`products/filmstrip`, a Ingest source timeline asseti ne smiju se prepisivati iz
preview modula. Forma/layout nije diran, nema novog write puta i nema promjene
worker pokretanja.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story korak 4: model programa iz baze):
korisnik je rekao "da, nastavi"; vrijedi pravilo v5. Otkljucano i izvedeno samo:
`crates/qnc-content-store` (citanja: svi segmenti s `active`, svi markeri sa
`system_role`, slotovi s `has_cover`, odabir iz `story_state`; zapisi v5
`select_part` i `select_marker_slot`), `crates/qnc-program-segments` (model
cita markere i slotove iz baze umjesto da ih racuna; zakljucani pocetak i kraj;
v5 navigacija: slot od odabranog, inace pod playheadom, inace prvi prazni;
marker prije/poslije playheada; pocetni marker se ne odabire; odabir segmenta i
slota zapisuje se u bazu, pri otvaranju i nakon zapisa cita se iz baze; M
stvara marker, a odabrani marker premjesta samo izmedu susjeda (docs/94 7a);
tab Segment dobiva i neaktivne segmente), `crates/qnc-segment-panel` (granicni
marker crta se na kraju ranijeg reda kao v5 `local_marker_frame`; slot pokazuje
pokrivalicu; tab Segment prikazuje neaktivne sivo s "neaktivno" bez klika),
ugovori `program-segments` i `segment-panel` (moduli). Nije izvedeno: playlista i
puštanje programa, Source/Wrap pogled, pokrivalice, Sync/B-roll. Player, forme
(osim poziva javnih kockica) i layout ugovori nisu dirani. Verificirano: `cargo
test -p qnc-content-store` (43), `qnc-program-segments` (15), `qnc-segment-panel`
(1), `qnc-editorial-application` (11), `cargo build --workspace`, `cargo run -p
qnc-conformance`. Dopuna (odluka korisnika): korisnik markere postavlja na Wrap
segmentima, ne na program timelineu; M salje segment pod playheadom i frame
unutar njega (v5 `create_marker_from_part_frame`, `origin_local_frame`), a
spremiste iz toga izvodi frame programa. Dopuna 2 (korisnik: playhead mora na klik, brzo): klik na red
segmenta ili slot postavlja Wrap playhead odmah na kliknuti frame (v5
`set_wrap_playhead_frame`); potvrdjeni frameovi playera preuzimaju tek kad
player dode na trazeni frame. Svaki red panela ima vlastiti egui id (inace
klik dobiva samo zadnji nacrtani timeline). Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story korak 3: slotovi i pokrivalice u
bazi): korisnik je rekao "ok, nastavi" nakon koraka 1; vrijedi pravilo v5
(`qnc-story-segment-timeline.mdc`). Otkljucano i izvedeno samo:
`crates/qnc-content-store` (nakon svake promjene price `finalize_story`: pocetni i
zavrsni marker, `story_marker_slots` iznova izmedu svaka dva susjedna markera s
identitetom `start_marker_id|end_marker_id`; odluka korisnika: brisanje
markera (rucno ili s obrisanim segmentom) brise i njegove slotove s njihovim
pokrivalicama; svaka druga promjena ostavlja pokrivalicu na istom paru markera s
novim frameovima, inace na slotu s istim pocetnim markerom (novi marker ju je
skratio); odabrani slot i pokrivalica se ponistavaju ako vise ne postoje;
`story_covers.slot_id`, `public_story_marker_slots.has_cover`; razvojne
`story_covers` bez `slot_id` se brisu), ugovor `story.database.json` (pravila).
Nije izvedeno: citanje slotova u modelu programa (korak 4), izrada pokrivalica
(korak 9), prikaz neaktivnih segmenata. Player, forme i layout nisu dirani.
Verificirano: `cargo test -p qnc-content-store` (42), `qnc-program-segments`
(13), `qnc-editorial-application` (11), `cargo build --workspace`,
`cargo run -p qnc-conformance`. Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story v5 tablice, korak 1): prvi dio
je napravio drugi AI agent prema dokumentu `qnc-v5-story-postupak-2026-09-24.md`,
bez korisnikove odluke o otvorenim pitanjima; audit je to utvrdio, korisnik je
izabrao da se rad zadrzi i ispravi (plan B). Odluke korisnika 2026-09-24: vrijedi
PRAVILO v5 (`QNC_v5/.cursor/rules/qnc-story-segment-timeline.mdc`), kod pise
QNC; kad se slotovi promijene pokrivalica se ponovno veze ili skracuje, ne brise;
identitet slota je par markera; razvojni zapisi se smiju brisati. Otkljucano i
izvedeno samo: `crates/qnc-content-store` (tablice `story_state`, `story_parts`,
`story_markers`, `story_marker_slots`, `story_covers`, `story_object_history` i
javni pogledi; razvojne `program_segments`/`program_markers` brisu se pri
otvaranju; vrsta samo `tonovi`/`offovi`; segment se puni kao v5
`segment_source_from_clip_frames` (tc, sekunde, oznaka i boja trajanja); brisanje
segmenta je v5 `delete_part`: `active = 0`, markeri unutar prozora se brisu,
kasniji se pomicu ulijevo, odabir prelazi na najblizi segment; M je v5
`create_marker_frame`: pamti segment (`origin_part_id`), marker na istom frameu se
osvjezava; pomak i brisanje markera su v5 `update_marker_frame`/`delete_marker`
sa zakljucanim pocetkom i krajem; pocetni i zavrsni marker su v5
`ensure_start_marker`/`ensure_end_marker`; javni pogled `public_story_parts`
nosi i neaktivne segmente), `crates/qnc-program-segments` (samo `tonovi`/`offovi`,
M salje odabrani segment), ugovori `story.database.json`,
`program-segments.database.json`, `program-segments.module.json`. Nije izvedeno:
preracun slotova u bazu i vezanje pokrivalica (korak 3), prikaz neaktivnih
segmenata u tabu Segment, Source/Wrap playlist, player. Player, forme i layout
nisu dirani. Verificirano: `cargo test -p qnc-content-store` (38),
`qnc-program-segments` (13), `qnc-editorial-application` (11), `cargo build
--workspace`, `cargo run -p qnc-conformance`. Odobrenje je zatvoreno.

Otvoreno ograniceno odobrenje 2026-09-25 (Story pokrivalice i Sync/B-roll, v5):
korisnik je potvrdio plan P1-P4 ("ok, odkljucaj i kreni"). v5: Cover slot i B
su `quick_cover` (prazan odabrani M-M slot + IN/OUT izvornog klipa: nastaje
virtualni kadar kategorije `cover` pa pokrivalica u slotu), Overwrite mijenja
pokrivalicu slota, izvor se ne skracuje na slot; Sync/B-roll: IN na izvoru,
Space pusta program od markera prije Wrap playheada s privremenim overlayem
(slika + A2, A1 price ostaje), O zavrsava slot (marker na kraju ako ga nema),
Enter upisuje pokrivalicu. Otkljucano: `crates/qnc-content-store`,
`crates/qnc-virtual-shots`, `crates/qnc-program-segments`,
`crates/qnc-segment-panel`, `crates/qnc-program-input`,
`crates/qnc-source-preview`, `crates/qnc-editorial-application`,
`crates/qnc-editorial-desktop` (tab B-roll i crtanje pokrivalica kao dodaci
zakljucanom sucelju), novi javni `crates/qnc-sync-cover`, ugovori modula i baze,
`contracts/qnc-keyboard-shortcuts.json` samo ako treba nova akcija, root Cargo.
Tijekom rada dirani su i `crates/qnc-wrap-session` (ponovno otvaranje programa
nakon upisa, `hold` za Sync) i `crates/qnc-virtual-short-cards` (oznaka B-roll
kartice); korisniku je to javljeno.
Izvedeno do sada: P1 (56e9f9f) pokrivalica je u bazi virtualni kadar klase
`b_roll` (odluka korisnika: pokrivalice idu u B-roll, ne u virtual short) i red
`story_covers` u slotu, zamjena postojece, brisanje i odabir; P2 (247f71d,
c457c6f) Cover slot/B i Overwrite iz IN/OUT source timelinea, crtanje
pokrivalica na Wrap redovima, program se ponovno otvara nakon upisa; P3
(5595e63) tab B-roll; P4 (8a1cc4c) Sync/B-roll: `qnc-sync-cover`, prozor
programa s privremenom pokrivalicom u istom playeru (`qnc-program-input`
`window_loader`), O zatvara slot (marker na kraju ako ga nema), Enter ili kraj
izvora upisuje pokrivalicu. Live provjereno: Cover slot, Overwrite, Play kroz
pokrivalicu u Off segmentu, tab B-roll. Sync jos nije provjeren uzivo.
Dopuna 2026-09-25 (korisnik: "Keyboard map je QNC"): otkljucan
`crates/qnc-keyboard-shortcut` samo za eksplicitnu tablicu egui tipka -> catalog
`key`/`code` (M, B, T, V, S, U, Z, R, Y, W, Delete, Backspace, Escape, Tab, F1,
Home, gore/dolje, [, ], /, '), jer do sada nisu stizale do aplikacije; katalog se
ne mijenja, Ingest i Project koriste samo vec podrzane tipke. Pravila korisnika:
Ctrl+tipka odabire (Ctrl+M marker u uredivanje, strelice/M/povlacenje, Enter
potvrda, Esc odustani), Shift+tipka dodaje (Shift+M novi marker), Delete brise
samo ono uzeto s Ctrl+; Sync ide od source IN do source OUT ili prvog sljedeceg
markera i ceka Enter.
Dopuna 2026-09-25 (korisnik "da" na plan): keyboard map vrijedi za panel u
fokusu, ne za aplikaciju (v5 `story/focus.rs` `PanelFocus`): Pool, Source
timeline (samo I/O i playhead izvora), Segmenti (Wrap, markeri, pokrivalice).
Otkljucano: novi javni `crates/qnc-panel-focus` (+ ugovor modula), dodatak
`crates/qnc-editorial-desktop` samo za oznaku fokusiranog panela,
`crates/qnc-timeline` samo za povlacenje M markera (intent `DragMarker`, bez upisa;
Enter potvrduje), `crates/qnc-segment-panel`, `crates/qnc-editorial-application`,
root Cargo.
Dopuna 2026-09-25 (korisnik "dodaj UNDO dugme"): Undo/Redo za cijelu pricu,
ne samo pokrivalice kao v5. Svaka izmjena price (segmenti, markeri, pokrivalice)
sprema prethodno stanje tablica price u `story_undo` u projektnoj bazi
(`crates/qnc-content-store`, operacije UndoStory/RedoStory); dugmad Undo i Redo u
traci panela Segmenti (`crates/qnc-segment-panel`), tipke kataloga `undo_object` /
`redo_object` kroz `crates/qnc-program-segments` kad je fokus na Segmentima.
B-roll kadrovi koje je pokrivalica stvorila ostaju. Ugovor `story.database.json`.

Otvoreno ograniceno odobrenje 2026-09-26 (klip bez probea kad XML moze, v5): korisnik: "ako
moze proci bez probe, probe ne treba; ako ne moze, probe samo za fajlove koji se uvoze";
"ok odkljucaj sto moras". v5: probe samo kad zapisu nedostaje fps, trajanje u frameovima ili
razlucivost (`media_probe_snapshot_ready`); player iz baze dobiva samo trajanje, fps, pocetni TC
i opcionalni opis slike/zvuka (`SourceRuntime`), a indeks streama, time_base, start_pts i
frekvenciju zvuka cita dekoder pri otvaranju datoteke. Nalaz: novi projekt bez naslijedenih
probe zapisa javlja `IncompleteMedia`, jer QNC engine te podatke spremnika uzima iz baze.
Otkljucano: `crates/qnc-sony-metadata` (sve iz MEDIAPRO.XML i XML-a klipa), `crates/qnc-camera-*`,
`crates/qnc-media-metadata`, `crates/qnc-media-records`, `crates/qnc-player-input`,
`crates/qnc-broadcast-engine`, `crates/qnc-broadcast-player`, `tools/qnc-player-runner`,
`crates/qnc-media-decode`, `crates/qnc-ffmpeg-decode`, `crates/qnc-decoder-catalog`,
`crates/qnc-ingest-select`, `crates/qnc-ingest-import-worker`, testovi, ugovori, dokumenti.
Redoslijed: 2) Sony citac, 1) podaci spremnika od dekodera, 3) probe samo pri Uvezi za klip bez
fps/trajanja/razlucivosti. Ne dira se UI ni layout.
Izvedeno 2): Sony citac iz XML-a upisuje i trajanje u sekundama (tocan broj frameova pri
stalnom fps-u, original i proxy), dimenzije iz eksplicitne oznake kodeka (`AVC_Proxy_1920_1080_...`)
i omjer piksela iz `aspectRatio`; boja iz `AcquisitionRecord` i audio streamovi iz CH opisa se ne
prenose (docs/26).
Audit v5 (26. 9.): v5 ne cita Sony XML; nakon skeniranja pozadinski raspored (`ingest_probe/scheduler.rs`)
za svaki klip izvora bez fps/trajanja/razlucivosti radi jedan `media_probe`, a Ingest preview neuvezenog
klipa ceka taj rezultat. Nas QNC od 24. 9. radio je samo jer je dijeljena baza racunala imala probe zapise
od 7. 9.; pravilo "XML = nikad probe" (19. 9.) nikad nije radilo. Odluka korisnika: v5 nacin, lego.
Izvedeno: nova kockica `crates/qnc-record-probe` (postupak dovrsetka zapisa premjesten iz Selecta, bez
nove logike; "sto nedostaje" je postojeca `qnc_media_metadata_compose::required_probes`), Select zapis
kartice ostavlja u fazi kamera za pozadinski dovrsetak, klip bez XML-a Select dovrsava odmah kroz istu
kockicu, red uvoza prima zapis kartice koji ceka dovrsetak (`qnc-content-store` `ready`).
Zatim nova kockica `crates/qnc-record-completion` (v5 `ingest_probe/scheduler.rs`): nade klipove aktivnog
projekta u fazi kamera, dovrsi svaki kroz `qnc-record-probe` (bez probea i bez kartice kad nista ne
nedostaje), objavi konacni zapis i ceka dok player radi; izvor probea joj daje proces koji je slaze
(`tools/qnc-ingest-worker`), pa ne ovisi o Ingestu (C6). Ingest pokrece pozadinski proces i nakon
uspjesnog Selecta.
Provjera od cistog stanja na izoliranoj kopiji (vlastiti QNC korijen, kopija baze projekata s vezom
projekta preusmjerenom na kopiju `project.db`, ocisceni klipovi i zapisi; kartica samo citana): Select 98
klipova bez probea, trajanja iz XML-a, pozadinski dovrsetak, preview neuvezenog klipa radi. Nalaz: 12
klipova serije 2676-2690 (proxy s vlastitim dur/fps u MEDIAPRO) nije dovrseno jer je citac proxyju upisao
trajanje iz XML-a, a spremnik proxyja je dulji od videa (zvuk) pa se sukobio s probeom; ispravljeno
(trajanje samo za original). Timecode izvora dodan i u Ingest source dock (`qnc-ingest-desktop` jedan
redak, `qnc-ingest-application` view model; C10 granice nisu podignute).
Brzina (korisnik: "taj nacin probe je usporio postupak, trazi rjesenje koje ne usporava"): mjereno,
jedan probe MXF originala na SD kartici oko 0,5 s, MP4 proxyja oko 0,1 s; usko grlo je kartica, ne
procesor. Izvedeno: `qnc-record-completion` radi s vise radnika (red, serijski upis kroz posrednika),
klip koji preview otvara ide prvi (`qnc-playback-activity` `PLAYBACK_CLIP`, preview ga javlja pri
otvaranju), preview klipa bez potpunog zapisa pokazuje "Podaci klipa se pripremaju..." i sam ponovno
otvara svake sekunde do 3 min (`qnc-player-input::is_incomplete_media`, `qnc-source-preview`),
dovrsetak ide istodobno s filmstrip/wave, 2 radnika (8 radnika je gusvalo karticu: istek vremena
probea i klip zauvijek nedovrsen). Provjera od cistog stanja na izoliranoj kopiji: Select bez
probea, kliknuti neuvezeni klip svira za nekoliko sekundi s TC-om kamere u Ingest docku, svih 98
zapisa dovrseno za oko 46 s, 196 probeova spremljeno, nijedan neuspjeh; stvarne baze nepromijenjene.
Otvoreni rizik: istek vremena probea zapise kao neuspjeh i klip ostaje nedovrsen (ugovor zapisa
dopusta samo jedan pokusaj po mediju); treba odluku o ponovnom pokusaju nakon isteka vremena.
Odluka korisnika 2026-09-26 ("zadrzi dosadasnje rjesenje"): kad svira proxy slika, zvuk i dalje
ide iz originala (§8.2, docs/66), pa pozadinski dovrsetak i dalje probe-a original i proxy.
Postavka probea (8 MB, 1 s za sve spremnike) ostaje ista; mjereno: MXF s kratkom analizom daje
iste podatke za oko 0,21 s umjesto 0,5 s, a MP4 s kratkom analizom gubi pixel format, profil i boju.
Izvedeno (ponovni pokusaj, v5 `JobHandlerError::retryable`: neuspjeli media probe posao vraca se u red):
probe koji medij nije procitao (istek vremena, ffprobe se nije pokrenuo) zapisuje se kao novi ishod
`Interrupted` (`qnc-media-records`) i smije se ponovno preuzeti (`qnc-media-record-db` zamjenjuje taj
pokusaj novim); spremljen, neuspio i nesiguran ishod ostaju konacni, pa se losa datoteka ne vrti u
petlji. `qnc-record-probe` vraca vrstu greske, `qnc-record-completion` prekinuti klip vraca na kraj
reda do 3 puta u jednom prolazu, a sljedeci prolaz ga opet uzima. Testovi: prekinuti pa dovrseni
klip, neuspio probe se ne ponavlja, zamjena prekinutog pokusaja u spremistu.
Live nalaz 30. 9. (korisnik: filmstrip presporo): iz baze, filmstrip sam 0,96 s po klipu (`tesssrt4`),
a s probeom uz njega 1,5-2,0 s (`hhhggff`, novi projekt; 95-96 od 98 zapisa dovrseno usred filmstripa);
kartica je usko grlo. Izvedeno: `qnc-record-completion` `Run::card_busy` (dok filmstrip/wave
citaju karticu, dovrsava se samo klip koji preview trazi, ostali odmah nakon njih),
`tools/qnc-ingest-worker` javlja zauzetost dok radi `run_artifacts`; test da dok je kartica
zauzeta probe cita samo medije trazenog klipa. Uz to worker cita red uvoza iz baze svake
sekunde i jos jednom nakon otpustanja najma (Uvezi dok radi worker Selecta vise ne propada).
Mjerenje na izoliranoj kopiji (30. 9.): isti release worker bez aplikacije napravi filmstrip 0,83 s po
klipu i wave uz njega; u pravom radu 1,5-1,6 s i bez wavea. Uzrok: nakon Selecta Ingest je generirao
filmstrip/wave i u svom procesu (`sync_timeline_artifact_content_db`) i pokretao worker (od `3489103`),
dva generatora i 4 ffmpega na istoj kartici. Izvedeno: jedan generator, pozadinski worker (najam ga
drzi jednim); Ingest ga samo budi (`wake_artifact_worker`, i pri ucitavanju projekta), ne konfigurira
generatore u svom procesu, a prikazani klip ponovno cita iz baze jednom u sekundi dok mu nedostaje
filmstrip ili wave (`qnc-content-artifacts::ProjectArtifacts::refresh_due`). C10 Ingest 1717.

Zatvoreno ograniceno odobrenje 2026-09-30 (filmstrip po projektnim postavkama): korisnik je trazio da
samostalno pokretanje filmstripa kontroliraju projektne postavke ("mozda nekome nece ni trebati
filmstrip"), kvacicu u Project postavkama pored AI postavki (zadano ukljuceno, auto), a kad je filmstrip
iskljucen, poster klipa u traci filmstripa na timelineu; odobrio "odmrznuti i zamrznuti" Project.
Otkljucano i izvedeno samo: `seed/system_seed.json` (svaki sistemski predlozak: `artifacts` =
`{filmstrip: auto, wave: auto}`), `crates/qnc-project-store` (`complete_project_settings` dopunjuje
`artifacts` kao `products`), `contracts/ui/project.layout.json` (`ai_switches`: tri postojece AI
kvacice i nova "Filmstrip automatski nakon Odaberi"), `crates/qnc-project-desktop` (AI okvir samo
poziva kockicu; forma je manja: C10 3387, C2 109), novi javni `crates/qnc-settings-switches` (+ ugovor;
kvacice iz layout ugovora, citanje i zapis nacrta postavki), `crates/qnc-settings-path` (`switch_on`),
`crates/qnc-work-settings` (`artifact_mode` strogo, `filmstrip_made` samo za prikaz; testni fixture
dobio `products` kao pravi projekt, 4 testa su padala od 23. 9.), `crates/qnc-content-artifacts`
(`off` generatoru ne daje klip), `crates/qnc-timeline-assets` (`with_clip_poster`: poster na svih 13
mjesta trake samo kad je filmstrip `off`; iznimka od §8 po odluci korisnika, pravi filmstrip se nikad
ne zamjenjuje), `crates/qnc-ingest-application` i `crates/qnc-editorial-application` (samo poziv),
`crates/qnc-player-input` (testni primjerak). `on_demand` nije izveden (trazi odluku tko ga pokrece u
Storyju/MA). Stari projekti bez bloka dobivaju kontroliranu gresku samo za artefakte (korisnik ih brise).
Odobrenje je zatvoreno, Project je ponovno zamrznut.

Otvoreno ograniceno odobrenje 2026-09-26 (zapisi medija u bazi projekta, v5): korisnik je
otkljucao Ingest lanac uz pravila: Ingest nije vlasnik ni baze ni klipova; projektne
postavke odlucuju gdje sto zivi; aplikacija je OS-neutralna. v5: `ingest_assets`,
`probe_json`, `metadata_json`, poslovi i serije uvoza su tablice u bazi projekta, svaki
upis ide kroz `ProjectDbBroker::serialize_project_write`; nema baze racunala koju dijele
projekti. Nalaz audita: `QNC/data/ingest_media_records.db`, `ingest_source_index.db` i
`ingest_registry.db` (vlasnik `qnc.ingest`, lokacija iz `ingest-transport.json`) dijele svi
projekti; novi projekt nasljeduje zapise od 7. 9. (ffprobe uz XML). Plan: K1 ugovori
(`media-records`, `source-index`: vlasnik projekt, tablice u `project.db`, pisac
`qnc-content-store`); K2 `qnc-media-record-db` i `qnc-source-index-db` zive u bazi
projekta (provjera samo svojih tablica, `media_write_receipts`/`source_write_receipts`);
K3 posrednik `qnc-content-store` izvrsava njihove operacije serijski; K4 Select salje
zapise kroz transport projekta, `ingest-transport.json` ostaje samo za racunalo
(montiranje kartice, ffprobe, katalog kamera); K5 Select sesije u `project.db`, identitet
kartice kroz OS-neutralni `qnc-source-volumes`; K6 conformance pravilo i brisanje
`data/ingest_content.db`. Stare baze u `QNC/data` se ne brisu bez korisnika.
Otkljucano: `qnc-media-record-db`, `qnc-source-index-db`, `qnc-content-store`,
`qnc-ingest-select`, `qnc-ingest-store`, `qnc-ingest-application`, `qnc-source-bindings`,
`qnc-dir-browser`, `qnc-work-settings`, `qnc-ingest-import-worker`, `tools/qnc-ingest-worker`,
novi `qnc-source-volumes`, ugovori baza, modula i aplikacije Ingest, `tools/qnc-conformance`,
root Cargo. UI i layout se ne diraju. Izvedeno: K1; K2 (`open_in_project`,
`media_write_receipts`, `source_write_receipts`, journal PERSIST kao baza projekta); K3
(`qnc-content-store`: operacije `MediaRecord`/`SourceIndex` i serijski posrednik
`ContentRecords`). Korisnik 2026-09-26: `qnc-content-store` je monolit i mora preci na lego
sistem ("kreni"). Novi plan B1-B7: B1 javni posrednik `crates/qnc-db-broker` (v5
`ProjectDbBroker`: otvara samo postojecu bazu tog projekta, PERSIST/WAL, busy timeout,
serijski pisac `ProjectDbWriter`, isti zahtjevi Local/LAN/Intranet, bez SQL-a ijedne
domene; moduli tablica dobivaju vezu i posjeduju svoje tablice); B2 zapisi medija i
izvorni indeks kao moduli tablica posrednika (`MediaRecordsModule`/`ProjectMediaRecords`,
`SourceIndexModule`/`ProjectSourceIndex`), K3 dodaci iz `qnc-content-store` uklonjeni
(zadrzan samo prelazak dviju transakcija na IMMEDIATE), Select pise zapise kroz posrednika
(`SelectTarget`), `ingest-transport.json` vise nema baza zapisa; B3-B7 Story i undo,
virtualni kadrovi, filmstrip/wave, red uvoza i runtime, katalog klipova, pa brisanje
`qnc-content-store`. Otkljucani su i `qnc-db-broker` (novi) i ugovor
`contracts/modules/db-broker.module.json`. Izvedeno: B1, B2 (ukljucuje K4).
Izvedeno B3: novi modul tablica `crates/qnc-program-db` (+ ugovor `contracts/modules/program-db.module.json`;
naziv je neutralan jer `qnc-story-*` pripada obitelji aplikacije Story) posjeduje `story_*` tablice i
`story_undo`: kod, undo i 22 testa premjesteni iz `qnc-content-store` bez promjene ponasanja; vlastiti
autorizator (pise samo svoje tablice), klip u prici samo ako je uvezen kroz jedini citac kataloga
(`qnc-content-read::imported_on`, C11). Pokrivalica kao v5 (`add_virtual_shot_from_frames` pa
`create_cover`): `qnc-content-store` `CreateCoverShot` stvara B-roll kadar, zatim `qnc-program-db`
`CreateCover` s tim kadrom. `qnc-db-broker` dobio `submit`/`Pending` (upis bez cekanja forme).
Potrosaci: `qnc-program-segments` (`StoryReader`/`StoryWriter`, pokrivalica u dva koraka),
`qnc-program-input` i `qnc-source-preview` (cilj baze projekta), `qnc-editorial-application`.
Ugovori `story`/`program-segments` baze: vlasnik tablica `qnc.module.program-db`. Conformance:
`qnc-program-db` na popisu javnih DB vlasnika. Nije izvedeno: live provjera Storyja na release buildu.
Izvedeno B4: `crates/qnc-virtual-shots` je sam modul tablica posrednika i posjeduje `virtual_shots`
(shortovi, B-roll kadrovi pokrivalica, IN/OUT slicice); kod premjesten iz `qnc-content-store` bez
promjene ponasanja, vlastiti autorizator, uvezeni klip kroz `qnc-content-read::imported_on`.
`qnc-content-store` vise nema ni jednu operaciju virtualnih kadrova (ni odgovore `SavedShort`/`Created`,
Select ih vise ne ocekuje). Pokrivalica: `qnc-program-segments` salje `CreateCoverShot` modulu
virtualnih kadrova, pa `CreateCover` modulu price, oboje kroz posrednika; cilj sadrzaja mu vise ne
treba. Editorial drzi cilj sadrzaja (timecode) i cilj baze projekta (prica, virtualni kadrovi).
Testovi `qnc-virtual-shots` (6, novi), `qnc-program-db` (22), `qnc-program-segments` (21).
Izvedeno B5: novi modul tablica `crates/qnc-artifact-db` (+ `contracts/modules/artifact-db.module.json` i
ugovor baze `contracts/databases/timeline-artifacts.database.json`; tablice i pogledi izasli iz ugovora
`ingest-content` i iz `schema.sql`) posjeduje `filmstrip_artifacts`, `filmstrip_frames`, `wave_artifacts`;
kod i testovi premjesteni bez promjene ponasanja, zapis filmstripa jedan tip (`qnc-filmstrip`), bez
pretvorbi. Tablice ostaju sa stranim kljucem na `clips` (bez gubitka postojecih artefakata), zato
brisanje nestalog klipa ide u dva koraka: Select prvo salje `ForgetClips` modulu artefakata (samo za
klip `detected`/`failed`, kroz istog posrednika kao zapise medija), pa katalog brise klip.
`qnc-content-read` dobio `import_status_on` i `media_uris_on` (jedini citac kataloga, C11).
`qnc-content-artifacts` cita i pise artefakte kroz modul (cilj baze iz postavki), `focus` i
`timeline_artifact_reader` vise ne primaju cilj sadrzaja (`qnc-ingest-application`,
`qnc-ingest-preview-source` po jedan poziv); dijagnostika cita kroz modul. C10 baseline Ingest 1749.

Otvoreno ograniceno odobrenje 2026-09-25 (originalni timecode izvora): korisnik je
odredio: broadcast montaza radi po timecodeu (veza proxy-original i export); pocetni TC
klipa cita se iz XML-a kamere, ffprobe samo za klip bez XML-a (takvi klipovi sluze za
test tog puta); source klip odreduje fps (50p broji 0-49, LTC kamere na 25 s
`halfStep` se preracunava); program smije imati svoj interni TC, a XML export i ravna
playlista programa koriste originalne TC-ove izvora. Otkljucano: novi javni
`crates/qnc-source-timecode` (+ ugovor `contracts/modules/source-timecode.module.json`),
`crates/qnc-source-dock` (samo IN/OUT/Trajanje kao TC kad je TC dan; bez TC-a ostaju
brojevi frameova, pa se Ingest ne mijenja), `crates/qnc-editorial-application`,
`crates/qnc-editorial-desktop`, root Cargo. Koraci: 1) modul i Story source dock
(izvedeno: testovi, citanje kopije baze: 2676 -> 00:14:37:16, 2002 -> 17:35:04:42 isto
kao ffprobe); 2) segmenti, pokrivalice i tab Segment s izvornim IN/OUT u TC-u;
3) TC programa; 4) ravna playlista i export s originalnim TC-om. Live prikaz u docku
nije provjeren jer kartica G: nije bila spojena (player ne otvara medij).
Izvedeno i korak 3 (v5 `segment_panel.rs` zaglavlje): Segment, Playhead i Trajanje u
panelu Segmenti su interni TC programa HH:MM:SS:FF od 00:00:00:00
(`SegmentsView::timecode`, otkljucani `crates/qnc-program-segments` i
`crates/qnc-segment-panel`); live provjereno. v5 u retcima segmenata ne prikazuje TC,
pa korak 2 ide uz korak 4 (izvorni TC segmenata i pokrivalica treba playlisti i
exportu, a export jos ne postoji).

Zatvoreno ograniceno odobrenje 2026-09-25 (vlasnik baze je projekt): korisnik je
odredio: ako baza ima vlasnika, to je projekt; nijedna forma ne pise u bazu pa ne moze
biti ni vlasnik; podaci nastali u aplikaciji stizu u bazu samo preko posrednika, jedinog
javnog pisca `qnc-content-store`. Otkljucano i izvedeno samo: ugovori baza u `project.db`
(`ingest-content`, `story`, `virtual-shots`, `program-segments`, `media-assist*`): polje
`database_owner: project`, `owner_application`/`write_owner` = `qnc.module.content-store`,
`ownership_rule`; aplikacijski ugovori `ingest`, `story`, `media-assist*` vise ne navode
te baze u `owned_database_contracts`; `crates/qnc-ingest-store` provjerava ugovor
`ingest-content` pod piscem `qnc.module.content-store`. Nije mijenjano: nazivi `ingest_*`
(URI, tablica `ingest_content_schema`), host baze `ingest-registry`, `source-index`,
`media-records` (zasebne datoteke u `data/`, izvan projekta; ostaje otvoreno pitanje).
Verificirano: testovi `qnc-ingest-store`, `qnc-db-contract`, `qnc-content-store`,
`qnc-virtual-shots`, `qnc-contracts`, `qnc-conformance`, `--check-contracts` svih
aplikacija, `cargo build --workspace`, conformance. Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story program playlista, koraci 1 do 4):
korisnik je potvrdio opci plan prijenosa v5 pustanja programa (Wrap) i dodao:
korisnik mora moci promijeniti koji source kanal ide na A1 (snimatelj ponekad
snimi razgovor na kanalu 2 jer mikrofon na kanalu 1 nije bio ukljucen); isto
za pokrivalicu i A2. Pocetna vrijednost je v5: kanal 1 izvora (indeks 0).
Odluka korisnika: samo jedan kanal ide na A1 (izbor jednog od postojecih kanala
klipa, bez zbrajanja kanala); kontrola je mali prozor na klik oznake A1 u Wrap
segmentu s popisom stvarnih kanala klipa (korak sucelja, trazi otkljucavanje).
Otkljucano samo: novi javni `crates/qnc-program-playlist` i ugovor
`contracts/modules/program-playlist.module.json` (doslovno v5
`qnc-program-playlist` + `qnc-service-contracts/program_playlist.rs`: ravna
playlista programa, Ton = slika + A1, Off = bez slike + A1, pokrivalica =
slika + A2 uz A1 segmenta, jedan fps, prozor programa, privremeni overlay), root
`Cargo.toml`/`Cargo.lock`. Razlike prema v5 po QNC pravilima: broj izlaznih
kanala dolazi od pozivatelja (projektni `audio.channels`), medij je QNC URI
(ne raw path), nema zamjenskih vrijednosti (1920x1080, 48 kHz), source kanal
za A1/A2 je polje segmenta/pokrivalice. Korak 2 (conformance C5 ne pusta
javni modul bez korisnika): `crates/qnc-content-store` (stupci
`a1_source_channel` u `story_parts` i `a2_source_channel` u `story_covers`,
pocetno kanal 1; citanje pokrivalica), novi javni `crates/qnc-program-input`
(samo citanje: prica iz `project.db` + medij svakog klipa kroz
`qnc-player-input`, slika po `playback.input`, zvuk iz originala) i njegov
ugovor, ugovor `story.database.json`. Player i sucelje nisu dio ovih koraka.
Korak 3 (korisnik "da", 2026-09-24): isti Broadcast Player pusta program kao
jedan virtualni izvor na osi programa (v5 `qnc-player-runtime/program_playlist.rs`):
adapter slike daje sliku po itemu programa ili crno, adapter zvuka slaze A1/A2
po izlaznim kanalima projekta ili tisinu; sat, cue, play, pauza i korak ostaju
isti. Otkljucano: `crates/qnc-player-contract`, `crates/qnc-player-input`,
`crates/qnc-player-launcher`, `crates/qnc-player-client`,
`tools/qnc-player-runner`, `crates/qnc-broadcast-engine` i njihovi testovi i
ugovori. Monitor i frame-map ostaju isti; nema novog probea, nema zamjenskog
dekodera, nema HTTP ulaza dekoderu.
Korak 4 (korisnik "odkljucaj", 2026-09-25): otkljucano jos
`tools/qnc-conformance` samo za dopustenu ovisnost `qnc-program-playlist`
(cisti ugovor playliste, bez I/O i baze) u granici playera, te
`crates/qnc-source-preview` i `crates/qnc-editorial-application` za spoj s
Wrapom: Wrap otvara program u istom previewu/playeru i na istom monitoru,
source dock za to vrijeme pokazuje izabrani izvorni klip (v5), klik u poolu
vraca izvorni klip.
Izvedeno: `qnc-program-playlist` (ugovor i graditelj ravne playliste, 13
testova); `qnc-content-store` (`a1_source_channel`, `a2_source_channel`,
`ListCovers`; razvojna prica bez tih stupaca se brise pri otvaranju, 45 testova);
`qnc-program-input` (prica + medij svakog klipa -> `ProgramInput`, `loader`);
`qnc-player-input` (`ProgramInput` s provjerom: slika je medij reprodukcije, zvuk
original); `qnc-broadcast-engine` (`ProgramPlan`, `ProgramVideo`: jedan niz
dekodiranja u frameovima programa kroz rezove, pretvarac po formatu slike, Off
crn; `ProgramAudio`: izabrani kanal na A1/A2, ostali izlazi tisina; `DeviceSink`
izdvojen iz zvuka klipa bez promjene ponasanja; `InputPlan::for_program`; 28
testova); runner (klip ili program, veza po izvoru medija, program samo za
preview monitor); klijent/launcher (`LaunchInput`, `prepare_program_launch`);
`qnc-source-preview` (`open_program`, `show_program_frame`: dok program svira
source klip i timeline ostaju, klik na source timeline vraca klip);
`qnc-wrap-session` (`apply` vraca zahtjev); editorial application samo predaje
Wrap frameove previewu; conformance: `qnc-program-playlist` dopusten u granici
playera. Verificirano: testovi navedenih crateova, `cargo run -p qnc-conformance`,
release build, live: klik na Wrap segment otvara sesiju `program:<projekt>` u
istom playeru, `Cue(38)`, `Ready`, monitor frame 38 (Off = crno), source dock
ostaje na izvornom klipu. Nije provjereno uzivo: Play kroz rezove, slika Ton
segmenta i pokrivalice, zvuk A1/A2. Nije izvedeno: nova snimka programa nakon
izmjene price dok je Wrap otvoren (program se ponovno gradi tek pri sljedecem
otvaranju), kontrola izbora kanala u sucelju. Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-24 (Story Wrap kao samostalna komponenta):
korisnik je upozorio "krivo vezes source i segment.. to nije tako u qnc v5" i
predlozio samostalnu Wrap segment komponentu "da ne brkas source i wrap i
program". v5 (`qnc-app/src/story.rs`, `editorial_playback_transport.rs`,
`editorial_playback_session.rs`): Source i Wrap su dva nacina rada s dva
playheada; Wrap pusta PROGRAM (`OpenProgram` na frameu programa, zatim
`ScrubFrame` u istoj sesiji), a izbor klipa, source timeline i IN/OUT se ne
diraju (`source_dock_ignores_segment_carrier_when_wrap_is_active`). Otkljucano
samo: novi javni `crates/qnc-wrap-session` i ugovor
`contracts/modules/wrap-session.module.json` (Wrap nacin, Wrap playhead na osi
programa, zahtjev Open/Scrub prema playeru programa), `crates/qnc-program-segments`
(uklanja se veza segmenta na izvorni klip u playeru: Cue, CueStep, wrap_segment,
drive_player; model samo trazi frame programa), `crates/qnc-editorial-application`
(sastavlja Wrap; u Wrapu ne otvara izvorni klip i ne pusta source; monitor
pise da player programa nije spreman), ugovor `program-segments.module.json`,
root `Cargo.toml`/`Cargo.lock`. Player programa (playlista i player) nije dio
ovog koraka. Forme i layout se ne diraju.
Dopuna korisnika: "monitor ostaje isti, broadcast player isti... samo wrap
timeline" - nova komponenta je samo Wrap timeline; isti monitor i isti player
ostaju za sve, bez poruke na monitoru. Izvedeno: `qnc-wrap-session` (Wrap nacin,
playhead programa, Open/Scrub zahtjev, pomak za frame, prati potvrdjeni frame
programa); `qnc-program-segments` nema vise Cue/CueStep/drive_player, samo
`take_seek` i `set_playhead`; editorial application: klik na segment ide samo u
Wrap, klik u poolu i na source timeline vraca Source, Play i koraci u Wrapu ne
diraju izvorni klip. Zahtjev Open/Scrub jos nitko ne izvrsava (player programa
je sljedeci korak). Verificirano: testovi `qnc-wrap-session` (4),
`qnc-program-segments` (14), `qnc-editorial-application` (11), conformance, live:
nakon klika na segment izvorni klip, monitor i source timeline ostaju, Wrap
playhead je na kliknutom frameu. Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-24 (brza slika na klik u Wrap segmentu):
korisnik je rekao "oba koraka" nakon mjerenja: klik na Wrap segment daje sliku
na monitoru tek nakon 0,8-1,5 s (isti klip) odnosno 1,5-2,4 s (drugi klip).
Uzroci: (1) nova sesija playera prvo pripremi frame 0, a tek onda skace na
trazeni frame; (2) nakon skoka player prikazuje sliku tek kad je za taj frame
spreman i zvuk. Otkljucano samo: `crates/qnc-player-client` (sesija se
priprema na zadanom pocetnom frameu umjesto na 0), `crates/qnc-source-preview`
(otvaranje klipa na zadanom frameu), `crates/qnc-broadcast-player` (nakon skoka
slika se prikazuje cim je dekodirana; Play i dalje ceka sliku i zvuk cijelog
pocetnog spremnika), `crates/qnc-broadcast-engine` i `tools/qnc-player-runner`
samo ako se pokaze nuzno, `crates/qnc-program-segments` i
`crates/qnc-editorial-application` (Wrap otvara klip odmah na trazenom frameu),
testovi i ugovori tih modula. Nema promjene sata, A/V sinkronizacije, formi ni
layouta. Izvedeno: `Player::prepare_at` i `SourcePreview::open_at` (prva slika
sesije je trazeni frame; naredba pozivatelja ponistava zakasnjeli pocetni skok),
`CueStep::Open(clip, frame)` bez drugog skoka nakon otvaranja, u playeru
`cue_picture_ready` (slika nakon skoka ceka samo svoj dekodirani video; Play i
dalje ceka sliku i zvuk cijelog pocetnog spremnika). Runner i engine nisu dirani.
Verificirano: testovi `qnc-broadcast-player` (79, novi test slike prije zvuka),
`qnc-player-client` (17), `qnc-program-segments` (15), `qnc-source-preview`,
`qnc-editorial-application`, `cargo run -p qnc-conformance`, release build i
live log playera: skok unutar klipa do slike 0,45-0,5 s (prije 0,8-1,5 s),
novi klip bez koraka preko frame 0. Odobrenje je zatvoreno.

Zatvoreno ograniceno odobrenje 2026-09-23 (racionalni poster klik): korisnik je
trazio v5 postupak za klik na poster: klik nosi samo `clip_id`, projektni
kontekst je vec poznat, a posteri se citaju iz URI-ja zapisanih u projektnoj
bazi kroz javnu komponentu. Otkljucano samo: `crates/qnc-clip-posters`,
`crates/qnc-ingest-application` i `crates/qnc-editorial-application`. Izvedeno:
`qnc-clip-posters` pamti aktivni red i ne restartira isti posao; Ingest
application koristi taj javni poster loader umjesto privatnog thumbnail batch
puta; Editorial application vise ne resetira poster servis pri obicnom zahtjevu
za postere. Forma/layout nije diran, nema citanja globalne baze na klik, nema
novog write puta.

Zatvoreno ograniceno odobrenje 2026-09-30 (brzina filmstripa, I-frame najblizi lokaciji):
korisnik: "još uvijek nevalja filmstrip.. presporo", "uzimaš I frame najbliži lokaciji".
Otkljucano i izvedeno samo: `crates/qnc-filmstrip` (svi klipovi, i kratki, koriste skok na
kljucni kadar; prije je klip do 10 s dekodirao svaki kadar: 0,80 s po klipu prema 0,36 s) i
`crates/qnc-ffmpeg-decode` (`-copyts` uz skok na kljucni kadar: bez njega ffmpeg odbacuje
kljucni kadar prije trazene pozicije kao negativno vrijeme i uzima tek sljedeci, a zadnja
slicica kratkog klipa nije imala nijedan pa je nedostajala). Jedan prolaz kroz cijeli klip
je izmjeren i odbacen (3,08 s prema 0,50 s po duljem klipu s kartice). Probe se ne uvodi,
izgled filmstripa, baza i ugovori isti. Verificirano na izoliranoj kopiji aktivnog projekta
(kartica samo citana, stvarna baza nepromijenjena): 98 klipova, svaki 13 slicica i wave,
oko 0,47 s po klipu (prije 0,83 s); ffmpeg na 60 klipova bez ijedne slicice koja nedostaje.
Odobrenje je zatvoreno.
Live nalaz 30. 9. (korisnik: "nema wave"): zapisnik wavea u pravom radu: prvi prolaz artefakata
za svih 98 klipova daje "Wave treba finalni spremljeni media zapis", jer se zapisi zvuka dovrsavaju
usporedno i kasnije (filmstrip je gotov prije njih), a radnik je artefakte prolazio samo jednom.
Izvedeno: `tools/qnc-ingest-worker` nakon dovrsetka zapisa jos jednom pokrece artefakte; gotovi
filmstripovi se preskacu, nastaju wave koji su cekali konacni zapis. Live provjereno na novom
projektu: 98 filmstripova i 98 wavea.

Zatvoreno ograniceno odobrenje 2026-09-30 (wave u segmentima, v5): korisnik: "wave se ne prikazuje u
segmentima"; odobrio ("da") `qnc-segment-panel`, `qnc-editorial-application` i novu kockicu. Po v5
(`editorial/program_waveform.rs`, `qnc_segment_timeline.rs` `local_peaks_for_row`) wave programa se
slaze iz spremljenih waveova klipova: segmenti na A1, pokrivalice na A2, po izvornom IN/OUT; svaki Wrap
red crta svoj isjecak. Razlika od v5 (odluka korisnika): crta se izabrani kanal (`a1_source_channel`,
`a2_source_channel`), kanal 1 zadrzava v5 zamjenu sljedecom trakom kad je prazan. Izvedeno: novi javni
`crates/qnc-program-waveform` (+ `contracts/modules/program-waveform.module.json`; racun, isjecak reda,
cuvanje waveova klipova s ponovnim pokusajem za 2 s, citanje kroz javni citac artefakata, bez
dekodiranja i pisanja), `crates/qnc-program-segments` (kanal i izvorni IN/OUT segmenata i pokrivalica,
`SegmentsView::peaks`, `view_with_waves`, citac u `configure`), `crates/qnc-segment-panel` (A1/A2 trake
reda), `crates/qnc-editorial-application` (dva postojeca retka; C10 nije narastao), root Cargo.
Forma i layout nisu dirani. Sljedeci korak (odobren, nije izveden): u source timelineu klik na oznaku A1
otvara tanki izbornik desno od nje, visine audio trake, s kucicama 1-4 za kanal koji ide na A1 (jedan
kanal). Odobrenje za wave u segmentima je zatvoreno.
Izvedeno 2026-10-01 (korisnik "da, dodaj"; grana `desktop-blocks`): novi javni pasivni
`crates/qnc-channel-picker` (red kucica 1..N visine trake, jedna izabrana) i novi javni
`crates/qnc-source-channels` (broj audio kanala originala iz spremljenog zapisa, kao sto ih
broji player; bez probea i bez izmisljenog broja), oba s ugovorima modula. `qnc-timeline`
(polje `a1_channel`, namjera `ChooseA1Channel`, klik na oznaku A1 otvara izbornik samo kad ga
pozivatelj da), `qnc-source-dock` (`with_a1_channel`), `qnc-program-segments` (izbor uz
oznaceni izvor: kanal 1 za novi klip, samo postojeci kanali; Talking Head / Voice over pise ga
kao `a1_source_channel`), `qnc-program-db` (`CreateSegment.a1_source_channel`, zadano 0),
`qnc-editorial-blocks` (jedan poziv), `qnc-editorial-application` (dva retka, C10 nije
narastao), `qnc-ingest-application` (samo krak `ChooseA1Channel => None`; Ingest izbornik nema).
Forme i layout nisu dirani. Izbornik ima samo Story (grupa o); MA ga nema jer nema segmente.
Dopuna 2026-10-01 (korisnik: "istu opciju treba dodati i za A2, ne trpaj to u zajednicke kocke,
svaka svoja"): A2 ima svoje polje (`a2_channel`), svoju namjeru (`ChooseA2Channel`) i svoj izbor
u `qnc-program-segments` (`a2_choice`, `choose_a2_channel`), odvojeno od A1; crta ga ista
pasivna kocka `qnc-channel-picker`. Cover slot, Overwrite i Sync pisu izabrani kanal kao
`a2_source_channel` nove pokrivalice (`qnc-program-db` `CreateCover`, zadano 0).
Dopuna 2026-10-01 (korisnik: "A2 mora po defaultu biti 2. kanal, A1 1. kanal"; "preview prati
izbor"; glasnoca ostaje kako je snimljeno, kao v5): novi klip ima A1 na kanalu 1, A2 na kanalu 2
(klip s jednim kanalom: A2 na kanalu 1). Source preview pusta izabrani A1 kanal na izlazu 1 i
A2 na izlazu 2: `qnc-player-input` dobio polje `lead_audio_channels` (spremljeni raspored se ne
mijenja, provjera ostaje), `qnc-broadcast-engine` izlaze slaze iz `output_audio_channels()`,
`qnc-source-preview::hear_channels` ponovno priprema isti klip na potvrdjenom frameu.
Dopuna 2026-10-01 (korisnik: "postoji opcija, Enter ili nailazak na M marker"): Sync/B-roll
zapisuje pokrivalicu kad Sync dode do M markera ili OUT-a izvora, ili na Enter za vrijeme
Synca (slot se zatvara na Wrap playheadu); O i dalje zatvara slot i ceka Enter. Privremena
pokrivalica Synca pusta izvor na kanalu izabranom za A2 (`TransientCover.a2_source_channel`),
ne vise uvijek kanal 1. `qnc-program-segments`, `qnc-program-input`, `qnc-panel-focus`.
Dopuna 2026-10-01 (korisnik: Edit segmenta, "skracivanje ili produljivanje s pocetka i kraja";
odluke: markeri prate sliku, Enter sprema / Esc odustaje; v5 to nema): tab Segment dobio je gumb
Edit (`qnc-segment-panel`) koji otvara klip segmenta u source timelineu s njegovim IN/OUT; I/O ih
pomicu unutar klipa, Enter zapisuje novu `TrimSegment` operaciju (`qnc-program-db`: isti klip,
vrsta i A1 kanal; markeri unutar segmenta ostaju na istom mjestu slike, oni u odrezanom dijelu
idu sa slotom i pokrivalicom, markeri iza se pomicu za razliku duljine; Undo je vraca), Esc
odustaje (`qnc-panel-focus`). Novi javni `crates/qnc-pending-marks` (+ ugovor modula): IN/OUT s
kojim se klip otvara, izdvojeno iz `qnc-editorial-application` (virtualni short ga koristi kao
prije, sada se otvara na svom IN-u); aplikacijski sloj je manji (C10 737 -> 718).
Dopuna 2026-10-01 (korisnik: "kod edita segmenta Ctrl+I i Ctrl+O ne selektiraju IN i OUT"):
katalog je imao `select_mark_in`/`select_mark_out`, ali ih QNC kod nikad nije obradjivao (samo v5).
Novi javni `crates/qnc-source-mark-focus` (+ ugovor; v5 `select_mark_in`, `nudge_in`): Ctrl+I/O
uzme IN/OUT i playhead ide na njega, strelice ga pomicu za frame, Esc vraca tipke playheadu;
`qnc-panel-focus` ga poziva kad je fokus na source timelineu (aplikacija predaje svoj timeline,
bez novih redaka). Preview klipa ciji zapis jos ceka pozadinski dovrsetak (`NotFinal`) pise
"Podaci klipa se pripremaju..." i sam ponovno otvara klip (`qnc-player-input::is_incomplete_media`).
Dopuna 2026-10-01 (korisnik: "sve funkcije" iz kataloga koje kod nije obradjivao), po v5:
Ctrl+S `select_current_marker_slot`, Shift+S `focus_empty_slot`, Alt+strelice
`navigate_prev/next_object` (Segmenti: marker, slot ili segment; source: pocetak, IN, OUT),
Shift+I `mark_in_fit_duration` (IN na playhead, OUT za duljinu odabranog ili prvog praznog slota).
`qnc-program-segments` (modul `nav.rs`), `qnc-source-mark-focus` (`adjacent`, `fit`),
`qnc-panel-focus`. Shift+W `toggle_source_wrap` i F1 `toggle_cheatsheet` nemaju postupak ni u v5
kodu; OFF snimanje (`record_toggle`, `save_off`) nije dio ovog koraka.
Otkljucano 2026-10-01 (korisnik: "odmrzni i sredi"; "samo slika po slika, inace nije broadcast
player"): Broadcast Player za zvuk pri koraku. Audit 22.9./sada: ni QNC 13.9. i 22.9., ni v5 ni
v4 nisu imali zvuk pri koraku u pauzi (ugovor: priprema u pauzi je necujna); strelice su do
25.9. uvijek isle playheadu, a fokus panela ih je od 25.9. gasio nakon klika u poolu (vraceno).
Izvedeno: `qnc-audio-output` ima zaseban kanal preslusavanja (`AudioOutput::audition`) koji
uredaj svira samo dok su vrata reprodukcije zatvorena i odbacuje cim krene Play; red, sat i
pripremljeni Play se ne diraju. `qnc-broadcast-engine` (`DeviceSink::audition_at` iz
`cue_audio`) nakon cuea pusti tocno uzorke tog jednog framea. Program (Wrap) jos nema korak sa
zvukom. Strelice: koraci se zbrajaju od zadnjeg trazenog framea (`qnc-source-preview::step`).
Dopuna 2026-10-01 (korisnik: "dekoder treba pauzirati, ne zatvarati pa ponovo otvarati"):
`qnc-broadcast-engine` drzi dekoder otvoren; korak naprijed cita dalje, slike oko zadnjeg cuea
(25 iza) ostaju u memoriji, korak unatrag koji ipak otvara dekoder zadrzi i 25 slika prije;
svaka predana slika dobiva redni broj pri predaji (monitor vise ne odbacuje "stale").
Dopuna 2026-10-01 (korisnik: A1/A2 bez misa; "lijevo-desno izbor kanala, Enter zatvara i sprema,
gore-dolje prosirenje prikaza, prikazi preko video trake, jednim pritiskom maksimum"): katalog
`select_audio_a1` (Ctrl+1) i `select_audio_a2` (Ctrl+2) u svim presetima, tablica tipki zna 1 i 2;
dok je traka uzeta lijevo/desno biraju kanal (nacrt u izborniku), Enter sprema i odmah se cuje,
Esc odustaje, gore pokaze val te trake preko cijele video trake, dolje ga skrije. `qnc-program-segments`
(`lanes.rs`), `qnc-timeline` (`wave_over_video`, izbornik otvoren dok je traka uzeta),
`qnc-channel-picker` (`open`), `qnc-source-dock`, `qnc-editorial-blocks`, `qnc-panel-focus`.
Klik na oznaku A1/A2 salje istu akciju kao Ctrl+1/Ctrl+2 (stari prekidac izbornika uklonjen).
Enter po fokusu (korisnik: "veži funkciju entera prema fokusu"): jedan Enter radi jednu stvar
redom: uzeta traka sprema kanal; Ctrl+M marker, Edit segmenta ili Sync se potvrduju; inace
virtualni kadar samo iz source timelinea ili poola; na panelu Segmenti bez cekanja nista.
Zamijenjeno 2026-10-01 (korisnik: "ako je source timeline u fokusu tada se samo na njega odnose
sortkat komande... enter je potvrda u svim layout panelima"; "korisnik ne mora pamtiti nebrojeno
sortkatova"): tipke djeluju strogo samo na panel u fokusu. Source timeline: Enter potvrduje uzetu
traku, Edit segmenta ili Sync, inace sprema virtualni kadar; O zatvara Sync samo tamo; strelice,
gore/dolje i Esc za traku samo tamo. Segmenti: Enter potvrduje samo marker uzet s Ctrl+M. Pool:
Enter i strelice ne rade nista. `qnc-program-segments` (`confirm.rs`), `qnc-panel-focus`.

Otvoreno ograniceno odobrenje 2026-09-30 (jedna radna povrsina od kocaka, grana `desktop-blocks`):
korisnik je odbacio samostalne programe (grana `shell-standalone-apps` ostaje kao zapis: spor i trzav
prelazak, vise klikova, oko 220 MB po aplikaciji) i odredio: "sve se pretvara u kocke.. qnc-app se
pretvara u radnu povrsinu"; "sve se moze zasnivati na istom layoutu, samo neki dijelovi budu drugih
dimenzija" (Project: monitor, traka i source timeline visine 1); "idemo od pocetka.. projekt, pa dalje
redom"; "ok otkljucaj i nastavi.. to radimo u novoj grani". Grana `desktop-blocks` krece od oznake
`v-embedded-shell-2026-09-30`. Model: jedna opca ploca (`crates/qnc-board`: monitor, traka i tijelo
lijevog stupca, desni panel, dock); aplikacija je samo mjere mjesta i kocke u njima iz svog ugovora
rasporeda; izgled ostaje isti. Otkljucano za prvi korak (Project): novi `crates/qnc-board` (+ ugovor
modula), `apps/qnc-project`, `crates/qnc-project-desktop`, `crates/qnc-project-application`,
`contracts/ui/project.layout.json` (samo mjere mjesta ploce, bez promjene izgleda), `apps/qnc-app`,
nove kocke Projecta i njihovi ugovori, `tools/qnc-conformance`, root Cargo. Zatim redom Ingest, MA,
Story, uz isto odobrenje za njihove forme i ugovore rasporeda.

Zatvoreno ograniceno odobrenje 2026-10-01 (F1 pregled tipki): korisnik je trazio da F1 radi kako je
zapisano u katalogu (`toggle_cheatsheet`, F1 i Shift+/). Izvedeno: novi javni `crates/qnc-key-cheatsheet`
(+ `contracts/modules/key-cheatsheet.module.json`): prozor "Tipke" s tipkama aktivnog preseta i
oznakama akcija iz istog kataloga, za scopeove ploce; nista ne mijenja i ne zna za aplikaciju.
Poziva ga `crates/qnc-key-intents`, pa ga dobiva svaka ploca koja cita tipke, bez rasta formi.
Ingest scope u katalogu nema `toggle_cheatsheet` (Ingest dio kataloga je zamrznut), pa u Ingestu
F1 za sada ne otvara pregled. Odobrenje je zatvoreno.
Dopuna 2026-10-01 (korisnik: "help bi koncipirao da najprije objasnim sto koja komanda + tipka znaci...
zatim tablice po grupama ili po akciji"): katalog tipki dobio je blok `help` (pravila: tipka radi odmah,
Ctrl+ uzima kontrolu nad necim sto postoji, Shift+ dodaje novo, Alt+strelice skacu po objektima,
strelice, Enter, Esc, Delete, fokus; grupe akcija). `crates/qnc-keyboard-shortcut` ga cita
(`ShortcutCatalog.help`, nepoznata akcija u grupi je greska) i daje `chord_help` (svaka tipka jednom,
strelice i brojke citljivo). Prozor "Tipke" prvo pokazuje pravila, zatim tablice "Po grupama" ili
"Po akciji" (abecedno). Tekstovi su u katalogu, ne u kodu.
Popravak 2026-10-01 (korisnik: "playback programa zastajkuje na prelasku s pokrivalice na pokrivalicu"):
`player.log` pokazuje `audio_underrun` tocno na rezovima pokrivalica (frame 148, 302). Zvuk programa
otvarao je dekoder pokrivalice tek kad je trebao njezin prvi frame (slika ga je vec otvarala unaprijed),
pa se audio red ispraznio. `crates/qnc-broadcast-engine` (`program.rs`): traka zvuka je kanal klipa s
jednim pomakom program-izvor (isti kanal s drugog mjesta je druga traka), a trake stavki koje pocinju
u sljedece 2 s otvaraju se unaprijed; trake koje vise nitko ne treba se zatvaraju. Sat, red i Play se
ne mijenjaju. Test `a_cover_has_its_own_lane_so_its_sound_opens_before_the_cut`.
Dopuna 2026-10-01 (korisnik: "sve bi trebalo biti zapisano u bazi"; "ok, napravi"): provjera baze aktivnog
projekta (samo citanje, 98 klipova): pocetni TC originala iz XML-a = TC probea originala = TC proxyja; kraj
iz XML tablice = pocetak + frameovi (bez prekida); XML `Duration` = `clips.duration_frames`. Duljina se ne
pise kao TC, racuna se. `crates/qnc-source-timecode`: Sony vrijednost na 50p (`halfStep`) nosi drugi
frame u gornjem bitu bajta sati (prije se ignorirao: krajevi su bili frame krivo, poceci na ovoj kartici
ga nemaju); `end` (TC zadnjeg zapisanog framea), `continuous()`, `proxy_matches()`. `crates/qnc-program-playlist`:
svaki izvor ravne playliste nosi `source_timecode_start` (TC framea 0 izvora iz baze; raspon ostaje u
frameovima). `crates/qnc-program-input` ga puni iz spremljenog zapisa i odbija program kad slika ide s
proxyja ciji je pocetni TC razlicit od originala (kontrolirana greska, bez pogadanja pomaka).
Testni literali u `qnc-player-input` i `qnc-broadcast-engine` dobili su novo polje. Export jos nije izveden
(ceka odluku o formatu).

Promjena pravila 2026-10-01 (korisnik: "dugme Close project mora zatvoriti projekt i vratiti na projekt
formu kako bi korisnik mogao kreirati novi ili otvoriti drugi projekt"; "desktop launch bar nije vezan na
projektne postavke"): zamjenjuje recenicu iz odjeljka 3 da shell nakon zatvaranja ne smije prebacivati
tab. Close project i dalje zatvara samo kroz `qnc-project-close` (nista se ne brise, nijedna povrsina se
ne unistava, workflow se ne pokrece), a zatim desktop prikaze aplikaciju prve prioritetne grupe
(Project), kao v5 `close_project` (`Screen::Project`). Launch bar (v5 `workspace.tabs`): bez aktivnog
projekta samo prva grupa, s aktivnim projektom samo aplikacije njegova templatea, redom slijeda iz baze.
Audit: u QNC-u launch bar nikad nije bio vezan na projekt (13.9. `0ab6021` i kasnije uvijek svi
registrirani), postojalo je samo u v5. Izvedeno: novi javni `crates/qnc-desktop-tabs` (+ ugovor;
`qnc-active-project-read` + `qnc-application-sequence`, cita bazu ponovno svake sekunde, nista ne pamti),
`crates/qnc-desktop` (bar iz tog bloka; prikazana aplikacija koja vise nije u projektu ustupa prvoj;
prijelaz na sljedecu grupu najprije ponovno cita bazu), test zatvaranja promijenjen prema novom pravilu.
Kao v5 (`footer_project_only`): bez aktivnog projekta ni dugme Close project se ne prikazuje
(`crates/qnc-shell-footer` `project_open`).
Project nije diran.
Popravak 2026-10-01 (korisnik: "nakon zatvaranja i ponovnog otvaranja projekta u Ingestu se vise ne
prikazuju posteri s kartice"): Ingest je pri zatvaranju projekta ispraznio popis klipova, ali je
`qnc-clip-posters` i dalje pamtio koje je postere ucitao, pa ih za ponovno otvoreni projekt nije trazio.
`crates/qnc-ingest-application/src/settings.rs`: kad se popis klipova prazni (nema projekta ili drugi
projekt) poster loader se resetira (`posters.reset()`), kao sto vec radi Editorial. Bez novih redaka.
Popravak 2026-10-01 (korisnik: "ne radi play na source timeline niti se playhead moze premjestiti
klikom"): nakon Synca (ili Wrapa) player drzi program, a klik na source timeline treba vratiti klip.
`qnc-source-preview::open_at` je odbijao ponovno otvaranje jer je isti klip vec bio prikazan; sada dok
player drzi program klip se otvara ponovno. Mjerenje iz `player-timing` (isti dan): citanje iz baze
23-65 ms, pokretanje procesa playera 0,7-1,9 s, do "spreman" ukupno 1,5-2,5 s.
Ubrzanje 2026-10-01 (korisnik: "i dalje se sporo pokrece play i sync/b-roll"; "ok nastavi"): mjerenje
(`player-stage`, `player-open-timing`, `player-timing`) pokazuje: baza 22 ms (Sync 230 ms), proces ~70 ms,
GPU pretvarac 140-200 ms, audio uredaj ~450-500 ms, do "spreman" jos 0,45-1,1 s. Audio: Windows je pri
svakom otvaranju popisivao sve formate uredaja (~250 ms). `crates/qnc-audio-output`: kad uredaj vec radi
u trazenom formatu (kanali, frekvencija, f32) otvara se takav (~1 ms provjere); svaki drugi format i dalje
prolazi punu provjeru bez downmixa. `crates/qnc-broadcast-engine`: GPU pretvarac se priprema usporedno s
otvaranjem audio uredaja. Ponasanje playera se ne mijenja.
Dopuna 2026-10-01 (korisnik: "prilikom kreiranja pokrivalica treba kreirati i poster da u B-roll tabu
imamo poster"; "B-roll ne bi smjela ici u virtual klip nego u B-roll tab"; "ako je ukljucen Sync/B-roll
virtualni klip se ne kreira, samo B-roll"): nalaz u bazi: nakon Synca koji je zapisao M marker, sljedeci
Enter na source timelineu spremio je i short istog IN-a do kraja klipa. Izvedeno: `qnc-program-segments`
pamti IN od kojeg je pokrivalica nastala (`marks_used_by_cover`); dok je on prikazan, Enter ne sprema
virtualni klip. Poster pokrivalice: uhvacena IN slika (`qnc-virtual-short-stills::store_poster`) pod
`products/b_roll_virtual_clips/<shot>/in.jpg`, upisana kroz `qnc-virtual-shots::publish_poster_now`
(zapis slika sada dopusta i klasu `b_roll`). Novi javni `crates/qnc-shot-stills` (+ ugovor) slaze
slike shorta i postere pokrivalica; `qnc-editorial-application` ga samo poziva (C10 718 -> 709).

Otkljucano i izvedeno 2026-10-02 (korisnik: "odmrzni i rijesi" na nalaze audita grane `desktop-blocks`):
(1) Ingest filter `Novi / Sve`: `qnc-ingest-catalog::apply_catalog_rows` je pri ponovnom ucitavanju kataloga
zadrzavao odabir, ali je svaki klip oznacio vidjenim, pa je "Novi" nakon Selecta ostajao prazan. Sada
zadrzava i oznaku "nov" koju je dao tekuci Select (`CatalogClipItem::catalog_new`, test). Oznaka
`previously_seen: true` za klip iz baze ostaje ispravna (klip koji je bio u bazi prije Selecta nije nov).
(2) Glava poola je javna kocka: `qnc-media-pool-head` dobio je red iz layout ugovora (`show_row`, `tab`,
`RowStyle`, `RowTab`, `RowClick`): tabovi lijevo, naredbe desno, oznake iz ugovora, odabrani tab iz viewa,
klik vraca indeks. `qnc-ingest-blocks` i `qnc-editorial-blocks` ga koriste; njihove privatne kopije
`text_tab` i `small_button` su uklonjene (isti izgled). Ingest tabovi rade kao v5 (`ingest/mod.rs`
`dispatch_pool_head`): klik samo bira tab, tijelo ostaje preglednik lokacija. `contracts/ui/ingest.layout.json`
tabovi dobili `action_id` (`ingest_tab_all`, `ingest_tab_virtual`, upisani u katalog tipki bez tipke),
`qnc-ingest-layout` (`tabs_left` kao naredbe, `tab_gap`), `qnc-ingest-application` (`pool_tab`, C10 1714).
Popravak 2026-10-02 (korisnik: "novi projekt, nakon Uvezi na Storyju bljeska monitor, a izmedu bljeskanja
pise player input: NotFinal"; "to je bilo dok je pravio filmstrip i wave"): klip je bio uvezen dok je
pozadinski dovrsetak zapisa jos radio (dok filmstrip/wave citaju karticu, dovrsava se samo klip koji
preview trazi). Preview ga svake sekunde ponovno otvara; svaki pokusaj je praznio monitor pa pokazivao
sirovu gresku. `crates/qnc-source-preview`: dok zapis prikazanog klipa ceka dovrsetak, monitor i status
stalno pokazuju "Podaci klipa se pripremaju..." (i dok se novi pokusaj priprema); kad je zapis gotov,
poruka nestaje. Ponovni pokusaj i granica od 3 minute ostaju isti.
Popravak 2026-10-02 (korisnik: "u play segmentu u nekim situacijama pred kraj pocne zastajkivati klip prije
prelaska na novi klip"): `player.log` dvaput `audio_underrun` na frameu 2877, a u prici na 2878 pocinje
pokrivalica drugog klipa. Zvuk pokrivalice vec se otvara 2 s unaprijed, ali dekoder slike sljedeceg
klipa otvarao se tek kad je predcitanje (~0,5 s) doslo do reza; pokretanje ffmpega i skok traju toliko,
pa je player ostao bez slike. `crates/qnc-broadcast-engine` (`program.rs`): dekoder slike sljedeceg reza
na drugi izvor otvara se 2 s unaprijed (`next_picture_cut`, `upcoming`) i preuzima kad rez dode; rez koji
nastavlja isti izvor ne treba novi dekoder. Sat, red i Play se ne mijenjaju. Test.
Popravak 2026-10-02 (isti zastoj na 2877, dijagnostika `program-audio open`): pokrivalica 2878-3088 prelazi
granicu segmenta na 2961, pa je u playlisti u dva dijela s istom audio trakom (izvor od 19 i od 102).
Predotvaranje traka otvaralo ju je za oba dijela, svaki je ponistavao drugi (oko 50 otvaranja ffmpega u
sekundi), pa zvuk pokrivalice nikad nije bio spreman na rezu. `qnc-broadcast-engine` (`next_lanes`): svaka
traka otvara se jednom, na prvom dijelu koji je koristi. Test.
Audit playera 2026-10-02 (korisnik: "stabiliziraj player, napravi dubinski audit"; "nesigurno od 22. 9."):
pregledane sve izmjene playera od 30. 9. (24 commita), `player.log` (zastoji po danima, prikazane slike) i svi
testovi playera. Popravljeno: (1) traka zvuka pokrivalice podijeljene granicom segmenta otvarala se 50 puta
u sekundi (regresija iz `ee4d5e6`, popravak `ae18a0b`); (2) kod klipa svaka predana slika trosila je novi
redni broj (od `d593471`), pa je monitor brojao svaku drugu kao preskocenu i mjerenje glatkoce nije
vrijedilo; sada novi broj dobiva samo slika ponovno pokazana nakon koraka unatrag; (3) dijagnostika
"pending" pise se jednom po frameu, a ponovno otvaranje dekodera slike klipa biljezi se (`player-video
reopen`). Otvoreno: automatska provjera playa (§8.3 tocka 6) jos ne postoji.
Stabilizacija playera 2026-10-02 (korisnik: "da" na plan audita): novi alat `tools/qnc-player-acceptance`
(§8.3 tocka 6): na izoliranoj kopiji aktivnog projekta (odbija projekt izvan zadanog korijena) pusta klipove i
pricu kroz isti `qnc-source-preview` kao forme i mjeri spremnost, Play do prve slike, udio slika koje stignu do
monitora, najvecu rupu, trzaje (> 3 slike), zastoje i ponovna otvaranja dekodera iz loga; izvjestaj
`player-acceptance.json`. Alat je nasao i potvrdio tri uzroka nesigurnog playa: (1) program je imao premalo
medjuspremnika za slike (prefetch + 4), dekoder je radio u realnom vremenu; sada 2 x prefetch + 8; (2) redni broj
slike programa davao se pri nastanku, crne Off slike dobile su veci broj od slika pokrivalice prikazanih prije
njih, monitor ih je odbacivao kao stare (~0,45 s smrznuta slika); sada se daje pri predaji; (3) dekoder koji vise
ne treba gasio se u niti reprodukcije (ffmpeg kill + join do 0,9 s), sto je praznilo zvuk i zaustavljalo player
("explicit preparation required"); sada se gasi u pozadini (`retire`). Prije: program 97-98 % slika, rupe
440-500 ms, tvrdi zastoj u ~1/6 pustanja; poslije: 8/8 bez zastoja, 100 % slika, klipovi 2002/2679/2676 PASS.
Uz to dijagnostika razloga zastoja dekodera programa i trajanja otvaranja dekodera.
Dopuna 2026-10-03 (korisnik nakon live probe: "sad je jos gore"; log: dekoder slike radi tocno u realnom
vremenu, bez rezerve): zaliha slika unaprijed dopunjavala se samo kad trazena slika nedostaje, a kad je bila
spremna vracala se bez dopune, pa je dekoder radio u paketima od 4 slike tocno na vrijeme; u pravoj aplikaciji
(GPU dijeli i sucelje) to je trzalo. `crates/qnc-broadcast-engine` (klip `fill_conversion_queue`, program
`fill`): zaliha se dopunjuje pri svakoj predanoj slici. Provjera `qnc-player-acceptance`: cekanja na sliku
tijekom playa ~240 -> ~19 po pustanju (vecinom pri pripremi), program 3/4 i klipovi PASS (jedan trzaj 77 ms
na hladnoj kartici, priprema 3 s samo prvi put).
Dopuna 2026-10-03 ("nastavi"): `qnc-player-acceptance` mjeri i pomak zvuka i slike: za svaku sliku koja stigne
na monitor racuna frame koji se u tom trenutku cuje (iz `AV_A`: uzorak koji audio uredaj svira i kada);
prolaz trazi medijan unutar 1 slike i da slika nigdje ne kasni za zvukom vise od 2 slike (1. percentil).
Izmjereno: klipovi 2002/2679 i program od 2513 medijan -0,2 slike (slika najvise 0,8 slike iza zvuka).
Program uz istodobni rad druge aplikacije na istoj kartici ima 1-6 kratkih trzaja (44-62 ms), sam prolazi.

Otvoreno ograniceno odobrenje 2026-10-03 (odziv koraka i sucelja, pa HDMI): korisnik: "nisam zadovoljan s
trenutnim rjesenjem rada u Storyju.. editiranje IN/OUT i M je djelovalo bolje, pomak za slicice naprijed nazad
je reagirao na svaki klik"; "odkljucaj sve sto treba". Audit `player.log` (1. 10.): korak strelicom do nove
slike medijan 859 ms, najgore 2,9 s; pet pritisaka dalo je jedan frame pomaka. QNC je news cutter: laptop na
terenu i HDMI monitor, bez 4K (odluka korisnika); HDMI izlaz je postojao samo na sporednim granama 17. 9.
(`wip/gpu-shared-texture-attempt`, `gpu/from-2026-09-14`), nikad na mainu. Otkljucano: `qnc-broadcast-player`,
`qnc-broadcast-engine`, `qnc-source-preview`, `qnc-player-client`, `qnc-desktop`, `qnc-desktop-tabs`,
`qnc-program-segments`, `qnc-program-waveform`, `qnc-db-broker`, `qnc-player-acceptance`, zatim za HDMI novi
`qnc-program-output`, `tools/qnc-player-runner`, `qnc-video-output`, root Cargo. Forme i layout se ne diraju.
Redoslijed: 1) mjerenje i korak bez punog skoka, 2) naredbe playeru i citanje baze izvan niti sucelja,
3) HDMI programski izlaz kao samostalna kocka (izbor vanjskog ekrana, nije obavezan, ne drzi sat).
Izvedeno 1): `qnc-player-acceptance --steps NAME` mjeri korak do slike (10 naprijed, 10 nazad) i niz od 10
brzih pritisaka; alat cita log tamo gdje ga player stvarno pise (`locate_qnc_root`: izolirana kopija bez
datoteka korijena pise u QNC iz kojeg je player izgraden; prije je alat citao prazan log i zastoje i A/V nije
vidio). Uzrok sporih koraka u `qnc-broadcast-engine`: priprema u pauzi trazi 25 slika unaprijed, slike koje
nisu stale u spremnike su izbacivane, interno ponovno otvaranje dekodera na kraju pripreme pomicalo je zadnji
trazeni frame i brisalo zadrzane slike, pa je sljedeci korak naprijed bio "korak unatrag" i otvarao dekoder
25 slika ranije (~400 ms, unatrag ~1 s). Sada: interno otvaranje nije cue (zadnji trazeni frame i zadrzane
slike prije njega ostaju, dobivaju novu generaciju kad se ponovno pokazu), slike koje je dekoder vec napravio
unaprijed se ne izbacuju, spremnika ima i za njih (+ prefetch). Izmjereno na izoliranoj kopiji: korak do
slike prije medijan 38 ms / najgore 1039 ms, 6 od 10 slika u nizu; poslije 34-36 ms / najgore 52 ms, 10 od 10,
bez ijednog ponovnog otvaranja dekodera; klipovi 2002/2679 i program od 2513 100 % slika, bez zastoja, A/V
-0,1 do -0,2 slike.
Izvedeno 2): nijedno citanje baze ni dekodiranje filmstripa na niti sucelja. Naredbe playeru vec idu iz
niti klijenta (poziv iz forme 5-30 us), pa tu nema promjene. `qnc-desktop-tabs::Watcher` cita launch bar u
pozadini svake sekunde (desktop ga samo preuzima; odmah se cita samo pri pokretanju, Close project i
prijelazu na sljedecu grupu; nit se pri gasenju ceka). `qnc-program-segments` ponovno cita pricu nakon upisa
u pozadini (`reading`, jos jedno citanje ako je trazeno usred citanja), `qnc-program-waveform` cita valove
klipova programa u pozadini. `qnc-timeline-assets::TimelineAssetReader::request`/`take_loaded`: filmstrip i
val klipa citaju se i dekodiraju u pozadini; `qnc-source-preview` pri kliku na klip pokaze sto ima u
memoriji i preuzme procitano u `poll`; `qnc-content-artifacts::poll_shown` (Ingest: prikazani klip se cita
ponovno dok mu artefakti nedostaju) i citac se slaze jednom po projektu (prije pri svakom citanju, s
citanjem veze projekta). Forme se osvjezavaju svakih 15 ms dok citanje traje. `qnc-desktop` biljezi svaki
frame sucelja dulji od 25 ms (`ui-frame-slow`, samo uz dijagnostiku playera). C10 granice nisu podignute
(Ingest application 1714, Editorial application 709). Provjereno: testovi `qnc-desktop`, `qnc-desktop-tabs`,
`qnc-program-segments`, `qnc-program-waveform`, `qnc-timeline-assets`, `qnc-source-preview`,
`qnc-content-artifacts`, `qnc-ingest-application`, conformance, `qnc-player-acceptance` (klipovi i program
100 %, bez zastoja, koraci 37-39 ms medijan, najgore 58 ms, 10 od 10).
Izvedeno 3): novi javni `crates/qnc-program-output` (+ `contracts/modules/program-output.module.json`): kad je
spojen ekran osim glavnog (laptop + HDMI), na najvecem takvom ekranu je prozor bez okvira tocno preko cijelog
ekrana sa slikom koju pokazuje preview monitor, crno kad slike nema; bez drugog ekrana nema prozora ni greske,
ekrani se ponovno gledaju svake 2 s (ukljucen ili iskljucen monitor). `qnc-monitor` ponudi sliku koju crta
(`offer`, kopija samo kad izlaz radi), `qnc-desktop` jednom po frameu pozove `show` nakon formi; forme i layout
nisu dirani. Ne drzi sat, ne dekodira, ne cita bazu, ne zna aplikaciju; prati potvrdjene slike monitora u
ritmu crtanja desktopa (izlaz na sat playera, SDI/NDI i genlock su kasniji adapter istog ugovora). Popis
ekrana ide kroz adapter po OS-u: Windows `EnumDisplayMonitors`; ostali OS-ovi daju kontroliranu poruku da
popis jos nije podrzan i izlaz ostaje iskljucen (bez nagadjanja). Grane 17. 9. nisu prenesene (GPU/DMA put
koji je zamrzavao sucelje); uzet je samo izbor ekrana. Provjereno: testovi izbora ekrana, probni program
`cargo run -p qnc-program-output --example on_main_screen` (bez drugog ekrana na glavnom ekranu: prozor
1920x1080 preko cijelog ekrana, slika razvucena tocno), conformance. Nije provjereno: pravi HDMI monitor
(nije bio spojen), dva prozora uz vsync (moguce manje slika u sekundi; `ui-frame-slow` u logu to pokazuje),
slika je pola rezolucije previewa (960x540 za 1080 izvor).
Audit 2026-10-08 (korisnik: "napravi audit da budemo sigurni do kuda smo dosli"): grana `desktop-blocks` na
`ec183fa`, bez nespremljenih izmjena; gradnja workspacea, conformance i `qnc-player-acceptance` na izoliranoj
kopiji (klipovi 2002/2679 i program od 2513 100 % slika, bez zastoja, A/V -0,1 do -0,2 slike; koraci 31-36 ms
medijan, 10 od 10; program na hladnoj kartici jednom 2 trzaja od 74 ms na rezu pokrivalice, ponovljeno 3/3
PASS). Popravljeno (korisnik "da"): zastarjeli test `qnc-source-bindings` (probni podaci bez obaveznog
`analyze_duration_us`) i probni primjer `qnc-editorial-shell/examples/editorial_demo.rs` (nova polja kartice
od 23. 9.: `top_right_marker`, `overlay_label`, kao MA/Story bez njih). Testovi: 1081 prolazi, 0 pada.
Nije provjereno: HDMI na pravom monitoru, Sync/B-roll uzivo, Story kroz sucelje.
Live 2026-10-08 (HDMI spojen): HDMI ekran 1920x1080 na 100 % desno od laptopa (125 %); prozor izlaza dobio je
velicinu po mjerilu laptopa i nije pokrivao cijeli HDMI ekran. Ispravak u `qnc-program-output`: prozor se
otvori unutar vanjskog ekrana i odmah prebaci u puni ekran (`ViewportCommand::Fullscreen`), pa OS sam daje
cijeli ekran bez obzira na mjerilo. Provjereno snimkom oba ekrana: probni program pokriva cijeli HDMI ekran,
aplikacija na tabu Project daje crni HDMI ekran (nema preview monitora). Reprodukcija na HDMI kroz Ingest i
Story ceka probu korisnika.
Live 2026-10-08 (korisnik: "video nije fluidan ni na monitoru ni na HDMI"): log probe s HDMI-jem 107 frameova
sucelja od 25-56 ms tijekom playa (izvor 50p treba 20 ms; 3. 10. bez HDMI-ja 7). Uzrok: dva prozora u istom
prolazu, svaki ceka vsync svog ekrana. `qnc-program-output::wgpu_configuration` (koristi ga `apps/qnc-app`):
na Windowsima DX12 i mailbox (predaja slike bez cekanja i bez kidanja), ostali OS zadano. Uz to naredba za
puni ekran salje se jednom po ekranu (svaki frame ju je slao i bez vsynca aplikacija je u mirovanju trosila
63 % jezgre, sada 5,5 %).
Live 2026-10-08 (korisnik: "ima kao duhove"; "putanja vertikalne linije nije fluidna, iza sebe ostavlja
duhove, ili ima kao nepravilan field"; "HDMI moze biti i50, i60, p60, p24 ili neki drugi"; "da" na vlastiti
proces izlaza): crtanje drugog prozora u prolazu sucelja (i s vsyncom i s mailboxom) davalo je dvostruke i
preskocene slike. Mailbox za sucelje je vracen na zadano. Novi `tools/qnc-program-output-host`: proces s
prozorom preko cijelog vanjskog ekrana; vlastita nit ceka osvjezavanje bas tog ekrana (Windows DXGI
`WaitForVBlank`, inace FIFO), uzme najnoviju sliku iz playerove zajednicke memorije (`qnc-player-frame-transport`,
samo citanje) i preda je (mailbox uz cekanje osvjezavanja). Sucelje ne nosi slike: `qnc-player-client` i
`qnc-source-preview` uz sliku nose adresu frame mape, `qnc-monitor` je nudi, `qnc-program-output` pokrece proces
kad postoji drugi ekran, salje mu `map <put>`/`clear` na stdin i gasi ga (proces zavrsava i kad mu stdin
zatvori nestala aplikacija). `qnc-program-output::screens` cita i osvjezavanje i isprepletenost moda (Windows
`EnumDisplaySettings`); proces u log pise `mode screen_hz=.. source_fps=.. even|uneven`, mod ekrana nikad ne
mijenja. Izmjereno probnim izvorom 50p na HDMI 50p (`examples/output_test`): prije 257 osvjezavanja i 211 novih
slika u 5 s (petlja kroz winit propustala osvjezavanja), sada 248/248 i 249/249, nijedna preskocena.
Ceka probu korisnika s pravim klipom.
Live 2026-10-08 (korisnik: "i dalje su duhovi i kao krivi field, iako znam da u progresivu nema fielda"): log
pokazuje da HDMI izlaz dobiva svaku sliku (251/251 u 5 s), a izvor 2002 je progresivni 50p (XML kamere i
probe); `qnc-player-acceptance` sada broji i slike izvan reda (0 unatrag, 0 ponovljenih). Uzrok je u
`qnc-gpu-raster`: smanjivanje 1080 -> 540 za preview uzimalo je svaki drugi redak i stupac (namjerno, da ne
mijesa fieldove), pa su tanke crte i rubovi u pokretu skakali izmedu redaka kao krivi field. Sada se za
progresivni izvor uzima srednja vrijednost bloka 2x2; isprepleteni izvor i dalje ide bez mijesanja redaka
(pretvarac ga i inace odbija). Test na GPU-u (`half_size_averages_a_progressive_frame`), pretvorba ~6 ms po
slici, provjera playera prolazi.
Live 2026-10-08 (korisnik: "svenk ne valja"): u pravom radu HDMI izlaz je povremeno preskakao 5-15 slika u 5 s
(player preda dvije slike unutar jednog osvjezavanja, a izlaz je uzimao samo najnoviju) i nit crtanja je
kasnila na osvjezavanje (do 43 u 5 s). `tools/qnc-program-output-host`: slike redom, jedna po osvjezavanju,
iz prstena od 8 mjesta (`qnc-player-frame-transport::LatestFrameReader::backlog`); na najnoviju skace tek kad
zaostane vise od 3 slike; nit crtanja ima najvisi prioritet (Windows). `qnc-player-acceptance --hdmi`
pokrece izlaz uz pravi player i zbraja njegove brojke. Izmjereno (klipovi 1492 i 2002 po 15 s): tijekom
reprodukcije 251 osvjezavanje, 251 nova slika, 0 preskocenih, 0 zakasnjelih u svakih 5 s.
Live 2026-10-08 (korisnik usporedio ffplayom na HDMI-ju: "original je puno bolji"): duhovi i "krivi field"
na svenku su u proxy datoteci kamere (2002S03.MP4), ne u QNC-u. Original (H.264 4:2:2 10-bit 1080p50) kroz
player je bio na granici laptopa (i5-1135G7): kroz cijev dekodera 8 MB po slici. `qnc-media-decode`
`DecodeRequest::output_pixel_format` (samo `yuv420p`, ista velicina; izbor dekodera i dalje po spremljenom
formatu), `qnc-ffmpeg-decode` tada izricito pretvara (`scale,format`), `qnc-broadcast-engine` `InputPlan`
trazi 4:2:0 kad je izvor tezi i pretvarac dobiva taj format (opis izvora ostaje iz baze). Izmjereno na
izoliranoj kopiji s `playback.input = original`, 2002 od frame 1500 s HDMI-jem: 3/3 prolaza, 100 %, rupa
najvise 45 ms, HDMI 0 preskocenih (prije 1 od 3 s rupom 191 ms). Program (prica) s originalima jos ne
prolazi: 1 od 4, 1-5 trzaja 78-154 ms oko rezova (dva dekodera originala istodobno). Stvarni projekt i dalje
pusta proxy; izbor je postavka projekta `playback.input`.
Otkljucano i izvedeno 2026-10-08 (korisnik: "ako bi koristio original" i "ok" na prijedlog: popravak kopije
originala): opcija Project postavki "Ingest media = Original" vec je kopirala original u projekt, ali player
je kopiju odbijao (baza nije znala da je kopija isti medij kao original), pa se klip nije pustao. Izvedeno:
`qnc-content-store` stupac `clips.imported_copy_of` (`original` | `proxy` | NULL; vlasnik ga dodaje pri
otvaranju za pisanje, stari katalog ga cita kao NULL), `FinishImport.copy_of`, `StoredClip.imported_copy_of`;
`qnc-work-settings::project_media_source_uri` (mapa projekta kao izvor medija:
`qnc://<okruzenje>[/<authority>]/source/project-<project_id>`); `qnc-ingest-import-worker` kopiju zapisuje kao
`.../source/project-<id>/file/<original|proxy>/<naziv>` s oznakom kopije, naziv datoteke je citljiv s
ekstenzijom (prije `Mironik_202002_2EMXF`); `qnc-player-input` za kopiju koristi spremljeni zapis te
reprezentacije s adresom kopije (bez probea; link i nepoznata adresa kao prije); `qnc-source-preview` playeru
daje i vezu mape projekta ove masine (bez nje ne radi samo kopirani medij). Izmjereno na izoliranoj kopiji
(karticu samo citam, stvarni projekt nije diran): kopiranje s kartice 28 MB/s (sat materijala oko 13-14 min);
uvoz 9 originala price (4,6 GB) kroz `qnc-ingest-worker`; s nedostupnom karticom prica od 2513 prolazi 7 od 8
(jednom trzaj 69 ms), s kartice 0 od 4 (trzaji 60-150 ms); 2002 od 1500 PASS, koraci 36 ms. Moguce je da su
svjeze kopirane datoteke bile u memoriji sustava. Nije izvedeno: izrada montaznog proxyja, NVDEC adapter,
filmstrip/wave/probe i dalje citaju medij s kartice dok ih rade (jednom nakon Odaberi).
Popravak 2026-10-08 (korisnik: "na HDMI slika nije fluidna"; "da" na istragu potrosnje aplikacije u
mirovanju): aplikacija na Storyju u pauzi trosila je 57 % jezgre i crtala 61 puta u sekundi. Uzrok:
`qnc-player-client` u pauzi pita player za stanje jednom po slici, svaki odgovor ima novi `sequence`, a
`publish` je to brojao kao promjenu i budio sucelje. Sada budi samo novo stanje (`same_state`: sesija,
generacija i dogadjaji), test. Nova dijagnostika `qnc-desktop` `ui-paint-rate` (jednom u sekundi: broj
crtanja i tko ga je trazio, samo uz dijagnostiku playera). Poslije: aplikacija u mirovanju 2 crtanja u
sekundi i oko 2 % jezgre. Izmjereno (izolirana kopija, prica od 2513, 20 s, aplikacija otvorena), prije ->
poslije: proxy s HDMI-jem 90-94 % slika i 42-65 preskocenih na HDMI-ju -> 99,4-100 % i 0-6; proxy bez
HDMI-ja 95-97 % -> 100 %; original s diska s HDMI-jem 60-66 % -> 95-96 %; original s diska bez HDMI-ja
71-76 % -> 98-99 %. Original s HDMI-jem jos ima 7-11 kratkih trzaja na rezovima pokrivalica.
Popravak 2026-10-08 (korisnik "ok" na smanjenje tereta na rezovima): `qnc-broadcast-engine` (`program.rs`
`fill`) trazio je sljedeci rez od playheada, a dekodiranje je ~0,5 s ispred njega: kad je dekodiranje proslo
rez i preuzelo dekoder otvoren unaprijed, isti rez je jos bio ispred playheada pa se isti dekoder (ffmpeg,
skok na isto mjesto) otvarao jos jednom tocno na svakom rezu. Sada se trazi od mjesta dekodiranja; test.
`qnc-player-acceptance` ispisuje svaki trzaj (vrijeme, rupa, frame). Izmjereno (izolirana kopija, prica od
2513, 20 s, aplikacija otvorena na Projectu): proxy bez HDMI-ja 3/3 PASS, rupe do 30 ms; proxy s HDMI-jem
1/3 PASS, ostali 1-5 trzaja (98,6-98,7 %); original s diska s HDMI-jem 96-98 %, 2-6 trzaja; original bez
HDMI-ja 98-99 %, 0-4 trzaja. Preostali trzaji padaju kad se dekoder sljedeceg reza otvara unaprijed
(skok na kljucni kadar i dekodiranje do trazene slike).
Popravak 2026-10-08 (korisnik: "prekida i zastajkuje... sada nemamo ni play cijelog projekta"): play price
u aplikaciji s HDMI-jem stajao je na praznom zvuku (`audio underrun`, pa "explicit preparation required").
Mjereno po procesu tijekom playa: sucelje Storyja 100-138 % jezgre, player 65-97 %, ffmpeg slike ~100 %,
HDMI izlaz 30-42 %; zvuk se cita iz originala na kartici. Bez HDMI-ja (20:36) cijela prica je svirala.
Izvedeno: `qnc-broadcast-engine` (`program.rs` `fill_lane`): traka zvuka uzima sve sto je dekoder vec
pripremio, do 2 s unaprijed (prije samo zvuk tekuce slike, najvise 2 paketa), pa kratki zastoj kartice ili
procesora ne prazni zvuk; kraj izvora tijekom citanja unaprijed nije greska; `output.rs` traka smije drzati
4 s (prije 2 s). `tools/qnc-player-runner` na Windowsima postavlja playeru prioritet AboveNormal, a
`qnc-media-decode` dekoder pokrece s prioritetom procesa koji ga pokrece (player iznad sucelja; pozadinski
radnici normalno; Linux i macOS bez promjene, tamo obican korisnik ne smije dici prioritet). `qnc-ui-kit`
monitor istu sliku ne kopira niti ne salje ponovno grafickoj. Dijagnostika: `qnc-desktop` `ui-paint-rate`
(i `ui_cpu_ms`), `qnc-editorial-blocks` `ui-block-ms` po dijelu ploce. Izmjereno (izolirana kopija stvarnog
projekta, proxy i zvuk s kartice, cijela prica 5363 slike, HDMI): do kraja, 99,8 % slika, HDMI 4
preskocene, 3 trzaja 64-94 ms; uz dvije umjetno zauzete jezgre do kraja, 99,6 %. Ceka probu u aplikaciji.
Popravak 2026-10-08 (korisnik "moze" na blazu varijantu: sucelje dok svira crta samo uz novu sliku):
`qnc-editorial-desktop` i `qnc-ingest-desktop` dok player svira ne traze novo crtanje zbog promjene koju je
crtanje naslo (pomaknut playhead), nego cekaju sljedecu sliku playera; `qnc-player-client` dok svira ne budi
sucelje za novo stanje koje je samo pomak pozicije (pauza, kraj, greska i dalje bude odmah). Live (Story,
HDMI, cijela prica 108 s, stvarni projekt, proxy): HDMI 251 od 251 osvjezavanja u gotovo svakih 5 s,
ukupno preskoceno oko 11 slika (prije 32-67 u svakih 5 s druge polovice), bez prekida zvuka; sucelje
~417 ms procesora u sekundi (prije ~605).
Izmjena rasporeda 2026-10-08 (korisnik je izricito trazio i potvrdio u zasticenoj kopiji): Story (grupa o)
- ispod monitora uklonjeni `[`, `]`, `B` i "Export HI-res"; "Export HI-res" je u traci panela Segmenti na
mjestu "Sync/B-roll" (jos neaktivan, export ne postoji); u source docku desno od "Pokrivalice" je
"Sync/B-roll", par radi kao prekidac (katalog `cover_mode_covers`, `cover_mode_sync`; `qnc-program-segments`
`CoverMode`, `action_selected`; `qnc-panel-focus` ih pusta iz svakog panela; `qnc-editorial-blocks`
`switch_button`). Ispod monitora novi prekidac Monitor Auto | On (katalog `monitor_auto`, `monitor_on`):
Auto (zadano) dok radi HDMI izlaz monitor ne crta sliku ("Slika je na HDMI izlazu"), HDMI i dalje dobiva
svaku sliku; On crta i uz HDMI (`qnc-program-output::showing`, `qnc-monitor`
`MonitorSurface::yield_to_external`, `qnc-source-preview` `PreviewView::monitor_auto` i `set_monitor_mode`,
`qnc-media-pool-head` `RowCommand` s oznakom ukljucenog). Izbor vrijedi dok aplikacija radi (postavka
laptopa, ne projekta). Ingest nema prekidac i uvijek crta. Probano u zasticenoj kopiji korijena (kopija
programa, ugovora i baze projekata s jednim projektom preusmjerenim na kopiju). Usput: C10 granice Story i
Ingest forme vracene ispod baselinea (popravak iz 69953d8 ih je prekoracio za po jedan redak).
Otkljucano i izvedeno 2026-10-09 (korisnik: "izbor tipkovnice prebaciti iz New template u Project Advanced,
jer na istom templateu vise korisnika moze imati razlicite tipke"; "otkljucaj"; "to se odnosi na sve
sadasnje i buduce forme"; "default postavka QNC keyboard map"): Project (`qnc-project-blocks`) izbornik
"Tipkovnica" iz bloka "Novi template" u Advanced postavke (blok "Tipkovnica", "Preset tipki"), zapis
ostaje `keyboard_shortcuts.active_preset` u postavkama projekta; template i dalje nosi zadanu vrijednost
(radne postavke traze taj blok). Nalaz: postavku do sada nije citala nijedna forma. Izvedeno:
`qnc-work-settings::keyboard_preset`, novi javni `crates/qnc-key-preset` (+ ugovor
`contracts/modules/key-preset.module.json`; u pozadini jednom u sekundi cita preset aktivnog projekta,
samo citanje), `qnc-key-intents` ga sam primjenjuje (i u pregledu tipki F1), pa vrijedi za svaku formu koja
tipke cita kroz tu kocku (Ingest, sve Media Assist, Story i buduce) bez izmjene forme. Project forma ga ne
koristi. `seed/system_seed.json`: svi sistemski templatei imaju zadani preset `default` (QNC; dva su bila
Resolve). Naziv preseta ostaje "QNC" (conformance: isto kao qnc_v4). Provjereno: testovi, conformance,
zasticena kopija (preset `resolve` u kopiji projekta procitan iz baze).
Otkljucano i izvedeno 2026-10-09 (korisnik: "u project formi trebali bi dodati i optimizirani source media u
ingest i play media... 8 bitna verzija kako bi se mogao koristiti GPU kod laptopa koji nemaju Nvidia RTX 50";
"najprije xavc 4:2:2"; nestandardni formati odbijeni, XDCAM 1080i50 original kasnije): Project Advanced
"Ingest media" dobio je "Optimizirani (H.264 8-bit)" (`optimized`) i "Optimizirani + original"
(`optimized_original`), "Playback input" "Optimizirani ako postoji" (`optimized_if_available`); stvarni
projekt se ne mijenja (bira korisnik). Novi javni `crates/qnc-media-optimize` (+ ugovor modula): H.264 High
8-bit 4:2:0 iste velicine, sve slike, GOP 10, bez B slika, ~35 Mbit/s, svi audio streamovi kopirani bit za bit
(`-c:a copy`), metapodaci i timecode zadrzani, QuickTime; enkoder po OS-u probnim kodiranjem (Windows QSV,
NVENC, AMF, libx264; macOS VideoToolbox, libx264; Linux NVENC, QSV, libx264), ffmpeg iz kataloga dekodera,
ispod normalnog prioriteta. `qnc-ingest-import-worker` (`optimize.rs`, `Action::Optimize`): kopija ide u
`optimized/<naziv>.mov` projekta kroz `.partial`, opisuje je jedan probe nove datoteke (Ingest proces, isti
lokalni ffprobe; original se ne probe-a ponovno) i provjerava prema spremljenom originalu (tocan broj slika,
fps, pocetni TC); inace se brise i uvoz je neuspio. `optimized_original` kopira i original, `optimized` ga
ostavlja vezanog na izvoru. `qnc-content-store` stupac `clips.optimized_json` (`FinishImport.optimized`,
`StoredClip.imported_optimized`). `qnc-player-input` `Representation::Optimized`: uz `optimized_if_available`
slika iz optimizirane kopije, zvuk i export ostaju iz originala; bez kopije svira original; kopija s drugim
brojem slika se odbija. `qnc-work-settings` `PlaybackInput::OptimizedIfAvailable`, `qnc-ingest-work-plan`
`IngestMedia::Optimized`. Izmjereno (klip 1492, 34 s): QSV kodiranje 13,5-15,2 s; dekodiranje originala 40,4 s
procesora, optimizirane kopije softverski 28,5 s, s QSV 1,2 s. Nije izvedeno: GPU (QSV) adapter dekodera
(sljedeci korak, bez njega player optimiziranu kopiju dekodira procesorom), XDCAM 1080i50, export.
Dopuna 2026-10-09: player klipa dobio je drugu vezu izvora za zvuk (`sound_binding`: `qnc-player-launcher`,
`qnc-player-client`, `tools/qnc-player-runner`) kad zvuk nije na izvoru slike (optimizirana kopija u
projektu, zvuk originala na kartici); svaki URI ide vezi svog izvora, kao u programu. Proba na zasticenoj
kopiji (kartica samo citana, stvarni projekt nediran): uvoz 2002 i 1492 s `optimized` napravio je, opisao i
provjerio obje kopije (MOV 169 MB i 1 GB); play s kopije 2002/1492 PASS 100 %, A/V -0,1 do -0,2 slike, koraci
32-36 ms, 10 od 10.
Otkljucano i izvedeno 2026-10-09 (korisnik "da" na GPU dekoder nakon optimizirane kopije): katalog dekodera
(`catalogs/decoders/catalog.json`) dobio je popis `prefer`: GPU dekoderi ovog racunala koji se pokusavaju prije
odabranog (`selected`, ffmpeg na procesoru). Zahtjev dekodira prvi koji izjavljuje spremljeni format (kodek,
spremnik, pixel format), odluka se donosi prije pokretanja dekodera, nikad nakon greske. GPU provjerava i spremljeni
format izvora, ne samo isporuceni (4:2:2 10-bit original isporucen kao 4:2:0 ide procesoru, jer bi ffmpeg na GPU-u
tiho presao na procesor). `qnc-decoder-catalog` (`Chain`, `Driver::FfmpegHwaccelV1`, provjera popisa),
`qnc-ffmpeg-decode` (`FfmpegAdapter::with_hwaccel`, `HWACCELS`: d3d11va, dxva2, cuda, vaapi, videotoolbox; slika se
spusta i pretvara u isporuceni format). Prvi unos: `qnc.ffmpeg.d3d11va` (Windows, svaki GPU; samo H.264 8-bit 4:2:0).
Provjera uredaja pri pokretanju nije uvedena (~150 ms po procesu playera); ako uredaj ne radi, ffmpeg javlja gresku.
Izmjereno (1492, 34 s, optimizirana kopija, isporuka yuv420p): procesor 20,0 s CPU, D3D11VA 11,7 s, QSV 11,8 s;
uz isporuku nv12 D3D11VA 8,0 s, DXVA2 7,7 s. Zasticena kopija: play 2002/1492 PASS 100 % (slika D3D11VA, zvuk
procesor), koraci 33 ms, 10 od 10.
Otkljucano i izvedeno 2026-10-09 (korisnik "da" na plan XDCAM 1080i50; v5/v4 `video_decode_filter`):
isprepleteni izvor (spremljeni `scan_mode` tff/bff) player trazi od dekodera kao progresivne slike:
`qnc-media-decode` `DecodeRequest.progressive` (i `saved_video`), `qnc-ffmpeg-decode` tada dodaje
`yadif=mode=send_frame:parity=tff|bff:deint=all` (oba polja u jednu sliku, isti broj slika i fps, redoslijed
polja iz baze), `qnc-broadcast-engine` (`InputPlan.progressive`, pretvarac slike dobiva progresivni opis;
opis izvora ostaje isprepleten). Original za export ostaje netaknut. GPU katalog takav izvor ne preuzima
(MPEG-2 4:2:2 ne izjavljuje). Proba (umjetni XDCAM HD422 1080i50 MXF iz ffmpeg testnog uzorka; javni uzorak
s archive.org je XDCAM HD 1440x1080i59.94 4:2:0 s anamorfnim pikselima, koji player i inace odbija): 25 slika
i bez pretvorbe i s njom, "cesalj" polja 3,44 -> 0,96 (test `real_interlaced_xdcam_hd422_...`). Nije izvedeno:
adapter kamere za XDCAM karticu (`sony-xdcam-mxf`: MEDIAPRO.XML + Clip/*.MXF), pa XDCAM klip jos ne ulazi
kroz Select u projekt i nije probani kroz player i monitor; anamorfni 1440x1080.
Otkljucano i izvedeno 2026-10-09 (korisnik "odkljucaj": pojedinacne datoteke iz `incoming/ftp`, v5 izvor "Kamera / FTP
incoming" i `scan_media_files`): kad u izabranoj mapi nema kartice poznate kamere (nalaz s datotekama klipa u mapi),
Select uzima svaku video datoteku kao jedan klip, samo original. `qnc-source-groups` (`FileReader`, `single_file`:
datoteka je sama sebi dokaz, bez proxyja; dijeljeni medij i dalje blokira), `qnc-scanner` (`scan_files`: samo izabrana
mapa, dubina 8, samo citanje, linkovi se ne slijede), `qnc-camera-adapter` (`files()`, `file_readers()`), novi javni
`crates/qnc-camera-generic-file` (+ ugovor `contracts/modules/camera-generic-file.module.json`; nastavci v5 bez
audio-only, zapis zna samo URI, uvijek jedan probe u Ingestu, nista iz imena datoteke), `qnc-ingest-cameras`
(registrira ga), `qnc-ingest-select` (`scan.rs`). Uz to, nuzno i javljeno korisniku: `qnc-source-index-contract`
(zapis s dokazom = original broji datoteku jednom), `qnc-media-metadata` (prazan popis streamova prije probea je
"nedostaje", ne "neispravno"; konacni nepotpun zapis Select i dalje odbija), `qnc-ffprobe-metadata` (MXF: kad je
vremenska baza tocno 1/fps i brzina stalna, `duration_ts` je tocan broj slika; inace i dalje procjena). Probni
program `crates/qnc-ingest-select/examples/select_folder.rs` pokrece isti Select na mapi izolirane kopije (odbija
projekt izvan zadanog korijena). Mapa izvora nikad se ne pise (indeks u mapi odbijen: izvor je samo za citanje).
Proba na zasticenoj kopiji (korisnikov Catalyst export `Mironik 2002.MXF`, XDCAM HD422 1080i50, kopiran u
`incoming/ftp` kopije; original u Videos samo citan): Select 0,5-2,8 s, jedan probe, zapis konacan i potpun, 5097
tocnih slika, isprepleten tff, TC 17:35:04:21; Uvezi (link); play 2 x PASS 100 % slika, A/V -0,1 slike, koraci 35 ms,
10 od 10, dekoder `yadif=mode=send_frame:parity=tff`. Nije provjereno: kroz sucelje Ingesta i Storyja, uz HDMI.
Dopuna 2026-10-09 (korisnik: "treba jedino dodati kreiranje postera"; "ne iz sredine klipa nego s pocetka"): klip
ciji izvor nema poster (pojedinacna datoteka) dobiva poster odmah nakon Selecta, prije Uvezi (v5 tu pokazuje samo
obojeni okvir). Pozadinski `tools/qnc-ingest-worker` nakon dovrsetka zapisa poziva
`qnc_ingest_import_worker::make_missing_posters`: popis iz jedinog citaca kataloga
(`qnc-content-read::clips_without_poster`, klipovi bez postera koji nisu uvezeni), prva kljucna slika klipa (pocetak,
i pri Uvezi; prije sredina) kroz `qnc-poster-create`, datoteka u `products/thumbnails/<klip>/poster.jpg` projekta
(nikad u izvor), upis kroz pisca baze novom operacijom `qnc-content-store` `SetPoster` (samo za klip bez postera).
Ponovni Select vise ne brise poster koji klip vec ima (bez postera kartice ostaje postojeci); Uvezi ne kopira poster
koji je vec u projektu. Ingest dok neki klip nema poster svake 2 s (najvise 3 min) cita potpis kataloga
(`qnc-clip-posters::missing_due`), pa se poster pojavi sam; citanje izvora za postere premjesteno u
`qnc-ingest-select::SelectionConfig::source_readers` (C10 Ingest application 1714 -> 1707). Greska postera ide u
log radnika. Proba na zasticenoj kopiji: dva klipa bez postera, oba napravljena, prva slika klipa.
Popravak 2026-10-09 (korisnik: "ne moze obrisati bazu projekta" dok QNC radi; "radimo u lego kockama"): bazu
zatvorenog projekta drzale su kocke, ne forme: nit pisca baze sadrzaja (`qnc-content-store` `ContentWriteTransport`,
preko nje i prijavljivac "player radi"), nit posrednika baze projekta (`qnc-db-broker` `ProjectDbWriter`: prica,
virtualni kadrovi, artefakti) i dijeljene veze za citanje u `qnc-content-artifacts`. Sve tri sada nakon 2 s
mirovanja zatvaraju vezu (`IDLE_RELEASE`) i otvaraju je pri sljedecem poslu; vrijedi za zatvoren projekt, drugi
otvoren projekt i skrivenu aplikaciju. Forme i aplikacijski slojevi nisu dirani (pokusaj da Ingest i Story sami
"zaborave" projekt je vracen jer je to logika u formi). Novi alat `tools/qnc-release-check` (samo izolirana kopija):
Story/MA aplikacija bez sucelja otvori klip, projekt se zatvori kroz `qnc-project-close`, i nakon 3 s baza kopije mora
se dati preimenovati. Prije: baza zauzeta; sada otpustena. Testovi: posrednik i pisac nakon mirovanja otpustaju bazu
i ponovno je otvaraju; dijeljena veza se otpusta, a nadzornik zavrsava s njom. Ingest koristi iste kocke; zivo nije
provjeren (alat ne smije ovisiti o Ingest aplikaciji, C6).

Otkljucano i izvedeno 2026-10-09 (korisnik: "montaza u originalu, a export u formatu za predaju"; "ok, kreni s
2:1"; odluka korisnika mijenja v5 pravilo jednog fps-a u prici): stopa price je stopa prvog segmenta, zapisana u
`story_state.story_fps_num/den` i zadrzana dok postoje aktivni segmenti (jedini segment je smije promijeniti). U
pricu smije izvor iste, dvostruke ili upola manje stope: 1080i50 XDCAM (25) u 50p prici daje svaki field kao sliku
price, 50p u 25 prici svaku drugu sliku, 25p u 50p svaku sliku dvaput; svaka druga stopa daje jasnu poruku.
`qnc-program-db` (`story_rate`, `story_per_source`; `story_parts.duration_frames` je duljina u slikama price,
`in_frame`/`out_frame` ostaju slike izvora; 50p u 25 prici zaokruzuje se na par; pokrivalica prolazi istu provjeru),
`qnc-program-segments` i `qnc-program-input` (izvor se racuna kao virtualni izvor na stopi price; ugovor playliste
nije mijenjan), `qnc-broadcast-engine` (`InputPlan` sa stopom price i `delivered_rate`), `qnc-media-decode`
(`DecodeRequest::delivered_rate`, samo 2x ili 1/2; 2x trazi spremljeni nacin snimanja), `qnc-ffmpeg-decode`
(`yadif` send_field po spremljenom redoslijedu fieldova, `fps=` za ponavljanje/izostavljanje, vremenske oznake iz
filtra). Ugovor `story.database.json` (`story_rate`). Sync/B-roll i dalje odbija razlicitu stopu. Provjereno:
testovi navedenih crateova, conformance, izolirana kopija (priča FX6 50p 5 s + XDCAM 1080i50 5 s = 250 slika price +
FX6 50p 5 s; XDCAM je kopiran u scratchpad, izvor se samo cita): 1 od 3 PASS, ostali 100 % i 95 % slika s 1-3
trzaja 63-79 ms, A/V -0,2 slike, zvuk XDCAM-a od izvora 200 (= 100 x 2). Export u format za predaju nije izveden.
Dopuna 2026-10-09 (korisnik: "rijesi pokrivalice i sync/b-roll"): pokrivalica i Sync/B-roll primaju izvor iste,
dvostruke ili upola manje stope od price. `qnc-sync-cover`: `start` prima omjer slika price i izvora (1:1, 2:1,
1:2; odlucuje ga pozivatelj po pravilu price), prozor programa, slika izvora pod Syncom i OUT izvora zatvorenog
slota racunaju se tim omjerom (prije je odbijao svaku drugu stopu); `qnc-program-segments` omjer uzima iz
`qnc_program_db::story_per_source`, prikaz pokrivalice (val na A2) ide do slike izvora koju pokrivalica stvarno
pusti, a preslikavanje frame programa <-> slika izvora segmenta ide omjerom duljina. Nalaz i popravak u
`qnc-program-db`: polozaj segmenta (marker u segmentu) i duljina programa racunali su se iz slika izvora
(`out_frame - in_frame`), pa je marker u XDCAM segmentu 50p price padao na krivi frame; sada iz
`duration_frames` (test). Ugovor `sync-cover.module.json`. Provjereno: testovi, conformance, izolirana kopija
(prica FX6 + XDCAM + FX6, XDCAM pokrivalica u FX6 segmentu i FX6 pokrivalica u XDCAM segmentu, pisane kroz
`qnc-program-db`): 2 od 3 PASS, prvi prolaz 93 % s trzajima 62-140 ms (hladan disk), A/V -0,2 slike, zvuk XDCAM
pokrivalice od izvora 2000 (= 1000 x 2). Sync/B-roll uzivo kroz sucelje nije proban (alat pusta samo pricu).
Popravak 2026-10-09 (korisnik: "ako se odabere krivi folder, onda zablokira i neda brisati projekt.. treba uvesti
filter dozvoljenih datoteka"; odluka "po formatu"): izbor mape Videos pokupio je i Edius render datoteke
(`RenderAndPaste/*.avi`, HQX 4:2:2 16-bit). Select je klip objavljivao prije probea, pa su nepodrzane datoteke ostale
u katalogu, a `tools/qnc-ingest-worker` je radio i nakon zatvaranja projekta i drzao njegovu bazu. Izvedeno:
(1) worker svake sekunde cita aktivni projekt iz baze; kad vise nije onaj za koji je krenuo (zatvoren ili drugi),
staje (dovrsetak zapisa, artefakti i uvoz se prekidaju, radnici filmstripa/wavea se otkazuju) i pusta bazu.
(2) Filtar po formatu: zapis koji kartica ne opisuje (pojedinacna datoteka, kamera bez XML-a) postaje klip tek nakon
jedinog probea u Ingestu i samo ako ga dekoder ovog racunala moze pustiti: `qnc-decoder-catalog`
(`Catalog::refusal`, `installed_refusal`: spremnik, kodek i pixel format slike i kodek svakog zvuka moraju biti u
izjavi nekog dekodera kataloga, iste izjave koje provjerava play), `qnc-record-probe` (`Error::Unreadable`: probe
koji nije procitao medij, npr. spremnik izvan dopustenih, i ponovni Select istog medija), `qnc-ingest-select`
(`metadata.rs`: takav zapis se ne objavljuje, Ingest dobiva `Removed` i upozorenje "preskoceno, format nije
podrzan (...)"; zapis kartice koju kartica opisuje i dalje se objavljuje odmah). Provjereno: testovi
(`qnc-decoder-catalog` novi test: XDCAM MXF prolazi, HQX AVI i zvuk mp3 se odbijaju), conformance, izolirana kopija
(mapa s HQX AVI, VP9 MKV, ProRes 4444 12-bit MOV, H.264 MOV i XDCAM MXF: klipovi su samo H.264 i MXF, ostali
"preskoceno" s razlogom). Nalaz za kasnije: ponovni Select mape u kojoj je datoteka istog imena zamijenjena novom
daje "Izvorni indeks: Conflict" i Select ne prolazi.
Popravak 2026-10-10 (korisnik: "rijesi Conflict kod zamjene datoteke"): proba na zasticenoj kopiji pokazala je da
`Conflict` ne dolazi od zamjene datoteke nego od izbora druge mape: zapis pojedinacne datoteke za korijen je uzimao
mapu koju je korisnik izabrao, pa je ista datoteka iz roditeljske ili podmape bila drugi zapis s istim medijem i
izvorni indeks ga je odbijao. `qnc-scanner` (`scan_files`): korijen je mapa u kojoj datoteka stoji, pa je zapis isti
bez obzira na izabranu mapu (postojeci zapisi napravljeni iz izravne mape datoteke ostaju isti). Test u
`qnc-scanner`. Proba: Select roditeljske mape, podmape pa opet roditeljske prolazi, postojeci klip ostaje isti.
Nalaz (nije popravljen, ceka odluku): datoteka zamijenjena drugom istog imena prolazi kao nepromijenjena i zadrzava
stari zapis i probe, jer izvor daje samo velicinu datoteke, a izvorni indeks ne pamti ni velicinu.
Popravak 2026-10-10 (korisnik "ok" na plan: zamijenjena datoteka istog imena): `qnc-source-contract` `FileInfo`
dobio je vrijeme izmjene (`modified_unix_ms`, neobavezno), lokalni citac ga daje (isti ugovor ide i preko mreze);
`qnc-source-groups` `GroupProposal.stamp` (`FileStamp`: duljina i vrijeme izmjene, samo pojedinacna datoteka; zapisi
kartica ga nemaju i ne mijenjaju se); `qnc-scanner` cita pecat svake prihvacene datoteke (samo citanje).
`qnc-source-index-db`: pojedinacna datoteka trazi se po samoj datoteci; ista datoteka s ranijim korijenom ili bez
pecata zadrzava zapis i dobiva korijen i pecat ovog skena; drugi pecat na istom mjestu je zamijenjena datoteka:
stari zapis se povlaci, nova datoteka dobiva novi zapis; kartica i dalje daje `Conflict` na svaku promjenu.
`qnc-ingest-select` (`records.rs`, `publish.rs`): klip na mjestu zamijenjene datoteke uklanja se kao nestao ako nije
uvezen, a za uvezeni Ingest javlja upozorenje (uvezeni klip ostaje; uvezen kao link vise ne odgovara datoteci).
`qnc-media-record-db`: zavrseni probe drugog klipa na istoj adresi pripada datoteci koja je tamo bila, pa novi klip
dobiva svoj jedan probe; probe koji jos traje i dalje se ne preuzima. Ugovor `source-index.database.json`
(`single_file_rule`). Testovi `qnc-source-index-db`, `qnc-media-record-db`, `qnc-scanner`, conformance. Proba na
zasticenoj kopiji projekta `testllll`: zamjena 3 s -> 9 s daje novi klip s 225 slika i uklanja stari, ponovni Select
bez promjene ne dira nista, uvezeni klip ostaje uz upozorenje i novi klip; Select mape Videos: uvezeni
`Mironik 2002.MXF` ostaje isti klip bez duplikata, Edius datoteke su preskocene.
Audit 2026-10-10 (korisnik: "cini mi se da si napustio Lego kocke sistem i da ti je previse koda uslo u forme"):
forme `*-desktop` su tanke (Project 67, Ingest 89, Story/MA 96 redaka), ali je njihov kod 30. 9. preseljen u ploce
`*-blocks` (Project 2365, Ingest 903, Story/MA 450), koje pravila C1-C10 nisu gledala; `qnc-program-segments` je
narastao u upravljac cijele price (`lib.rs` 1547 redaka). Izvedeno (korisnik "ok, kreni s 1 i 2"):
(1) `tools/qnc-conformance` (`lego.rs`): ploce `qnc-*-blocks` su forme za C1-C4 i C10, novo pravilo "C2 form private
widget" (`Button::new` u formi); baseline dobio njihove kljuceve sa stanjem tog dana (smiju samo padati).
(2) Story ploca i pravilo crtanja u kocke: mjerenje blokova u `qnc-dev-diagnostics` (`timed_block`), prijevod A1/A2
trake i vala za source timeline u `qnc-segment-panel` (`source_lane_choice`, `source_wave_over_video`), gumb u
`qnc-ui-kit` (`chrome_button`, `ChromeButtonStyle`), trajanje kartice u `qnc-media-card` (`minutes_seconds`), tekstovi
praznog Virtual taba i ucitavanja u `contracts/ui/editorial.layout.json` (`virtual_empty_message`, `loading_message`;
`qnc-editorial-layout`), pravilo "dok player svira crtaj uz njegovu sliku" iz `app.rs` Ingest i Story forme u novu
javnu kocku `crates/qnc-paint-pace` (+ `contracts/modules/paint-pace.module.json`). Izgled i ponasanje isti.
Brojke: Story ploca 450 -> 382 redaka, Story forma 96 -> 88, Ingest forma 89 -> 79, C4 u obje forme 2 -> 1.
Nije izvedeno (sljedeci koraci 3 i 4): podjela `qnc-program-segments`, ciscenje Ingest ploce (tekstovi, gumbi,
`format_duration`) i Project ploce (uredjivanje postavki, 101 tekst u kodu; trazi otkljucavanje Projecta).
Korak 3 audita 2026-10-10 (korisnik "ok, kreni s 3"): `qnc-program-segments` (`lib.rs` 1547 redaka, upravljac cijele
price) podijeljen je po odgovornostima u tri nove javne kocke, s ugovorima modula: `crates/qnc-program-commands`
(naredbe price, vrsta segmenta Ton/Off i jedino preslikavanje `action_id` -> naredba; bez stanja),
`crates/qnc-program-view` (prikaz price iz zapisa baze: redovi, dijelovi taba Segment, markeri i slotovi (`markers.rs`
preseljen), pokrivalice s izvornim slikama koje pustaju (`cover_spans`), oznake trajanja, pitanja nad prikazom; bez
stanja, bez baze) i `crates/qnc-lane-choice` (kanal na A1 i A2, uzeta traka i nacrt, veliki val; svoje stanje, bez
baze). `qnc-program-segments` ostaje upravljac (stanje, citanje i upis price kroz posrednika baze, slaganje) i
izvozi sve pod istim imenima, pa forme i ostale kocke nisu mijenjane. `lib.rs` 1547 -> 1039 redaka. Ponasanje isto:
testovi `qnc-program-segments` (27, ukljucujuci A1/A2 tipkovnicu), `qnc-program-view` (11), `qnc-lane-choice`,
`qnc-segment-panel`, `qnc-editorial-application`, `qnc-panel-focus`, conformance, `cargo check -p qnc-app`.
Korak 4 audita 2026-10-10 (korisnik "ok, kreni s 4"; time i otkljucavanje Project ploce `qnc-project-blocks` i
njezina layout ugovora samo za ciscenje, bez promjene izgleda i ponasanja): Ingest i Project ploca daju aktivni kod
i tekstove kockama i ugovoru. Ingest (`qnc-ingest-blocks`): tekstovi preglednika, mreze i docka u
`contracts/ui/ingest.layout.json` (+ `qnc-ingest-layout`; vec postojeci `no_card_thumb_hint` i
`generate_posters_label` sada se i koriste), boje kroz `qnc_ingest_layout::rgb`, gumbi kroz `qnc-ui-kit`
(`chrome_button`, novi `strong_button`, `fill_block`), trajanje kartice `qnc_media_card::minutes_seconds`; zamjenski
tekstovi gumba vise nisu u kodu (bez oznake u ugovoru gumb pokazuje svoj action_id). Project (`qnc-project-blocks`):
Advanced postavke su obrazac u `contracts/ui/project.layout.json` (`advanced_settings`: sekcije, polja, izbori,
uvjeti prikaza, tekstovi) koji crta nova javna kocka `crates/qnc-settings-form` (+ `contracts/modules/settings-form.module.json`;
polja, izbor, broj s decimalnim zarezom, upis u nacrt postavki); ploca slaze samo tri bloka koji su vise od polja
(preset tipki, izvozni preset, izbor aplikacija). Ostali tekstovi ploce u `texts` istog ugovora; gumbi kroz
`qnc_ui_kit::look_button` (`ButtonLook`, isti izgled kao prije); redoslijed preseta tipki u ugovoru
(`keyboard_preset_order`), slaganje izbora u `qnc-keyboard-shortcut` (`ShortcutCatalog::preset_choices`).
Brojke: Ingest ploca 903 -> 882 redaka, tekstovi 14 -> 0, privatni gumbi 2 -> 0, boje 2 -> 0; Project ploca
2365 -> 1932 redaka, tekstovi 113 -> 8 (preostali su kljucevi mjesta iz ugovora), privatni gumbi 4 -> 1.
Ostaje (u baselineu): slaganje kartica iz podataka klipa u Ingest i Story ploci (C3), bojanje redaka popisa i
preglednika u Project ploci (C1), gumb X za brisanje templatea, pretvorba boja ugovora u `theme.rs`. Provjereno:
testovi `qnc-settings-form`, `qnc-keyboard-shortcut`, `qnc-ui-kit`, `qnc-ingest-layout`, `qnc-project-blocks`,
`qnc-contracts`, conformance, `cargo check -p qnc-app`. Uzivo nije provjereno.
Popravak 2026-10-10 (korisnik: posteri se u Storyju "prikazuju, ali ne dok se kreira filmstrip"; "otkljucaj"
`crates/qnc-monitor`): dok pozadinski proces radi filmstrip, zapisi klipova dovrsavaju se kasnije i preview javlja
"Podaci klipa se pripremaju..."; monitor je imao redoslijed slika -> poruka -> poster, pa je poruka skrivala poster.
`qnc-monitor::paint_source_monitor`: slika playera, inace poster s porukom kao natpisom pri dnu (tamna traka), inace
sama poruka, inace naziv klipa. Vrijedi za sve forme. Testovi `qnc-monitor`, conformance. Monitor je ponovno zakljucan.
Popravak 2026-10-10 (korisnik: "sporo reagira na klik misem"): dijagnostika je pokazala da se nit sucelja na klik
na klip blokira 1,5-1,7 s (`ui-frame-slow`) dok pozadinski proces radi filmstrip. Na klik su se na niti sucelja
citali timecode i broj audio kanala klipa iz baze sadrzaja (`qnc-source-timecode::SourceTimecodes`,
`qnc-source-channels::SourceChannels`), a citanje ceka dok pozadinski proces upisuje u istu bazu. Obje kocke sada
novi klip citaju u zasebnoj niti i do odgovora vracaju "nepoznato"; vrijednost se pojavi cim stigne. Ingest i Story
isto. Ostaje: priprema playera (`load_ms`) za vrijeme filmstripa traje 1,4-2,4 s umjesto ~50 ms (cita u pozadini,
ne blokira sucelje; uzrok je isto cekanje na bazu).
Otkljucano i izvedeno 2026-10-10 (korisnik: "pomicanje selektiranih IN/OUT/Marker je sporo"; "da" na otkljucavanje
`qnc-broadcast-engine`): u Wrapu svaki korak strelicom salje playeru programa cue, a slika programa
(`program.rs` `ProgramVideo`) je pri svakom cueu gasila oba dekodera, praznila gotove slike i otvarala dekoder od
kljucnog kadra (log: Cue do slike na monitoru 0,5-1,5 s). Sada kao korak klipa (3. 10.): cue na sliku koja je u
memoriji ili unutar dosega otvorenog dekodiranja ne dira dekoder; slike do 25 iza zadnjeg cuea ostaju (`cue_anchor`,
`kept_from`, `forget_far_from`, +25 spremnika slika), korak unatrag izvan njih otvara dekodiranje 25 slika ranije
(`restart`), a predana slika uvijek dobiva tekucu generaciju i sljedeci redni broj. Zvuk programa se pri koraku i
dalje ponovno otvara (~50 ms). Provjereno: testovi `qnc-broadcast-engine`, conformance, release build.
Dopuna 2026-10-10 (korisnik: "ovu mogucnost dodao i pocetku i kraju segmenta, ako korisnik pozeli skratiti ili
produziti segment"; "ok" na plan; v5 to nema): u panelu Segmenti Ctrl+I uzme pocetak, a Ctrl+O kraj segmenta pod
Wrap playheadom (inace odabranog). Klip segmenta otvara se na tom rubu s uzetom oznakom (isti put kao Edit), pa
strelice pomicu rub za sliku i monitor pokazuje novu prvu ili zadnju sliku, i izvan segmenta kod produljivanja;
Enter zapisuje `TrimSegment` (markeri prate sliku), Esc odustaje; oboje vraca tipke panelu Segmenti i Wrap na novi
rub, odnosno gdje je bio. `qnc-program-segments` (`edit.rs`: `take_edge`, `trimming_edge`, `take_wrap_return`),
`qnc-panel-focus`, `qnc-pending-marks` (oznaka koju treba uzeti i frame otvaranja), `qnc-editorial-application`
(isti broj redaka), ugovori triju modula. Katalog tipki nije mijenjan. Testovi, conformance.
Popravak 2026-10-10 (korisnik: "dok ne postoje segmenti, Pokrivalice i Sync/B-roll trebaju biti nedostupni; po
defaultu ni Pokrivalice ni Sync/B-roll nisu odabrani"): prekidac u source docku ima tri stanja: na pocetku nista
odabrano, zatim Pokrivalice ili Sync/B-roll. Bez segmenata oba gumba (i tipka Sync) su nedostupni, a kad nestane
zadnji segment izbor se ponistava i Sync se gasi. `qnc-program-view` (`covers_chosen`, `action_enabled`,
`action_selected`), `qnc-program-segments` (`CoverMode`, `toggle_sync`). Ploca i layout nisu dirani. Test.
Dopuna 2026-10-10 (korisnik: "Ctrl+S selektira slot, trebao bi mogucnost pomicanja slota strelicama"; "pomicanje
slota pomice marker, ako slot ulazi u susjedni slot, marker putuje sa slotom"; "otkljucaj"; v5 to nema): nakon
Ctrl+S strelice pomicu slot za sliku zajedno s oba njegova M markera; slot zadrzava duljinu i pokrivalicu, susjedni
slot se produljuje ili skracuje; slot staje prije susjednog markera, a slot uz zakljucani pocetni ili zavrsni marker
se ne pomice. Wrap playhead pokazuje prvu sliku slota. Enter zapisuje novu operaciju `MoveSlot` (oba markera u
jednoj transakciji, jedan Undo korak), Esc odustaje. `qnc-program-db` (`MoveSlot`, test), `qnc-program-segments`
(`slot_move.rs`, nacrt markera i slotova u prikazu, test), ugovor `story.database.json` (`slot_move`). Katalog
tipki, forme i layout nisu dirani. Conformance.
Dopuna 2026-10-10 (korisnik: "nije dovoljna prva slika slota... monitor bi trebao prikazivati sve kljucne slicice:
zadnju sliku prethodnog slota, prvu i zadnju slota kojeg pomicemo i prvu sljedeceg"; "moze to biti i na timeline";
"super bi bilo da bude univerzalna"; "ok"): dok je slot uzet (Ctrl+S) ili marker (Ctrl+M), Wrap redovi u panelu
Segmenti pokazuju uz svaki pomaknuti marker zadnju sliku prije i prvu poslije njega (slot 4, marker 2), iz nacrta:
pokrivalica pomaknutog slota ide s njim, susjedni slot pokazuje svoju pokrivalicu ili sliku segmenta, Off je crn.
Monitor i Broadcast Player nisu dirani. `qnc-program-view` (`edit_stills`, `EditStill`, `StillPicture`, polje
`SegmentsView.edit_stills`, testovi), `qnc-program-segments` (`edit_moves`, test), `qnc-timeline`
(`TimelineEditStill`, `TimelineStillImage`, polje `edit_stills`, crtanje u video traci), `qnc-segment-panel`
(sličice na redovima segmenata), `qnc-source-preview` (`StillLocator`: isti zapis, `playback.input` i veze izvora kao
player, lokalna datoteka; bez probea), nova kocka `crates/qnc-trim-stills` (+ ugovor modula; izvlacenje u pozadini
kroz katalog dekodera, jedan posao, najnoviji nacrt, cache 64), `qnc-decoder-catalog` i `qnc-ffmpeg-decode` (nacin
`ExactSeek`: tocan pomak bez `-noaccurate_seek`; provjereno na `Mironik 2002.MXF`, frameovi 100, 101, 437 jednaki
referenci dekodiranoj od pocetka), `qnc-editorial-application` (jedan poziv; C10 708, baseline spusten).
Nije izvedeno: rub segmenta (Ctrl+I/O) na source timelineu jos nema slicica.
Dopuna 2026-10-10 ("nastavi"): i rub segmenta uzet s Ctrl+I/O ima slicice, na source timelineu uz IN ili OUT koji se
pomice: pocetak pokazuje zadnju sliku programa prije segmenta i novu prvu sliku segmenta, kraj novu zadnju sliku i
prvu sliku programa poslije segmenta. `qnc-program-view` (`program_picture`, `EditStill::new` s identitetom slike,
`SegmentsView.source_edit_stills`), `qnc-program-segments` (`edge_stills`, test), `qnc-trim-stills` (puni obje
liste), `qnc-timeline` (`show_source_player_timeline_with_channel` prima slicice), `qnc-source-dock`
(`with_edit_stills`), `qnc-segment-panel` (`source_edit_stills`, `StorySourceTimeline::story`: dio source timelinea
koji daje prica u jednom pozivu ploce; C10 `qnc-editorial-blocks` 381 -> 380). Ingest nije diran.
Izmjena 2026-10-10 (korisnik: "nema dovoljno prostora na timeline za prikaz 4 slicice... overlay prozor"; "ne bi
dodavao novo slovo, koristimo vec postojeca vezana za Ctrl"; "na centru ekrana"; "kao nekakav kvazi zum"): slicice
vise nisu na timelineu (`qnc-timeline`, `qnc-source-dock` i Wrap redovi vraceni na stanje prije njih). Nova pasivna
kocka `crates/qnc-trim-overlay` (+ ugovor modula): dok je nesto uzeto postojecim Ctrl tipkama (Ctrl+S slot, Ctrl+M
marker, Ctrl+I/O rub segmenta), u sredini ekrana je prozor s tim slikama velikima u jednom redu (do 480x270), svaka s
nazivom, crta na mjestu markera; Enter/Esc ga zatvara. Nazivi slika dolaze iz `qnc-program-view`
(`EditStill::labelled`) i `qnc-program-segments`. Ploca Storyja ga zove jednim retkom (C10 379). Nova tipka nije
dodana.
Popravak 2026-10-10 (korisnik: "ne generiraju se slicice"; "prilikom rada strelicama ne mijenjaju se slike"): log je
pokazao da su trazene slike tocne, ali je svaka nova trebala 0,5-1 s (svaki posao ponovno je citao bazu za klip i
pokretao ffmpeg za jednu sliku), a zapeli posao (bez ffmpega) je zauvijek blokirao sljedece. `qnc-trim-stills`:
lokacija klipa se cita jednom po klipu; jedan prolaz dekodera izvlaci 17 uzastopnih slika oko trazene
(`extract_run_with_cancel` u `qnc-decoder-catalog`, `extract_frame_run_with_cancel` u `qnc-ffmpeg-decode`), pa su
koraci strelicom unutar +-8 slika iz memorije (cache 160); posao dulji od 8 s se prekida (ffmpeg se gasi) i krece
sljedeci; log biljezi trazene slike i trajanje svakog prolaza. Izmjereno s kartice: 17 slika 1,2 s, slika 196 klipa
2686 jednaka referenci.
Popravak 2026-10-10 (korisnik: slike izvan slota nisu dobre na strelicama): nacrt je pokrivalicu susjednog slota
drzao na staroj duljini, a baza (`rebind_covers`) pokrivalicu uvijek vezuje na cijeli slot i ona pusta izvor od IN
koliko je slot dug. Sada i nacrt (`qnc-program-view` `cover_picture`): pokrivalica pokriva cijeli pomaknuti slot,
pa zadnja slika produljenog prethodnog slota je nastavak njegove pokrivalice, a ne segment ispod. Testovi.
Popravak 2026-10-10 (korisnik: "kad selektiram slot i pomaknem strelicu, playhead uvijek skoci na pocetak slota"):
pomak slota vise ne pomice Wrap playhead (`qnc-program-segments` `slot_move.rs`); slike reza su u trim prozoru.
Dopuna 2026-10-11 (korisnik: "nema nikakvog pomaka ni promjene slicica... mozemo li imati dva playheada"): pomak
slota za jednu sliku na redu od nekoliko tisuca slika manji je od piksela, a pokrivalica putuje sa slotom, pa se
pomak nije vidio. Uz playhead playera (ostaje gdje je, monitor ga prati) sada je drugi, trim playhead: bijela crta
na svakom pomaknutom markeru (slot 2, marker 1) s TC-om nacrta, samo dok je nesto uzeto. `qnc-program-view`
(`SegmentsView.trim_frames`), `qnc-program-segments` (iz `edit_moves`, test), `qnc-timeline` (`trim_playheads`,
crtanje preko svih traka), `qnc-segment-panel` (na redu koji crta marker). Rub segmenta to vec ima na source
timelineu (playhead prati IN/OUT).
Dopuna 2026-10-11 (korisnik: "kada je aktivan Ctrl+ dodao bi strelicu gore i dolje: pomak za jedan GOP ako je LongGop,
ako je Intra pola sekunde"; "ako je GOP komplikacija, onda na sljedecu najblizu intra slicicu"): dok je rez uzet
(Ctrl+S slot, Ctrl+M marker, Ctrl+I/O IN/OUT ili rub segmenta), strelica gore skace na prethodnu, a dolje na sljedecu
kljucnu sliku medija koji se pusta; kad su sve slike kljucne (intra), skok je pola sekunde. Trake A1/A2 zadrzavaju
gore/dolje za val. Nova kocka `crates/qnc-keyframe-nav` (+ ugovor): kljucne slike cita iz paketa (kopija toka, samo
kljucni paketi, `framecrc`) kroz katalog dekodera u pozadini, nista ne dekodira ni zapisuje; slika se broji od
spremljenog pocetnog PTS-a kao u playeru; intra se pamti po klipu. `qnc-ffmpeg-decode` (`key_pictures_with_cancel`),
`qnc-decoder-catalog`, `qnc-source-preview` (`LocatedPicture.start`, `frame_at`, `seek_of`), `qnc-source-mark-focus`
(`taken_picture`), `qnc-program-segments` (`keyframe_reference`: slika prije prvog pomaknutog markera; `jump_edit`,
`say`), `qnc-panel-focus` (gore/dolje na uzetom rezu, `poll` primjenjuje odgovor), `qnc-editorial-application` (isti
broj redaka). Provjereno na proxyju 2686 (GOP 24): od 30 naprijed +18, unatrag -6, od 48 naprijed +24. Omjer 2:1
price i izvora pri skoku slota/markera nije uzet u obzir (skok je u slikama izvora).
Izvedeno 2026-10-11 (grana `decoded-window`, od spremljene verzije `v-2026-10-11-pre-intra-preview`; korisnik: "idemo
naci najbolje rjesenje u situaciji da ne postoji proxy koji je generirala kamera"): bez kodiranja. Dok player klipa
stoji, `qnc-broadcast-engine` (`backfill.rs`) nakon 600 ms mirovanja vlastitom niti i vlastitim dekoderom (nizi
prioritet procesa, `qnc-media-decode` `Decoder::set_background_priority`) dekodira 1,5 s prije pozicije (prethodni GOP)
i drzi slike u memoriji; korak unatrag ih uzima bez ponovnog otvaranja dekodera playera; dopunjava u komadima od
0,75 s; pozadina staje cim krene Play (oznaka `playing` u zajednickom `VideoSink` postavljaju Play/Pause/Stop/cue),
a pokrece se iz petlje playera (`Runtime::tick`). Samo za preview monitor; nista se ne kodira ni zapisuje. Uz to
ispravak veze playera (`control.rs`) i alat za mjerenje (`--back`, `--jumps`). Izmjereno na izoliranoj kopiji
(original 2002 s kartice, XAVC 4:2:2 10-bit, od 1500; ista verzija s pozadinom i bez nje, naizmjenicno, 3x):
koraci unatrag najgore 45-49 ms (bez 0,72-0,75 s), p90 43-44 ms (57-69 ms); koraci naprijed isti (40-43 ms);
skokovi +-50 slika medijan 0,52-0,55 s (0,68-0,70 s); Play originala 98,8-99,8 % slika (98,2-100 %).
Izvedeno 2026-10-11 (korisnik: "napravi isto i za Wrap"): pozadina je opcenita (`backfill.rs`: dijelovi u slikama
onoga sto player pusta; segment i pokrivalica citaju svoj klip od izvorne slike, Off nema sliku i player ga crni
odmah), pa je ima i player price (`ProgramVideo`, Wrap): isti uvjeti (600 ms mirovanja, nizi prioritet, staje kad
krene Play), pretvarac po formatu slike svakog klipa. `qnc-player-acceptance --program-back FRAME` mjeri korake
unatrag po prici kako ih salje Wrap. Izmjereno na izoliranoj kopiji (prica 2002 + 2676 s pokrivalicama 2677/2686,
originali s kartice; sa i bez pozadine naizmjenicno, 2x): unatrag od 1500 najgore 50-59 ms (bez 0,68-0,78 s), p90 32 ms
(86-93 ms); unatrag od 300 preko reza pokrivalica najgore 71-96 ms (1,00-1,13 s), p90 47-49 ms (135-140 ms); Play price
99,2 % slika (99,8-100,2 %), prva slika 164-181 ms (56-90 ms).
Otvoreno ograniceno odobrenje 2026-10-11 (jedan izvor slika; korisnik: "odkljucaj"): proba na zasticenoj kopiji
pokazala je da original istodobno dekodiraju player, zasebni izvlakac slika prozora reza (`qnc-trim-stills`, ffmpeg
prolazi po 17 slika, 0,6-2,5 s, okviri crni dok ne zavrsi) i ingest-worker; pozadina playera zato kasni (7 s umjesto
1,8 s) i koraci skacu. Plan: prozor reza trazi slike prije i poslije markera/slota od playera (slike koje player vec
drzi u memoriji), bez drugog dekodiranja; pozadina playera krece odmah kad je rez uzet i pokriva i iza polozaja.
Pikseli ne idu upravljackim kanalom nego zasebnom frame-mapom sesije. Otkljucano: `qnc-player-contract`,
`qnc-player-client`, `qnc-player-frame-transport`, `tools/qnc-player-runner`, `qnc-broadcast-engine`,
`qnc-source-preview`, `qnc-trim-stills`, po potrebi `qnc-editorial-application` (samo poziv), ugovori, testovi,
`tools/qnc-player-acceptance`. Forme i layout se ne diraju. Probe samo na zasticenoj kopiji (`scratchpad/proba` s
vlastitom kopijom programa; stvarne baze se provjeravaju prije i poslije).
Usput zapis: prvo pokretanje probe 2026-10-11 u 20:45 (aplikacija iz `QNC/target/release` uz `QNC_ROOT` zasticene
kopije) otvorilo je stvarne baze, jer dio aplikacije trazi korijen po mjestu programa; upisano je samo vrijeme
otvaranja projekta i sistemskih predlozaka u `data/qnc-projects.db` i `worker_lease = off` u `project.db` projekta
`novi test11`, bez promjene sadrzaja. Zasticena proba od tada radi s kopijom programa unutar zasticenog korijena.
Izvedeno 2026-10-11 (jedan izvor slika): `qnc-player-contract` naredba `PublishStills { pictures: [StillRef] }`
(najvise 8; slike koje player vec drzi u memoriji idu u zasebnu frame-mapu sesije `stills`, nista se ne dekodira),
`qnc-broadcast-player` `TransportEngine::video()` (samo citanje adaptera slike), `qnc-broadcast-engine`
`Runtime::held_pictures` (klip: pripremljene, zadrzane i pozadinske slike; prica: slika programa koja pusta tu
izvornu sliku), `tools/qnc-player-runner` (frama-mapa `stills_frame_map`, zapis nadenih slika), `qnc-player-client`
(`publish_stills`, `take_stills`), `qnc-source-preview` (`ask_stills`, `take_stills`), `qnc-trim-stills` (prvo pita
player; dekodira sam samo sliku koju player ne da u 250 ms), `qnc-editorial-application` (isti redak poziva).
Provjereno: testovi, conformance, `qnc-player-acceptance` (play klipa 100 %, koraci unatrag po prici najvise 73 ms,
po klipu 48 ms). Uzivo jos nije probano.
Live 2026-10-11 (korisnik: "i dalje nered u prikazu slika, ima pa nema"): log zasticene probe: player nije dao
nijednu sliku, jer drzi samo slike oko mjesta gdje stoji (1,5 s iza i malo ispred), a rubovi pomaknutog slota su
daleko od Wrap playheada; sve je islo kroz izvlacenje u `qnc-trim-stills`, a ono je sve prolaze jednog posla
vracalo tek na kraju (4 slike x ~0,6 s s kartice prije prve slike) i mjesto bez nove slike crtalo crno.
Izvedeno: `qnc-trim-stills` ima jednog pozadinskog radnika koji uvijek uzima slike koje nedostaju u najnovijem
nacrtu, svaki prolaz se pokaze cim je gotov, prolaz koji zapne prekida se nakon 8 s; ne ceka se 250 ms na player
(i dalje ga pita, prva stigla slika se pokaze); prolaz izvlaci 25 slika (12 sa svake strane); dok nova slika ne
stigne, mjesto u prozoru drzi zadnju sliku koju je pokazalo.
Live 2026-10-11 (korisnik: GOP skokovi slota rade, "pomicanje po sliku nije dobro"; "pomicanje markera pokazuje samo
slicicu prethodnog klipa, a novu granicnu slicicu sljedeceg slota ne mijenja"): log pokazuje tocne trazene slike;
prolaz izvlacenja s kartice traje 0,75-1 s za 25 slika, pa strelica po slici stigne prolaz. `qnc-trim-stills` sada
nakon slika koje nedostaju izvlaci i sljedece slike u smjeru zadnjeg pomaka (prolaz pocinje od prve neizvucene,
do dva prolaza ispred). Marker: slika poslije markera ostaje ista jer sljedeci slot ima pokrivalicu, a baza
(`qnc-program-db` `rebind_covers`) pokrivalicu uvijek pusta od njezina IN-a; prikaz to tocno pokazuje. Promjena
tog pravila (pomak markera pomice i IN pokrivalice sljedeceg slota) ceka odluku korisnika.
Otkljucano i izvedeno 2026-10-11 (korisnik: "da, roll rez, otkljucaj qnc-program-db"): pomak pocetnog markera slota
s pokrivalicom (`MoveMarker`, ili slot iza pomaknutog slota kod `MoveSlot`) pomice i IN/OUT pokrivalice tog slota za
isti broj slika (omjerom price i izvora), pa se mijenja i prva slika poslije markera; pokrivalica ne moze poceti
prije svog izvora (pomak se odbija); slot pomaknut u cijelosti zadrzava svoju pokrivalicu. `qnc-program-db`
(`roll_cover`, test; Undo ga vraca), nacrt prikaza isto pravilo (`qnc-program-view` `cover_picture`, test;
`qnc-program-segments` test prilagoden), ugovor `story.database.json` (`roll_edit`). B-roll kadar pokrivalice u
tabu B-roll zadrzava svoj izvorni raspon. Kod izvora 1080i u 50p prici neparan pomak zaokruzuje se na cijelu
sliku izvora.
Popravak 2026-10-11 (live: "Program source 'cover:...' range ends at 221, outside probed source duration 196"): roll je
pokrivalicu pomaknuo dalje u izvoru, pa je za svoj slot trazila slike iza kraja klipa i program se nije otvarao.
`qnc-program-db` (`check_covers_fit`): nakon pomaka markera ili slota svaka pokrivalica slota koji pocinje ili
zavrsava na pomaknutom markeru mora stati u svoj klip (IN + duljina slota omjerom price i izvora <= broj slika
klipa), inace se pomak odbija i nista se ne upisuje; vrijedi i za slot ispred markera koji se produljuje.
`qnc-content-read` `duration_frames_on` (jedini citac kataloga). Test.
Popravak 2026-10-11 (ista greska i nakon provjere u bazi: pokrivalica s izvorom 0..196 u slotu od 221 slike, nastala
prije provjere kraja): `qnc-program-input` (`fit_covers`): pokrivalica ciji klip zavrsi prije njezina slota pusta
se do kraja klipa, ostatak slota pokazuje segment ispod nje; pokrivalica bez preostale slike se ne pusta. Jedna
pokrivalica vise ne sprjecava otvaranje cijele price. Test.
Popravak 2026-10-11 (korisnik: "pomaknuti slotovi enterom se vracaju na pocetni polozaj, a ne snimaju novu poziciju"):
u novom projektu sve pokrivalice pocinju od prve slike svog klipa (IN 0), pa je pomak slota ulijevo trazio roll
prije prve slike i baza je odbijala cijeli pomak (poruka samo u statusu, nacrt se vratio). Sada roll ide koliko
izvor dopusta (IN ne ide ispod 0, pokrivalica se tada samo produlji), a pomak se nikad ne odbija zbog pokrivalice;
provjera kraja klipa iz prethodnog popravka je uklonjena (pokrivalica koja prelazi kraj klipa pusta se do kraja,
`qnc-program-input` `fit_covers`), a s njom i `qnc-content-read` `duration_frames_on`. `qnc-program-db`,
`qnc-program-view` (nacrt isto), testovi, ugovor `story.database.json` (`roll_edit`).
Popravak 2026-10-11 (korisnik: "puno se ceka pojava playheada na source timeline i nekih filmstrip"): privremeno
mjerenje koraka sucelja Storyja (uklonjeno) pokazalo je 2,9 s u ponovnom citanju virtualnih kadrova nakon promjene
price (`list_pool_shots` na niti sucelja ceka bazu dok pozadinski proces upisuje filmstrip/wave). Novi
`qnc-virtual-shots::PoolShotsReader` cita ih u pozadini (zahtjev usred citanja cita se jos jednom poslije), a
`qnc-editorial-application` ih samo trazi i preuzima (C10 708 -> 706). Priprema playera za vrijeme tog upisa i dalje
traje do ~3 s (citanje klipa ceka bazu, izvan sucelja). Svenk: na jednom rezu price (slika 1259) dekoder otvoren
unaprijed nije dao prvu sliku na vrijeme, player je cekao ~10 slika, HDMI je preskocio 4; ostalo 251/251. Otvoreno.
Uklonjeno 2026-10-11 (korisnik "ok, idemo po tvom planu": stabilizacija prije novog dekodera): "jedan izvor slika"
(`PublishStills`) u zivom radu nije dao nijednu sliku (player drzi samo slike oko mjesta gdje stoji, prozor reza
trazi daleke slike). Vraceni na stanje prije njega: `qnc-player-contract` (naredba i `StillRef`),
`qnc-broadcast-engine` (`held_pictures`, `held`), `qnc-broadcast-player` (`TransportEngine::video`),
`qnc-player-client` (frame-mapa `stills`, `publish_stills`, `take_stills`), `qnc-source-preview` (`ask_stills`,
`take_stills`), `tools/qnc-player-runner`; `qnc-trim-stills` vise ne pita player, `qnc-editorial-application` isti
redak poziva. Pozadinsko dekodiranje unatrag (`backfill.rs`) ostaje. Odobrenje "jedan izvor slika" je zatvoreno.
Sljedece (plan korisnika): mjerenje ugradenog dekodera (ffmpeg kao biblioteka, trajan, skok bez novog procesa) na
originalu u zasebnoj grani; QGS (`qgs-h264-422p10`) dijelovi slike u vise niti kasnije, zasebno.
Mjerenje 2026-10-11 (korisnik "nastavi"; plan: ugradeni dekoder prije integracije): probni program izvan
repozitorija (scratchpad `libav-spike`, `ffmpeg-next` 8 uz FFmpeg 8.1 shared, BtbN, i LLVM za veze; preuzeto uz
dozvolu korisnika u `C:\Users\miron\tools`), jedan dekoder ostaje otvoren na originalu Mironik 2679 s kartice (H.264
4:2:2 10-bit 1080p50, GOP 24, 8 dijelova slike), samo citanje. Proizvoljna slika (skok na kljucnu sliku i
dekodiranje do nje, bez novog procesa): medijan 107-119 ms, najgore 165-200 ms, najbolje 38-88 ms (danas kroz
`ffmpeg` naredbu oko 400 ms). Korak unatrag bez slika u memoriji 123-172 ms. Dekodiranje u nizu: 1 nit 71 slika/s,
4 niti 169-198 slika/s. Mjereno samo dekodiranje do YUV-a (bez pretvorbe i prikaza). Integracija u QNC nije
napravljena.
Otvoreno ograniceno odobrenje 2026-10-12 (korisnik: "da, LGPL i kreni u posebnoj grani"; "bilo bi dobro da dekoder
bude kocka kako bi se kocke mogle mijenjati"): grana `libav-decoder`. Prvi korak: nova kocka
`crates/qnc-libav-decode` (+ `contracts/modules/libav-decode.module.json`): video se dekodira u procesu playera kroz
FFmpeg biblioteke (LGPL shared 8.1, BtbN; `ffmpeg-next` 8), zvuk i sve sto ne preuzme ostaje na `ffmpeg` naredbi.
Kocka je novi adapter kataloga (`Driver::LibavV1`, `libav_v1`), pa je zamjenjiva kao i ostali dekoderi; gradi se samo
uz znacajku `libav` programa playera (`tools/qnc-player-runner`, `qnc-decoder-catalog`) i varijable `FFMPEG_DIR` i
`LIBCLANG_PATH`, izvan clanova workspacea (obicna gradnja i testovi ne trebaju razvojne datoteke). Program izgraden
bez znacajke tu registraciju dekodira istom `ffmpeg` naredbom. `qnc-media-decode`: adapter moze dati dekoder u
procesu (`PictureSource`, `DecoderAdapter::open_in_process`), a slike iz procesa i iz ugradenog dekodera prolaze iste
provjere ugovora (`pump`); `qnc-ffmpeg-decode` dijeli opis filtara (`video_filter`) i ime demuxera. Conformance:
nova kocka na popisu adaptera dekodera i u granici playera. Dokaz (`crates/qnc-libav-decode/tests/same_as_cli.rs`):
bajt za bajt iste slike, vremenske oznake i broj slika kao `ffmpeg` naredba (cijeli tok, skokovi, 4:2:0,
deinterlace, svaki field, pola brzine, stvarni original s kartice od cetiri mjesta). Izmjereno: kocka od otvaranja do
prve slike 170-260 ms (otvaranje MXF-a 65 ms), `ffmpeg` naredba 240-310 ms; u playeru skok koji ponovno otvara
dekoder 385 ms prema 460 ms. Katalog u repozitoriju nije promijenjen (mjereno s kopijom kataloga u scratchpadu).
Sljedeci korak: dekoder ostaje otvoren i skace unutar sebe (bez novog otvaranja), dijeljenje po dijelovima slike.
Popravak 2026-10-12 (proba korisnika s novim projektom): (1) novi projekt pusta proxy, a prerender pricu
prebacuje na originale mijenjajuci samo oznaku reprezentacije klipa, pa je provjera ulaza odbila program
(`InvalidDescriptor`) i radnik je tiho stao (izlaz pozadinskog procesa ide nikamo); mapa `products/prerender`
ostala je prazna. `qnc-player-input` `PreparedInput::with_original_picture` (ulaz, reprezentacija i raspored
streamova uskladeni kao za projekt koji pusta original, test), `qnc-broadcast-engine` (`original_pictures` ga
koristi; vrijedi i za slaganje jedinica u playeru), `tools/qnc-prerender` greske zapisuje u dijagnostiku
(`prerender stopped: ...`). Na kopiji: novi projekt 10 jedinica. (2) Klip: redni broj slike trosio se i pri
pretvorbi i pri ponovnom pokazivanju; nakon koraka unatrag svaka sljedeca slika je ponovno numerirana, pa je
broj rastao za 2 po slici i monitor je polovicu slika brojao kao preskocene (13 od 26 u svakom izvjestaju).
`qnc-broadcast-engine` (`stamped`): slika dobiva redni broj pri predaji, iz vlastitog brojaca (kao program).
Izvedeno 2026-10-12 (korak 2, grana `libav-decoder`; korisnik "nastavi"): dekoder u procesu ostaje otvoren i skace
unutar sebe. `qnc-media-decode`: `PictureSource::seek`, `Decoder::seek(start) -> bool` (nova generacija, paketi starog
prolaza se odbacuju; dekoder procesa vraca `false` i engine otvara novi kao prije), dekoder u procesu ostaje nakon
kraja toka. `qnc-libav-decode` `VideoSource::seek` (isti skok kao otvaranje na `start`: flush dekodera, filter graf
iznova). `qnc-broadcast-engine`: `reopen` prvo trazi skok u otvorenom dekoderu; predhod skoka 2 slike umjesto 25
(dekoderi skacu tocno i sami odbacuju slike prije pocetka; 25 je kostalo ~200 ms po skoku). Popravljeno curenje
memorije u kocki: izlaz filter grafa premjesta sliku u okvir bez oslobadjanja prethodne, pa je svaka predana slika
(~3 MB) ostajala u memoriji (120 skokova: 1,6 GB -> najvise 122 MB); zbog toga je CLI usporedba u punom skupu testova
povremeno padala bez memorije. Dokaz: 40 skokova na originalu s kartice daju bajt za bajt iste slike kao svjeze
otvaranje `ffmpeg` naredbom; svih 6 testova kocke prolazi zajedno. Izmjereno u playeru (izolirana kopija, original
2002): najgori skok 207-260 ms (CLI 456-490 ms), skok +40 73-81 ms (153-225 ms); play klipa 2/2 PASS. Repozitorijski
katalog nije promijenjen. Nije izvedeno: dekoderi price (Wrap) jos otvaraju novi dekoder, zvuk ostaje na `ffmpeg`
naredbi, kocka nije u zasticenoj kopiji aplikacije za zivu probu.
Popravak 2026-10-12 (proba kocke libav na izoliranoj kopiji): `qnc-player-acceptance` je kod playa povremeno
javljao sliku izvan reda (npr. 2199 nakon 2200), samo s kockom libav. Dekoder je bio ispravan (800 slika u redu i
bajt za bajt kao CLI, i nakon skoka); pogled preview-a se na ~2 ms vracao na prethodnu sliku: nit stanja playera
(`qnc-player-client` `publish`) zamijenila je cijeli pogled kopijom slike uzetom prije nego je nit slika objavila
noviju. Sada `publish` zadrzava noviju sliku iste sesije (`newer_picture`, test). Kocka je brza pa se utrka pokazala
cesce; ista utrka postoji i uz `ffmpeg` naredbu. Izmjereno (izolirana kopija test1234, `playback.input = original`,
originali s kartice, 4+4 pustanja): klip 2002 od 1500 bez slika izvan reda, 2 od 4 PASS (ostali 2-4 trzaja do 126 ms),
prica od 0 96-99 % slika s 3-6 trzaja; prije s `ffmpeg` naredbom klip 97-99 %, prica 87-93 %.
Popravak 2026-10-12 (ziva proba Storyja na izoliranoj kopiji, korisnik "pogledaj log"): sucelje Storyja se na
pokretanju Synca (Play) zamrzavalo 0,7-2,9 s, a na prvom Enteru pokrivalice 1,1 s; poslije su Sync i otvaranje klipa
cekali 1,6-2,8 s. Uzroci (izmjereno privremenim mjerenjem koraka, uklonjenim): (1) `qnc-source-preview` je bazu price
trazio na niti sucelja (`story_target`: registar i `project.db`); sada je trazi nit koja priprema player
(`open_story`). (2) Posrednik baze otvarao je bazu u niti koja ga pozove; `ProjectDbWriter::spawn` (`StoryWriter`,
`VirtualShotsWriter`) je otvara u svojoj niti, greska otvaranja je odgovor na prvi zahtjev; `start` ostaje za ostale.
(3) Slaganje programa citalo je postavke projekta dvaput po klipu; `qnc-player-input` `InputReader::session` ih cita
jednom za cijeli program i na kraju jednom provjeri da se nisu promijenile (`qnc-program-input`; Sync prozor 930 ->
430 ms, cijela prica 1150 -> 540 ms, 27 stavki). Flat playlista je ista. (4) Zastita direktorija projekta od brisanja
nasljedjuje se na datoteke, pa SQLite pri zatvaranju zadnje veze koja pise nije mogao obrisati `project.db-wal` i
ponavljao je oko 3 s drzeci bazu; svi su cekali. `qnc-db-broker` i `qnc-content-store` sada drze WAL datoteku
(`SQLITE_FCNTL_PERSIST_WAL`); zatvaranje 2926 -> 1 ms (probna baza sa istom zastitom). Project (zamrznut) i dalje
otvara bazu bez toga, samo pri kreiranju i otvaranju projekta. Nalaz: zapinjanje price na pokrivalicama danas nije od
koda nego od originala sa SD kartice (kopija je imala `playback.input = original`; kartica ~29 MB/s, original 15 MB/s,
na rezu se citaju 3-6 tokova); ista prica s proxyjem i danasnjim kodom 99,7-100 % slika, 0-1 zastoj.
Izvedeno 2026-10-12 (grana `libav-decoder`; korisnik: "potrazi neki novi nacin", "kopiranje nije opcija, za breaking
news", "da, kreni"): montaza na originalima izravno s kartice. Nalaz: kartica je u citacu Realtek USB 2.0 (~29 MB/s),
original XAVC 4:2:2 10-bit ~15 MB/s; zastoji price na pokrivalicama nisu bili od dekodera nego (1) od otvaranja
dekodera sljedeceg reza na niti playera (70-350 ms; vanjski `ffmpeg` 300-500 ms): svaki zastoj poklopio se u istoj
milisekundi s tim otvaranjem, i (2) od navala citanja kartice. Izvedeno: `qnc-media-decode` `SourceOpener` (dekoder u
procesu otvara medij u svojoj niti; greska otvaranja je prvi dogadjaj dekodera; `DecodePlan` je `Clone`),
`qnc-broadcast-engine` `DecodeInput::open_in_background` + `Opening` (u prici se dekoderi slike i zvuka otvaraju u
pozadinskoj niti; dok nisu spremni, prica ceka sliku/zvuk kao i do sada, nit playera ne stoji), nova javna kocka
`crates/qnc-media-readahead` (+ ugovor `contracts/modules/media-readahead.module.json`): jedan citac medija procesa,
jedna nit cita blokove na koje dekoder ceka (256 KiB), druga unaprijed u nizovima (1 MiB, do 32 MiB po toku, tok s
najmanje zalihe prvi), tokovi iste datoteke dijele blokove, memorija do 768 MiB i odlazi sa zadnjim tokom datoteke;
samo citanje. `qnc-libav-decode` cita medij kroz taj citac (`reader_io`: vlastiti AVIO), slike i dalje bajt za bajt
iste kao CLI (svih 6 testova). Zvuk ostaje na vanjskom `ffmpeg`-u (preskace sliku: 35 MB za 20 s zvuka; citac bi
citao cijelu datoteku). Izmjereno (izolirana kopija, `playback.input = original`, originali s kartice, prica od 2300,
30-40 s): prije 98 % slika i 10-11 zastoja (vanjski `ffmpeg` 94-95 %, 18-24); sada 99,4-99,9 %, 0-3 zastoja, najdulja
rupa 50-98 ms (bez citaca, samo s otvaranjem u pozadini: 98,4-99,5 %, 1-2 zastoja do 208 ms). Prica od pocetka
98,8-98,9 %, ostaje rupa u prvoj sekundi playa (do 183 ms). Pretvaranje u H.264 8-bit (9. 10.) ostaje kao izbor u
Projectu kad ima vremena za Uvezi.
Dopuna 2026-10-12 (ziva proba: zvuk zapeo 7 puta, slika skakala na skoro svakom rezu): citac je za tek otvorenu
pokrivalicu citao zalihu punom brzinom kartice i gusio vanjski `ffmpeg` zvuka; sada iznad 8 MiB zalihe cita najvise
20 MiB/s (`low_water`, `paced_rate`; pauza nakon upisa blokova, nitko ne ceka). Otvaranje u kocki libav: format iz baze
(kao `-f` u CLI-ju), analiza nakon otvaranja najvise 1 MiB / 0,1 s (prije 14-25 MB s pocetka datoteke); slike i dalje
bajt za bajt kao CLI (svih 6 testova), prva slika 160-200 ms. Topla kartica: prica na originalima 99,9 %, 2 zastoja;
hladna kartica: pocetak playa i dalje ceka citanje (prve sekunde trebaju >75 MB pri 29 MB/s).
Popravak 2026-10-12 (ziva proba: "prica nije dosla do kraja"): na frameu 4348 jedan korak playera trajao je 0,5 s, zvuk
je presusio usred koraka, a ta greska (za razliku od prekida nadjenog na pocetku koraka) zaustavljala je player.
`qnc-broadcast-engine`: prekid zvuka usred koraka ide istim putem kao na pocetku (`rebuffer_audio`: zvuk se ponovno
priprema i reprodukcija nastavlja). Uz to se i adresa medija (otvaranje datoteke, metadata, provjera putanje na
kartici) razrjesava u pozadinskoj niti dekodera (`DecodeInput::open_in_background`; opener je `Send`,
`tools/qnc-player-runner` `MediaOpener`). Izmjereno (originali s kartice, 25 s): od 4200 do kraja price 99,9 %, od
2300 99,4 % (2 zastoja), od 0 100 % PASS.

Otvoreno ograniceno odobrenje 2026-10-12 (prerender price u format predaje; korisnik: "da, uvijek ukljucen, kreni",
"da, otkljucaj oboje, nova mapa products/prerender"): dok se montira, prica se u pozadini renderira dio po dio (stavka
flat playliste) iz originala u format iz projektnih `export.*` postavki; player pusta gotove dijelove, export ih samo
spaja; prica u bazi ostaje istina (iz nje uvijek XML/EDL i export iz originala u bilo kojem formatu). Prerender je uvijek
ukljucen. Otkljucano: `crates/qnc-work-settings` (samo citanje `export` i `products.prerender`),
`crates/qnc-project-store` (samo dopuna proizvoda `products/prerender` za nove projekte), nove kocke rendera i njihovi
ugovori, `qnc-broadcast-engine` (opis jedinica iz plana price), root Cargo. Izvedeno (temelj): `WorkSettings.export`
(kako je spremljen), `ProductLocations.prerender` (neobavezno: projekt bez njega dobiva kontroliranu gresku samo za
prerender, `prerender_location`), Project novom projektu daje `products/prerender`.
Izvedeno (prerender, korak 1): nova javna kocka `crates/qnc-render-unit` (+ ugovor; opis jedne jedinice: slika kao
jedan niz izvora ili crno, po izlaznom kanalu jedan kanal izvora ili tisina, kako ih plan price pusta; otisak = sto
jedinica jest + format predaje, ne gdje stoji u prici), `qnc-broadcast-engine` `ProgramPlan::for_render` (isti plan,
slika iz originala) i `render_units` (jedinica izmedju svaka dva reza slike ili zvuka), nova javna kocka
`crates/qnc-program-render` (+ ugovor; dekodira kroz isti dekoderski sloj i katalog kao player, u punoj velicini,
kodira `ffmpeg`-om iz kataloga u format iz `export.*`: zasad H.264 i XDCAM MPEG-2 422 progresivno; isprepletena
predaja, druga stopa ili frekvencija jasna greska; zvuk PCM 24-bit; jedinica se pise pod konacnim imenom jer mapa
projekta ne dopusta preimenovanje), alat `tools/qnc-prerender` (prica iz baze kao player -> jedinice -> render u
`products/prerender`; `--list 1` ispis). Conformance: `qnc-render-unit` dopusten u granici enginea. Provjereno na
izoliranoj kopiji (originali s kartice): 4 jedinice, broj slika i uzoraka tocan (960 po slici), slika tocna (PSNR prema
originalu 40-41 dB na tocnom mjestu, 31-38 dB uz pomak od jedne slike), zvuk bit po bit jednak izvoru od tocnog uzorka
(oba izlaza). Brzina 0,4-0,7x stvarnog vremena (CLI dekoder, puna 4:2:2 10-bit slika kroz cijev, libx264 veryfast).
Nije izvedeno: zapis gotovih jedinica u bazu (korak 2), puštanje jedinica, export spajanjem, ubrzanje.
Izvedeno (prerender, ubrzanje; korisnik "prvo ubrzanje rendera"): mjereno na originalu kamere (300 slika): samo
dekodiranje 133 slike/s, s libx264 veryfast 33, superfast 70, Intel QSV 86 (tada granica dekodiranje). `qnc-program-render`:
H.264 predaja koristi hardverski koder koji na racunalu stvarno radi (`encoder.rs`, jednom po procesu probno kodiranje;
Windows QSV, NVENC, AMF, zatim libx264 superfast; macOS VideoToolbox), koder je dio otiska jedinice; dekoder za 4:2:0
predaju odmah daje 4:2:0 (trecina bajtova kroz cijev); zvuk se dekodira istodobno sa slikom, slika se kodira u
privremenu datoteku sustava, pa se spaja bez ponovnog kodiranja. `tools/qnc-prerender` ima znacajku `libav`. Izmjereno
(izolirana kopija, originali s kartice, 6 jedinica): 1,0-1,2x stvarnog vremena (prije 0,4-0,7x); fiksni trosak oko
0,6-0,8 s po jedinici; slika i dalje tocna (PSNR 39,8 dB na tocnom mjestu, kao libx264 crf 16).
Izvedeno (prerender, korak 2): nova javna kocka `crates/qnc-prerender-db` (+ ugovor; modul tablica posrednika:
`prerender_units` i pogled `public_prerender_units`: otisak, URI datoteke u mapi projekta, slike, uzorci, koder;
jedinica je gotova samo ako je ovdje), `tools/qnc-prerender` je pozadinski radnik: jedan po projektu (najam
`prerender_lease` u bazi), cita pricu kao player, renderira prvu jedinicu koje nema u bazi i upisuje je kroz posrednika;
dok player radi ceka (pita bazu svakih 100 ms, bez medjuspremnika), na Play prekida jedinicu i ne upisuje je, vraca se
tek kad je player miran 2 s; zavrsava kad se aktivni projekt promijeni ili nakon minute bez posla. Nova javna kocka
`crates/qnc-prerender-launch` (+ ugovor): budi radnika (najam i pokretanje u svojoj niti, nikad na niti suceljа);
`qnc-program-segments` ga budi pri otvaranju projekta i kad se spremljena prica promijeni (`configure` dobiva cilj
sadrzaja; Story aplikacija samo predaje cilj u postojecem pozivu, C10 nije narastao). Conformance: `qnc-prerender-db` na
popisu javnih DB vlasnika. Provjereno (izolirana kopija, originali s kartice): radnik upisuje jedinice i preskace vec
upisane; drugi primjerak odmah odustaje; uz radnika u pozadini prica od 4200 svira bez prekida zvuka (98 %, najdulja
rupa 122 ms), radnik se povukao cim je player poceo pripremu i nije se vratio izmedju "spreman" i Play.
Popravak 2026-10-12 (ziva proba: "nesto cudno zastaje kad sam napravio novu overwrite pokrivalicu"): nakon promjene
price radnik prerendera ponovno je renderirao zahvacene jedinice dok je korisnik korakom po slici pomicao marker u
Wrapu (player u pauzi, pa radnik nije cekao); karticu su istodobno citali radnik, slike prozora reza (25 slika 4,0 s
umjesto oko 1 s) i pozadinsko dekodiranje unatrag. `qnc-source-preview` sada javlja rad playera i dok korisnik
montira: jos 3 s nakon svakog koraka, skoka ili Playa (`touched`, `HANDS_ON`); radnik tada ceka i renderira samo u
pravim stankama.
Izvedeno (prerender, korak 3; korisnik "da, kreni"): player pusta gotove dijelove price. `qnc-program-render`: dio je
format `UNIT_FORMAT = 2` (MOV, slika stream 0 bez B-slika s vremenskom bazom stope price, PCM stream 1, BT.709 tv,
vremenske oznake od 0; provjereno: pts 0,1,2..., time_base 1/50), `unit_media` pise njegov opis (nikad probe), kljuc
formata (`key`: export, koder, verzija formata) ulazi u otisak. `qnc-prerender-db`: uz dio cuva kljuc i opis
(`key_json`, `media_json`; tablica starijeg razvojnog oblika cita se kao prazna i prvi upis je zamjenjuje), `all()`.
`qnc-player-input`: `ProgramInput.prerendered` (`PrerenderedUnit`). `qnc-program-input`: cijela prica dobiva gotove
dijelove iz baze (Sync prozor ne: privremena pokrivalica nije u prici). `qnc-player-launcher`: veze i za mapu projekta
kad prica ima gotove dijelove (bez veze prica svira iz izvora). `qnc-broadcast-engine`: `ProgramPlan::new` racuna
jedinice price kao prerender (`original_pictures`: slika i adrese slike iz originala, i kad projekt pusta proxy) i dio
s gotovim dijelom istog otiska pusta iz te datoteke (`InputPlan::for_unit`: slika stream 0, zvuk po izlaznim kanalima
iz streama 1), ostale iz izvora. Izmjereno (izolirana kopija, originali s kartice, 31 dio prerenderiran za oko 1 min
50 s): prica od 0 99,4 % (prije 98-99 % s prekidom zvuka na pocetku), od 2300 99,6 % (prije rupa 1,8 s), od 4200 100 %;
bez prekida zvuka; preostali kratki zastoji (do 153 ms) na prijelazima izmedju dijelova.
Popravak 2026-10-12 (proba: novi projekt na originalima "uopce nema pleya"): svih 98 zapisa ostalo je u fazi kamera
(`final = 0`), pa preview nijednog klipa nije pokrenuo player. Preview koji ceka dovrsetak zapisa svog klipa
(ponovno otvaranje svake sekunde, Play koji ceka spremnost) prijavljivao je rad playera, a pozadinski dovrsetak zapisa
(`qnc-record-completion`) ceka dok player radi: cekali su se medjusobno. `qnc-source-preview` (`report_activity`):
klip koji ceka svoj zapis ne prijavljuje rad. Rucno pokrenut `qnc-ingest-worker` na izoliranoj kopiji dovrsio je svih
98 zapisa i napravio 98 waveova.
Dopuna 2026-10-12 (korisnik: spremljena pokrivalica mijenja boju po trajanju, "ispod 2 s narancasta, do 3 zuta,
3-4 zelena, do 5 cijan, iznad 5 plava"; "ili je bolje brojcano napisati trajanje"): oboje. Pravila boja su u
`contracts/ui/editorial.layout.json` (`cover_lengths`: ispod 2, 3 i 4 s, do i ukljucivo 5 s, dulje), model u
`qnc-editorial-layout`; javna kocka `qnc-segment-panel` (`CoverLengthBand`, `cover_fill`, test) bira boju po trajanju
pokrivalice na stopi price i daje trajanje `sekunde:slike` (isti zapis kao trajanje segmenta); `qnc-timeline`
(`TimelineCoverSpan.fill`, `label`) samo crta zadanu boju i broj kad stane. Privremena pokrivalica Synca ostaje u boji
fokusa dok se ne spremi. Story ploca predaje pravila jednim argumentom postojeceg poziva.
Audit i dopuna 2026-10-13 (korisnik: "marker je OUT odabranog slota i IN narednog"; "duljine ostalih slotova ostaju
iste, znaci pomicu se"; "polozaj playheada odreduje koji slot je cilj"; "direktno Ctrl+I ili Ctrl+O"; "segment se
rjede dira, a slotovi su dio montaze"; v5 to nema): audit svih radnji s markerima i segmentima (baza `qnc-program-db`,
upravljac `qnc-program-segments`, tipke `qnc-panel-focus` i katalog). Izvedeno: na panelu Segmenti Ctrl+I uzme
pocetak, Ctrl+O kraj slota pod Wrap playheadom; strelice mijenjaju sliku na uzetom rubu, gore/dolje kljucna slika,
Enter zapisuje novu operaciju `TrimSlot` (jedan Undo korak), Esc odustaje. Ostali slotovi zadrzavaju duljinu: svi
markeri od kraja slota pomicu se za isto, zadnji slot price uzima razliku (zadrzava barem jednu sliku; zadnji slot
sam se ne skracuje ovim putem nego pomakom svog pocetnog markera). Na pocetku slot ostaje na mjestu, a njegova
pokrivalica pocinje ranije ili kasnije u izvoru (koliko izvor dopusta). Prozor reza pokazuje sliku prije i poslije
uzetog ruba (`qnc-program-view` `trim_stills`). Rub segmenta preselio je na Ctrl+Shift+I / Ctrl+Shift+O (nove akcije
kataloga `select_segment_start`, `select_segment_end` u svim presetima); gumb Edit ostaje. Popravak iz audita:
iskljucenje segmenta s markerom na prvoj i zadnjoj slici ostavljalo je dva markera na istoj slici; sada ostaje jedan
(`merge_coincident_markers`), pokrivalice susjednih slotova ostaju, pokrivalica iskljucenog dijela odlazi. Ugovor
`story.database.json` (`slot_trim`, `coincident_markers`). Otvoreno iz audita: premjestanje segmenta gore/dolje ne
pomice markere i pokrivalice (ceka odluku), baza ne provjerava susjede pri pomaku markera (provjerava upravljac),
pomak markera pomice Wrap playhead, a pomak slota ne. Testovi `qnc-program-db` (30), `qnc-program-segments` (36),
conformance.
Dopuna 2026-10-13 (korisnik: "u zoom prikazu da bude oznacena slika koja prikazuje polozaj reza, okvir crveni"):
`qnc-program-view` `EditStill.cut` (`at_cut`): slika na rezu koji se pomice (rub slota: nova prva ili zadnja; marker:
prva slika poslije njega; slot: njegova prva i zadnja; rub segmenta: nova prva ili zadnja segmenta), `qnc-trim-overlay`
je crta s crvenim okvirom. Nalaz iz loga: dok se rub slota pomicao strelicama, prerender je citao karticu (rub slota ne
mice playera pa se nije brojao kao montaza), slike reza 2,3-3,4 s po prolazu. `qnc-source-preview` `hands_on`, a
`qnc-panel-focus` ga zove za svaku tipku montaze: prerender ceka jos 3 s nakon svake.
Dopuna 2026-10-13 (korisnik: "kad klikne na slot on je oznacen; Ctrl+I ili Ctrl+O playhead skoci na te pozicije";
izabrao "bolju verziju"): Ctrl+I/O uzimaju rub kliknutog (oznacenog) slota, inace slota pod Wrap playheadom; Wrap playhead
skace na prvu (Ctrl+I) ili zadnju (Ctrl+O) sliku slota pa monitor odmah pokazuje rez iz playera; strelice i dalje mijenjaju
nacrt (player svira spremljenu pricu, nacrtne slike pokazuje zoom); nakon Entera playhead stoji na novom rubu.
`qnc-program-segments` (`slot_trim.rs`, test).
Popravak 2026-10-13 (korisnik: "slika susjednog klipa se sporo pojavljuje... presporo se prikazuju"): log je pokazao da
svaka slika zooma ceka cijeli prolaz dekodera od 25 slika (1,4-2,2 s) i da dvije slike idu jedna za drugom (druga
nakon 3-3,8 s). `qnc-trim-stills`: slika koja nedostaje prvo se dekodira sama, sve takve istodobno (jedan dekoder po
slici), a tek zatim prolazi sa susjedima za strelice. Izmjereno na kartici (topla): jedna slika 430-690 ms, prolaz od
25 slika 560-660 ms; vecinu vremena nosi pokretanje ffmpega i dekodiranje od kljucne slike, ne broj slika.
Dopuna 2026-10-13 (korisnik: "ok, napravi kako si planirao"; korak 1 od 2): slika na rezu u zoomu dolazi s monitora, bez
dekodiranja. `qnc-source-preview` `shown_program_picture` (slika price na monitoru s brojem slike, ne Sync prozor),
`qnc-program-segments` `stored_picture_at` (koju sliku klipa spremljena prica pokazuje na tom mjestu), `qnc-trim-stills`
je uzima samo kad je zoom bas tu sliku trazi (za vrijeme playa nista se ne kopira). Aplikacija predaje to u postojecem
pozivu (C10 isti). Korak 2: dekoder koji ostaje otvoren (libav u procesu) za ostale slike zooma.
Dopuna 2026-10-13 (korak 2 od 2): slike zooma iz dekodera koji ostaje otvoren. `qnc-trim-stills` `open_run.rs`: jedan
dekoder kataloga po klipu (najvise 4), u procesu aplikacije kad program ima ugradeni dekoder; slike cita redom ili skace
unutar dekodera (bez novog ffmpeg procesa po slici), trazi `yuv420p` i sam je smanjuje na 480x270 (BT.709). Program bez
ugradenog dekodera, ili dekoder koji javi gresku, koristi dosadasnji izvlakac (`decode_run`), pa mjesto u zoomu nikad
ne ostaje crno. `qnc-source-preview` `LocatedPicture` nosi spremljeni zapis medija, indeks video toka i pecat datoteke.
`apps/qnc-app` i `apps/qnc-story` imaju znacajku `libav` (kao player: `FFMPEG_DIR`, `LIBCLANG_PATH`); izdanje se gradi s
njom. Ciscenje diska (korisnik: "izbrisi nepotrebne privremene datoteke i nepotrebnu povijest"; disk je imao 1,2 GB
slobodno pa povezivac nije mogao pisati): `cargo clean --profile dev` (34,6 GiB), privremene datoteke starije od dana i
stari `QNC/data/diagnostics/player.log` (do 12. 10.); izvorni kod, git povijest, projekti i `target/release` nisu dirani.
Dopuna 2026-10-13 (korisnik: "ako je klip dosao na kraj, bilo in ili out, treba dobiti crvenu liniju na onoj strani gdje je
dosao do kraja"): `qnc-program-view` `mark_clip_edges` / `ClipEdge`: slika zooma na prvoj ili zadnjoj slici svog klipa
dobiva oznaku strane, a slika iza kraja klipa vraca se na zadnju (log: zoom je svakih ~0,35 s trazio sliku iza kraja
klipa); duljine klipova daje postojeci `view_with_waves`. `qnc-trim-overlay` crta crvenu liniju na lijevoj (pocetak) ili
desnoj (kraj) strani slike, `qnc-trim-stills` ne dekodira unaprijed iza ruba klipa. Test.
Popravak 2026-10-13 (log probe): zoom je i dalje dekodirao unaprijed slike iza kraja klipa (do 50 slika ispred, 302-317
klipa od 302 slike, svaka ~0,4 s). `EditStill.clip_frames` (iz `mark_clip_edges`), `qnc-trim-stills` ne trazi slike
od kraja klipa nadalje.
Dopuna 2026-10-13 (korisnik: "ako je doslo do kraja, ne smije vise prihvacati strelicu u tom smjeru"; "skok moze biti do
kraja klipa"; oznaka kraja je siroka crvena traka, 1/16 slike, najmanje 14 px): `qnc-program-segments` pamti duljine
klipova iz `view_with_waves` i oznacava rubove u svakom `refresh_view`; strelica u smjeru ruba ne radi nista i javlja
"Klip je na kraju" (rub slota: njegova slika; marker: obje slike uz njega; pomak slota: slike susjeda, njegova pokrivalica
ide s njim); pomak koji bi presao kraj (skok na kljucnu sliku) se ponistava, a skok ide slika po slika do zadnje slike
klipa. `EditStill.past_end`. Testovi.
Dopuna 2026-10-13 (korisnik: "dodaj brisanje starih dijelova prerendera"; mapa `products/prerender` projekta imala je
1,1 GB, svaka izmjena price ostavljala je stare dijelove): `qnc-prerender-db` operacija `Forget` (`forget`, test);
`tools/qnc-prerender` kad su svi dijelovi price gotovi (jednom po stanju price) brise iz baze dijelove kojih prica vise
nema, pa njihove datoteke i ostatke nedovrsenih rendera u mapi prerendera (samo datoteke imenovane otiskom). Mapa
projekta zasticena je od brisanja i za datoteke (ACL deny DE/DC), pa se datoteka koja se ne moze obrisati isprazni na
0 bajtova (prostor je slobodan; isti otisak kasnije se pise preko nje). Nakon Undo stari dijelovi se renderiraju ponovno.
Ispravak isti dan (korisnik: "ne svida mi se... Undo ne s datotekom nego s XML ili EDL, a naziv treba brisati, ne samo
vratiti na 0"): datoteka se brise s nazivom ili ostaje netaknuta (nikad se ne prazni), log kaze koliko ih je zastita mape
zadrzala. Undo vec radi iz opisa price u bazi (`story_undo`), ne iz datoteka. Nalaz: Project postavlja zastitu
`icacls /deny korisnik:(OI)(CI)(DE,DC)` koja se nasljeduje na sve datoteke projekta (zato se ni stari dijelovi ni WAL ne
mogu obrisati); predlozeno je da `products/prerender` bude mapa predmemorije (sadrzaj se smije brisati, mapa ne) -
ceka otkljucavanje Projecta.
Zatvoreno ograniceno odobrenje 2026-10-13 (korisnik: "da" na otkljucavanje Projecta za zastitu mape prerendera; "s
obzirom da baza usporava montazu treba pronaci bolje rjesenje"): otkljucano i izvedeno samo `crates/qnc-project-store`.
(1) Zastita od brisanja ostaje na cijelom projektu (`icacls /deny korisnik:(OI)(CI)(DE,DC)`), a mapa predmemorije
`products/prerender` dobiva izricitu dozvolu brisanja za svoj sadrzaj (`(OI)(CI)(IO)(DE)`; blizi predak ide prije
naslijedene zabrane): stari dijelovi se brisu s nazivom, mapa se ne moze obrisati, ostatak projekta i sam projekt ostaju
zasticeni (provjereno probom i testom). (2) Zastita se postavlja samo kad je nema: Project ju je pri svakom pokretanju
skidao i ponovno postavljao na svakom registriranom projektu, a nasljedivanje je Windows prolazio kroz sve datoteke
(prvi prikaz kartice Project 820-1045 ms, sada 155 ms; §3: otvaranje ne smije pokretati masovni ACL posao). (3) Project
otvara bazu s `SQLITE_FCNTL_PERSIST_WAL` kao ostali vlasnici (zastita mape ne dopusta brisanje WAL datoteke; SQLite je
ponavljao ~3 s i drzao bazu). Test `the_files_of_the_prerender_cache_may_be_deleted_the_rest_stays_protected`; 41 test
`qnc-project-store`, conformance. Odobrenje je zatvoreno, Project je ponovno zamrznut.
Popravci 2026-10-13 (korisnik: "b, i kreni s oba popravka"; probom nadjeno: player price zapne na 1901, kratak crni
dio 1896-1922; pokrivalica "unistena"): (1) Pokrivalica nije bila unistena pri pomaku ruba: oko 12:13 marker dodan usred
pokrivalice (v5 pravilo) ostavio ju je samo prije markera, a ostatak slota prazan; ripple pomaci su taj prazni slot
gurnuli preko kraja Off segmenta (crno). Odluka korisnika (b): `qnc-program-db` `split_cover_at` - novi marker usred
pokrivalice dijeli je na dvije, druga istog klipa i kadra nastavlja od markera (slika se ne mijenja), jedan Undo
korak; ugovor `story.database.json` (`cover_split`), test. (2) `qnc-broadcast-engine` (`program.rs`): nakon skoka
natrag na sliku koju ima samo pozadinsko dekodiranje unatrag, glavno dekodiranje ostalo je gdje je bilo (2116), pa
slika iza pozadinskog niza nikad nije napravljena i prica je stala; sada dekodiranje krece odmah iza tog niza. (3)
`qnc-program-render`: dijelovi s indeksom na pocetku datoteke (`+faststart`; dugi dio s indeksom na kraju otvarao se
4,6 s), `UNIT_FORMAT` 3 (postojeci dijelovi se jednom renderiraju ponovno, stari se brisu).
Dopuna 2026-10-13 (korisnik: "kad zaustavim play odmah kreira pokrivalicu, ne dozvoljava skracivanje ili produljivanje
strelicama ni nastavak playem; jedino Enter formira pokrivalicu, a Delete brise ako Enter nije odraden"): Sync/B-roll
vise nista ne zapisuje sam. Pauza (player potvrdi sliku-dvije ranije, sto se citalo kao povratak na pocetak prozora)
ni dolazak do M markera ili kraja izvora ne zatvaraju slot: Sync stane i drzi nacrt kraja. Strelice (dok player stoji,
na svakom panelu) pomicu kraj za sliku, Space nastavlja od kraja u novom prozoru, Enter zapisuje marker i pokrivalicu,
Delete odbacuje nacrt bez ikakvog zapisa. `qnc-sync-cover` (`draft_end`, `nudge`, `resume`, `at_end`, `discard`;
povratak na pocetak prozora tek pri skoku vecem od 3 slike; testovi), `qnc-program-segments` (`sync_nudge`,
`discard_sync`, `sync_drafting`), `qnc-panel-focus`. Zamjenjuje pravilo od 1. 10. ("Sync zapisuje pokrivalicu kad dode
do M markera ili OUT-a izvora").
Popravak isti dan (korisnik: "nakon Entera sljedeca pokrivalica u Sync/B-roll vraca se na pocetak upravo kreiranog slota";
log: cetiri Sync prozora zaredom od 0): zaustavljen Sync drzi Wrap playhead na zadnjoj slici slota, a novi Sync krece od
markera na ili prije playheada, dakle od pocetka tog slota. Nakon Entera Wrap playhead ide na krajnji marker slota.
Dopuna 2026-10-13 (korisnik: "nepokriveni slot ne bi smio gurati slotove iza njega ako se produljuju slotovi ispred njega"):
`TrimSlot` pomice markere samo do prvog slota bez pokrivalice iza uzetog slota; taj prazni slot preuzima razliku (zadrzava
barem jednu sliku), slotovi iza njega ostaju. Bez praznog slota razliku i dalje preuzima zadnji slot price.
`qnc-program-db` (`shift_markers_between`, testovi), `qnc-program-segments` (nacrt `SlotTrim.empty`), ugovor `slot_trim`.

Dopuna 2026-10-13 (korisnik: "nema prikaza da je Overwrite kliknut"; "trebamo shortkate za Overwrite i druge koji jos
nisu dobili"; "Shift+ je dodavanje necega"; "SubClip je razumljivije"; "otkljucaj conformance za iznimke"; "sada cemo se
zadrzati na nasim QNC shortkatima, ostale cemo dodati na kraju"): QNC preset (Story) po univerzalnom pravilu (Ctrl+ uzima,
Shift+ dodaje, Alt+ skace): Shift+S novi SubClip (`save_virtual_shot`), Shift+T / Shift+V Talking Head / Voice over
(kao prije), B Cover slot, Shift+O Overwrite (`overwrite_cover`, prije Shift+B), P B-roll, Y Sync/B-roll, Alt+A / Alt+S /
Alt+B / Alt+G tabovi All, SubClip, B-roll, Segment (nove akcije kataloga `editorial_tab_*`); Shift+S vise ne skace na
prazni slot (do njega vode Alt+strelice i Sync/B-roll). Tablica tipki zna A, P i G (`qnc-keyboard-shortcut`). Cover slot i
Overwrite pri kliku ili tipki bljesnu 0,4 s (`qnc-segment-panel` `flash_button`), a status 3 s kaze sto je napravljeno
(`qnc-program-view` `SegmentsView.cover_pressed`, `qnc-program-segments`). Story layout: tab Virtual -> SubClip, gumb
"Add virtual clip" -> "Add SubClip", prekidac "Pokrivalice | Sync/B-roll" -> "B-roll | Sync/B-roll"; Media Assist gumbi
i poruka praznog taba nisu dirani. `tools/qnc-conformance`: iznimke od qnc_v4 tipki samo s popisa `V4_KEY_EXCEPTIONS`
(QNC preset: `overwrite_cover`, `focus_empty_slot`); iznimka koja vise ne odstupa je greska, test. Ostali preseti
(Edius, Resolve, Premiere, Final Cut, Avid) ostaju kako jesu; plan za kasnije: originalni preseti tih programa i kocka
prevoditelj njihovih akcija u QNC akcije.
Dopuna 2026-10-13 (korisnik: "moramo rijesiti TAB setnju po panelima unutar forme i izbor formi kroz QNC desktop tab";
"imamo dva nacina, TAB i Alt+"; "TAB: Media pool tabovi -> Media pool -> source timeline -> grupa dugmadi u source timeline
panelu -> Segment panel"; "to je vezano za forme, ne mora biti univerzalno"): audit (v5 `story/focus.rs` ima Tab kruzenje
Media pool -> Segmenti -> Source; Ingest i Project ga nemaju; izbor formi u v5 i QNC-u samo misem) pokazao je da egui
prvim Tabom daje fokus gumbu ili kartici, a tada su sve tipke kataloga mrtve (i u Ingestu). Izvedeno: (1) nova javna kocka
`crates/qnc-key-focus` (+ ugovor `contracts/modules/key-focus.module.json`): svaki frame prije citanja tipki vraca egui
fokus svakom widgetu osim polja za upis teksta; poziva je `qnc-key-intents`, pa vrijedi za Story, MA i Ingest (Project
ne cita tipke kroz nju). (2) Redoslijed Tab-a je svojstvo forme: `contracts/ui/editorial.layout.json` svaka grupa ima
`tab_order` (Story: pool_tabs, pool, source_timeline, source_buttons, segments; MA isto bez segmenata),
`qnc-editorial-layout` (`focus_names`, `FOCUS_PANELS`, provjera imena). `qnc-panel-focus`: paneli `PoolTabs` i
`SourceButtons`, `FocusLayout`, `FocusView`, Tab/Shift+Tab kruze redom forme (panel bez sadrzaja se preskace: tabovi,
dugmad, Segmenti bez segmenata), pocetak na prvom panelu; na tabovima poola strelice lijevo/desno biraju tab, klik ili
Alt+slovo taba stavljaju fokus na tabove; na dugmadi docka strelice biraju dugme (okvir fokusa), Enter ga pritisne kao
njegovu akciju kataloga (`take_pick`). `qnc-editorial-application` (`new(root, FocusLayout)`, neiskoristen
`SwitchLibraryTab` uklonjen), `qnc-editorial-desktop` (`focus_layout` umjesto neiskoristenog `group`),
`qnc-editorial-blocks` (okvir fokusa na tabovima i dugmetu; stilovi docka, timelinea i gumba premjesteni u
`qnc-editorial-layout`). C10 spusteno: application 706 -> 703, blocks 379 -> 354, desktop 88 -> 86. Nije izvedeno:
strelice i Enter po karticama Media poola (fokusirana kartica, gore/dolje za red), Alt+broj za izbor forme u traci desktopa.
Dopuna 2026-10-13 (korisnik: "Enterom se slijedi krug: tab All -> Enter -> media pool (strelicama odabrati klip) -> Enter
-> source timeline (odabrati IN/OUT) -> Enter -> grupa dugmica (strelicama odabrati akciju) -> Enter -> segment panel;
naravno moze se ubrzati direktnim pozivanjem dostupnih akcija"): Enter bez onoga sto bi potvrdio vodi na sljedeci panel
kruga forme. Tabovi poola -> pool; u poolu strelice pomicu fokusiranu karticu (gore/dolje za red), Enter je otvara kao
klik (izbor klipa vodi na source timeline); na source timelineu Enter najprije potvrduje uzetu traku, Edit segmenta, Sync
ili pusta uzeti IN/OUT, inace ide na dugmad; na dugmetu Enter pritisne akciju i ide dalje; na panelu Segmenti Enter
najprije potvrduje uzeti marker/slot/rub slota, inace ide na tabove poola. Segment panel je u krugu i bez segmenata.
`qnc-panel-focus` (`advance`, `PoolStep` = `qnc_media_card::GridStep`, `FocusView.pool_step`, `FocusView.button` je
akcija dugmeta), `qnc-media-card` (`show_card_grid_focused`, `step_focus`, `GridStep`: fokusirana kartica u memoriji
mreze, pomak, pomicanje pogleda do nje, Enter = klik; `show_card_grid` isti za Ingest), `qnc-editorial-blocks` (mreza
s fokusom). Katalog: Enter vise ne sprema virtualni kadar (QNC preset, scope `off`; iznimka od qnc_v4 na popisu
`V4_KEY_EXCEPTIONS`); SubClip je Shift+S ili dugme Add SubClip, s bilo kojeg panela.
Dopuna 2026-10-13 (korisnik: "dodati okvir oko odabranog klipa da korisnik vidi gdje je fokus"): kartica na kojoj je
tipkovnica u Media poolu ima deblji okvir boje fokusa (3 px), a odabrani klip zadrzava svoj crveni okvir, pa se vidi i
gdje je fokus i koji je klip izabran. `qnc-media-card` (`show_card_grid_focused` prima boju fokusa, `KeyFocus`);
`show_card_grid` (Ingest) se ne mijenja.
Dopuna 2026-10-13 (korisnik: "treba dodati i strelice gore dolje i UP/DOWN"): strelice gore/dolje u Media poolu vec pomicu
fokus za red; nove akcije kataloga `page_prev` (Page Up) i `page_next` (Page Down), vezane u QNC presetu, pomicu fokus
za ekran redova (na prvi ili zadnji red u istom stupcu kad ekrana nema). Tablica tipki zna PageUp i PageDown
(`qnc-keyboard-shortcut`), `qnc-media-card` (`GridStep::PageUp`, `PageDown`, `step_focus` s brojem redova na
ekranu), `qnc-panel-focus`.
Popravak 2026-10-13 (korisnik: "strelice gore dolje ne rade u media poolu... funkcije treba vezati za aktivni panel, a ne
globalno za formu"): klik na karticu u Media poolu (kao v5) prebacivao je tipkovnicu na source timeline, pa su strelice
radile tamo. Sada klik bira klip i tipkovnica ostaje u poolu na kliknutoj kartici; na source timeline vodi samo Enter
(ili Tab, klik na timeline). `qnc-panel-focus` (`chose_clip`, `opening`), `qnc-media-card` (klik pomice fokus na
kliknutu karticu; izvan fokusa pocinje od izabrane), `qnc-editorial-application` (C10 703 -> 700).
Popravak 2026-10-13 (korisnik: "i dalje ne radi strelica gore dolje"; "ocito trebas vezati na fokus panela"): strelica
gore/dolje salje dvije akcije kataloga (`step_prev_part` iz scopea storyboard i `step_prev_segment` iz scopea off), a
svaka akcija ponovno cita stanje fokusa; druga je brisala pomak kartice prije crtanja. Pomak u poolu sada nosi redni
broj (`FocusView.pool_step`), ne brise se citanjem, a mreza kartica primjenjuje svaki broj jednom
(`qnc-media-card` `show_card_grid_focused`). Audit svih tipki QNC preseta je u tijeku.
Popravak 2026-10-13 (korisnik: "selektirani klip ne zadrzava crveni okvir"; "crveni se nije vidio ni prije ni sada"):
okvir kartice crtao se prvi, a slicica i red s tekstom preko njega cijelom sirinom; `qnc-media-card` sada crta okvir
zadnji (vrijedi i za Ingest), a okvir tipkovnice ide izvan kartice. Dugmad docka su dio source timeline panela (korisnik:
"prelaskom enterom na grupu gumba ne bi se smio gubiti fokus source timeline panela"): okvir panela ostaje, I/O, Ctrl+I/O
i Alt+strelice rade i na dugmadi, Ctrl+I/O vraca strelice timelineu (`qnc-panel-focus`, `qnc-editorial-blocks`).
Otkljucano i izvedeno 2026-10-14 (korisnik: "tab nam ne radi na ingestu"; "otkljucaj.. predlozi postujuci pravila koja
smo postavili"): Ingest dobiva isti krug kao Story. Nova javna kocka `crates/qnc-focus-walk` (+ ugovor
`contracts/modules/focus-walk.module.json`): krug panela po imenima iz ugovora rasporeda, Tab/Shift+Tab, Enter potvrduje i
bez potvrde vodi dalje, tabovi poola, dugmad docka, koraci po karticama s rednim brojem, klik u panel stavlja fokus na
taj panel (popis klikova u ugovoru), forma bez kruga propusta sve tipke kao prije; ne zna za aplikaciju.
`contracts/ui/ingest.layout.json`: `tab_order` (preglednik izvora, tabovi poola, kartice, source timeline, dugmad docka)
i `focus_clicks`; `qnc-ingest-layout` (`focus_names`; Ingest ugovor nabraja dugmad slijeva nadesno). Katalog (QNC preset,
scope `ingest`): Tab, Shift+Tab, Enter, Shift+Enter (nova akcija `toggle_focused_item`: kvacica izbora na kartici u
fokusu, jer Shift+ dodaje), strelice gore/dolje, Page Up/Down, Alt+A / Alt+S tabovi. `qnc-media-card` (`GridStep::Toggle`).
`qnc-ingest-application` (`with_focus`, kocka u `dispatch`, `IngestViewModel.focus`; posrednik `PlaybackGuard` uklonjen,
pozivi idu ravno u `qnc-playback-priority`), `qnc-ingest-desktop` (predaje redoslijed iz ugovora), `qnc-ingest-blocks`
(okvir fokusa na panelima i dugmetu, mreza kartica s fokusom; tri ista bloka dugmadi u jednoj petlji). C10 spusteno:
application 1707 -> 1705, blocks 882 -> 863, desktop 79 -> 76. Nije izvedeno: strelice i Enter po mapama u pregledniku
izvora (u prvom koraku preglednik je samo u krugu).
Odluka i izvedba 2026-10-14 (korisnik: "idemo sve na engleski, odnosno ENG rjecnik.. bilo bi najbolje da imamo kodove za sve
kako bi mogli jednostavno birati jezik"; "postavka racunala, otkljucaj Project, kreni s Ingestom"): sucelje je na engleskom,
svaki natpis ima sifru, jezik je postavka racunala (`ui-language.json` uz ostale postavke racunala u `data`, `{"language":
"en"}`; bez nje osnovni jezik, engleski), ne projekta. Nova javna kocka `crates/qnc-ui-text` (+ ugovor
`contracts/modules/ui-text.module.json`): rjecnici `contracts/text/<jezik>.json` (prvi `en.json`), ugovori rasporeda
pisu tekst kao `@sifra` (`resolve_contract`), kod kao `qnc_ui_text::text("sifra")` / `fill("sifra", ...)`; sifra koje
nema je greska (testovi: svaka sifra iz ugovora rasporeda i iz koda postoji u svakom rjecniku), nikad izmisljen tekst.
Ingest: svi natpisi ugovora (`contracts/ui/ingest.layout.json`) i poruke `qnc-ingest-application` (status docka,
greske, "Select a clip") idu kroz rjecnik; tab Virtual je SubClip. Uklonjeno dugme "Generiraj postere" i akcija
`ingest_approve_proxy_posters` (posteri dolaze s kartice ili ih pozadinski radnik napravi sam). Novi krug Ingesta
(korisnik: "najprije se bira gdje je izvor: racunalo, lan, intranet, zatim izbor direktorija, pa odaberi ili odustani,
pa media pool, select ili deselect, pa timeline, pa dugmad timelinea; All ili Virtual i druga dugmad direktno"):
`tab_order` = izvori, mape, Select/Cancel, kartice, source timeline, dugmad docka; `focus_rows` i `focus_clicks` u
ugovoru. `qnc-focus-walk` prepisan na opce redove (strelice biraju, Enter pritisne i ide dalje) i popise (strelice,
Page Up/Down, Enter otvara, Shift+Enter kvacica; `list_focus`, `list_keys`). Mape: strelice, Enter ulazi u mapu.
Izravne tipke (QNC preset, scope `ingest`): Alt+1/2/3 izvor, Alt+D diskovi, Backspace gore, Ctrl+Enter Select (bez URI-ja
uzima mapu koju preglednik pokazuje), Esc Cancel, Ctrl+A Select all, Ctrl+Shift+A Clear, Shift+I Import, Alt+N New/All
(bez filtra prebacuje), Alt+A/Alt+S tabovi. `qnc-ui-kit` `FormActionBarResponse` daje i polozaje dugmadi (dodatak).
Tablica tipki zna D, N, 3. C10 spusteno: application 1705 -> 1676, blocks 863 -> 862. Nije izvedeno: poruke zajednickih
kocki koje Ingest pokazuje (preglednik izvora, Select, player, `qnc-playback-priority`), Story/MA, shell, Project i
katalog tipki (nazivi akcija, F1).
Popravak 2026-10-14 (korisnik: "ne radi play u ingestu i ne ide na timeline"): `qnc-focus-walk` je imao jedan zadnji
korak za sve popise, pa je Enter iz preglednika mapa, kad tipkovnica dode na kartice, kartici bio "Enter" i otvarao klip;
korak sada vrijedi samo za popis na kojem je nastao (`step_on`, test). Proba kruga na stvarnom Ingest ugovoru: klik na
karticu, Enter, Tab i Space idu ispravno (Space ide playeru na svakom panelu). Log probe nije pokazao zasto Space nije
pustio klip, pa `qnc-ingest-application` uz dijagnostiku playera zapisuje svaku naredbu (`ingest-dispatch`: akcija,
ishod kruga, panel) i svaku odbijenicu (`ingest-refused`). C10 isti.
Izvedeno 2026-10-14 (korisnik: "da, kreni sa Storyjem"): Story i Media Assist kroz rjecnik. `contracts/ui/editorial.layout.json`
(tabovi poola, gumbi docka, prazne poruke, IN/OUT/Duration, Auto/On) pise sifre, `qnc-editorial-layout` ih razrjesava
pri ucitavanju (`resolve_contract`); MA gumb "Pokrivalice" je "B-roll" (engleski, isti kao Story). Poruke i natpisi koda
kroz `qnc_ui_text::text`/`fill`: `qnc-editorial-application`, `qnc-program-segments` (statusi Wrapa, markera, slotova,
Synca), `qnc-program-view` (nazivi slika prozora reza), `qnc-segment-panel` (traka, zaglavlje, popis segmenata,
savjeti), `qnc-sync-cover`, `qnc-source-mark-focus`, `qnc-keyframe-nav`, `qnc-trim-stills`, `qnc-shot-stills`,
`qnc-program-input` (greske programa s umetnutim klipom), te zajednicki `qnc-source-dock` ("Duration", vidi se i u
Ingestu) i `qnc-timeline` ("not in the program"). `qnc-program-segments` je kraj klipa prepoznavao po hrvatskom dijelu
poruke ("na kraju"); sada usporeduje s tekstom sifre `story.msg.clip_at_end`. Testovi koji su ocekivali hrvatski tekst
uzimaju isti tekst kroz sifru. Vrijednosti u bazi (`tonovi`/`offovi`) i dijagnostika nisu prevodeni. Nije izvedeno:
`qnc-source-preview`, `qnc-monitor` i ostale zajednicke kocke, Project, shell, katalog tipki.
Popravak 2026-10-14 (korisnik: "story add subclip ne kreira poster"): baza probne kopije: zadnji SubClip (IN 1, OUT 335)
ima `still_status = failed`, "IN/OUT slike nisu uhvacene". Slika se hvatala samo u trenutku tipke I/O; IN ili OUT
pomaknut kasnije (uzet i pomaknut strelicama, prilagoden slotu, povucen) vise nije odgovarao uhvacenoj slici i poster nije
nastao. `qnc-virtual-short-stills` `follow`: slika IN-a i OUT-a je slika koju monitor pokazuje na tom mjestu, kako god je
oznaka tamo dosla; novija slika istog mjesta zamjenjuje stariju (test). `qnc-source-preview` `shown_clip_picture` (slika
klipa na monitoru s klipom i brojem slike, ne slika price). `qnc-editorial-application` jedan poziv (C10 isti).
Ostaje: oznaka koju monitor nikad ne pokaze (npr. OUT postavljen prilagodbom slotu bez odlaska na njega) i dalje nema
sliku i SubClip ostaje bez postera.
Izvedeno 2026-10-14 (korisnik: "nastavi sa zajednickim kockama"): poruke i natpisi 43 zajednicke kocke kroz rjecnik
(`contracts/text/en.json`, oko 300 sifri): baze i posrednik (`qnc-content-store`, `qnc-db-broker`, `qnc-program-db`,
`qnc-virtual-shots`, `qnc-artifact-db`, `qnc-prerender-db`, zapisi medija i izvorni indeks), Select, uvoz, dovrsetak
zapisa, filmstrip i wave radnici, preview i monitor ("The player is getting ready.", "The picture is on the HDMI
output"), izlaz programa, prerender, izvori, slijed aplikacija i shell kocke. Rjecnik ne smije u granicu playera
(conformance "public player boundary": `qnc-player-input`, `qnc-player-client`, `qnc-decoder-catalog`,
`qnc-program-playlist`, `qnc-work-settings` i sve sto player povlaci), pa te kocke pisu engleski tekst izravno;
`qnc-player-client::PREPARING` je odgovor "player se priprema" koji preview prepoznaje. `qnc-dev-diagnostics` takoder
pise izravno engleski (rjecnik ovisi o njemu). Signal "radnik pauziran zbog playera" ostaje usporedba s tekstom sifre
(`filmstrip_paused_for_playback`, `wave_paused_for_playback`). Testovi koji su trazili hrvatski komad poruke traze
engleski. Nije izvedeno: Project, `qnc-project-close`, `qnc-project-session`, katalog tipki (nazivi akcija, F1).
Izvedeno 2026-10-14 (korisnik: "da, nastavi s Projectom"; Project otkljucan samo za zamjenu natpisa siframa): Project i
shell kroz rjecnik. `contracts/ui/project.layout.json`: 126 natpisa su sifre (naslovi, obrazac Advanced postavki, nazivi
izbora, tekstovi ploce, pitanja); vrijednosti postavki, kljucevi, redoslijedi i blokovi nisu dirani (rec709/rec2020
ostaju kako jesu); `qnc-project-blocks` razrjesava sifre pri ucitavanju. Poruke koda kroz `text`/`fill`:
`qnc-project-application`, `qnc-project-session` (stanja, "Računalo" je `source.computer`), `qnc-project-store` (greske
lokacija, brisanja, zastite i seeda), `qnc-project-close`, `apps/qnc-project`, te shell `qnc-desktop` i
`qnc-shell-footer` (Theme, Close project, nazivi tema). Conformance provjera vrsta lokacije Projecta trazi sifru
`source.computer` umjesto rijeci "Računalo". Testovi koji su trazili hrvatski tekst traze engleski. Project je ponovno
zamrznut. Nije izvedeno: katalog tipki (nazivi akcija, pomoc F1), imena sistemskih templatea u seedu (podaci).
Izmjena izgleda 2026-10-14 (korisnik: "sredio bi source timeline.. previse je upadljiva sa svojim horizontalnim
linijama; filmstrip ostaje kakav je, ali iznad i ispod filmstripa nemamo linije oko audio trakova; audio trakovi su
odmah uz filmstrip; boja audio trake moze biti kao u Final Cut Pro"): otkljucan samo `crates/qnc-timeline` (zajednicka
kocka, vrijedi za source timeline i Wrap redove). Audio trake i video traka (filmstrip) nemaju obrub, razmak izmedu
traka je 0, a 6 px nekadasnjih razmaka dobile su audio trake (15 -> 18 px), pa ukupna visina ostaje v4 (102 px, test).
Audio trake su zelene kao u Final Cut Pro (traka rgb 38,98,62, val rgb 132,214,158, sve trake iste). Slike filmstripa i
vanjski obrub timelinea nisu dirani. Usput: dva testa `qnc-editorial-application` trazila su hrvatski tekst (iz koraka
zajednickih kocki) i sada uzimaju tekst kroz sifru; svi testovi workspacea prolaze.
Dopuna 2026-10-14 (korisnik: "daj audiju boju kakva je na gornjem dijelu story timeline video", uz isjecak zaslona):
audio trake imaju boje video segmenta price (izmjereno sa zaslona): podloga tamnozelena traka segmenta (rgb 17,103,88),
val mint tijela segmenta (rgb 80,190,148). `qnc-timeline` `TimelineTheme.audio_wave` je zasebna boja vala audio traka,
jer `wave_a1`/`wave_a2` boje i segmente price (te ostaju kakve su bile na zaslonu).
Dopuna 2026-10-14 (korisnik: "iz source timeline izbaci tamno plave linije"): plave linije bile su obrub raspona klipa
(`shot_range`, rgb 96,165,250) oko svake trake; na source timelineu raspon je uvijek cijeli klip, pa obrub nista ne
govori. `qnc-timeline` `TimelineLayerFlags::source()` ga iskljucuje (testovi); IN/OUT, playhead i fokus ostaju.
Izmjena izgleda 2026-10-14 (korisnik: "izbor audio kanala prebaciti na vertikalan prikaz, jedan ispod drugoga, ali da ne
ulazi u podrucje drugog audio traka; ujedno bi prikazivao i zum audio trak odabranog kanala preko polja videa; to smo
dosad imali odvojeno"): `qnc-channel-picker` je stupac kucica 1..N (`column_rect`: od trake prema dolje za A1, prema
gore za A2, unutar prostora do druge audio trake; celija je visoka kao traka, niza kad kanali ne stanu; test).
`qnc-timeline` crta stupac nakon svih traka (klik na kanal je izbor, klik u stupac ne ide traci ispod) i dok je traka
uzeta preko video trake crta veliki val nacrta kanala; audio traka s izborom kanala crta val svog izabranog kanala
(nacrt dok je uzeta; peakovi timelinea su kanali klipa 1..4 redom). Veliki val koristi `audio_wave`. Strelice gore/dolje
nisu mijenjane (povecanje ima jednu razinu).
Ispravak 2026-10-14 (korisnik: "ne ovako, nego iznad A2, odnosno ispod A1; ne na crnoj pozadini nego na trenutnoj"):
stupac kanala je samo na video traci: ispod trake A1 prema dolje, iznad trake A2 prema gore (sama traka ostaje
vidljiva), bez vlastite podloge i okvira, izravno preko velikog vala (`column_rect` uzima prostor video trake i visinu
celije; prozirna podloga i okvir se ne crtaju; test).
Ispravak 2026-10-14 (korisnik: "tocno ispod A1, preko dijela gdje pise V, jer kad se audio izbor otvori video se ne
vidi jer ga prekriva zoom audio"): stupac kanala je u stupcu oznaka traka (sirina oznake), tocno ispod A1 odnosno iznad
A2, s podlogom oznaka (`label_background`, bez okvira), pa ne prekriva video traku ni veliki val
(`column_rect` dobiva i sirinu; test).
Dopuna 2026-10-14 (korisnik: "A1 i A2 trebaju biti iste svjetline kao tekst i okviri checkboxova, u obliku dugmeta";
"kada A1 ili A2 idu u zum oni se prosire, ne dodaje se novi zoom trak"): `qnc-timeline` crta oznake A1/A2 kao dugme
(okvir i tekst u boji teksta); zoom je sama traka prosirena preko video trake do druge audio trake (A1 prema dolje, A2
prema gore), u boji trake, s valom nacrta kanala dok je traka uzeta, inace izabranog kanala.
Dopuna 2026-10-14 (korisnik: "izborom nekog kanala ne zatvara se expandirani audio... tek Enter sprema i zatvara, Esc
ponistava i zatvara"; "sada su kanali postavljeni vertikalno, tako trebaju i strelice"; "izabrano dugme A1 ili A2 treba
dobiti crveni rub"): dok je traka uzeta (Ctrl+1/Ctrl+2 ili klik na A1/A2), visina joj je prosirena preko video trake i
stoji otvorena; klik na kanal u stupcu i strelice gore/dolje mijenjaju samo nacrt kanala (`qnc-lane-choice` `set_draft`,
`qnc-program-segments` `choose_channel`), Enter sprema i zatvara, Esc odustaje i zatvara. Lijevo/desno vise ne biraju
kanal, a gore/dolje vise ne otvaraju i ne zatvaraju prosirenje (`zoom`/`lane_zoom` uklonjeni; `qnc-panel-focus`).
Dugme uzete trake ima crveni rub (`qnc-timeline` `TimelineTheme.lane_taken`). Poruka uzete trake kroz rjecnik
(`story.msg.lane_taken`). Testovi `qnc-lane-choice`, `qnc-program-segments`, `qnc-panel-focus`, `qnc-timeline`, conformance.
Promjena pravila 2026-10-14 (korisnik: "treba prebaciti izradu wave iz originala, glavnina snimatelja u kamerama ne
snima proxy"; nalaz: svih 98 waveova aktivnog projekta bilo je iz proxyja s 2 kanala, pa kanali 3 i 4 koje A1/A2 smiju
izabrati nisu imali wave iako se cuju iz originala): zamjenjuje pravilo iz §8 da se uz projektni `audio.channels = 2`
wave gradi iz proxyja. Wave se uvijek gradi iz spremljenog originala, svi stvarni kanali redom u trake A1-A4.
`qnc-wave` (uklonjen izbor proxyja `plan_from_snapshot_with_project_audio_channels`; `WAVE_RENDER_VERSION` 2, pa se
waveovi verzije 1 ponovno grade), `qnc-wave-worker`, `qnc-timeline-artifacts`, `qnc-content-artifacts` (uklonjeno polje
`project_audio_channels`). Wave sada cita original s kartice, pa je prvi prolaz sporiji nego iz proxyja. Testovi,
conformance.
Dopuna 2026-10-14 (korisnik: "daj ono sto ce najbrze omoguciti pocetak montaze"): wave originala cita se u jednom
prolazu. Original kamere drzi svaki kanal u zasebnom streamu, a radnik je za svaki stream pokretao dekoder koji cita
cijeli MXF s kartice (4 citanja po klipu). `qnc-ffmpeg-decode` `audio_streams_with_cancel` (jedan ffmpeg, streamovi
spojeni `amerge` redom u jedan isprepleteni f32 signal, video se samo demuxa), `qnc-decoder-catalog`
`LocalFilmstripExtractor::audio_streams_with_cancel` (izabrani dekoder kataloga), `qnc-wave-worker` (`decode_wave`:
kanali svakog streama idu njegovom sakupljacu; prije citanja ista provjera kataloga za zapisani format svakog streama i
serijski broj kartice). Izmjereno: klip od 9 s s 4 kanala 2,2 s s kartice. Katalog dekodera nije prebacen na
ugradeni lib (`selected` ostaje `qnc.ffmpeg`), jer bi to promijenilo i player.
Dopuna 2026-10-14 (korisnik: "preview play audio treba biti moguc prebacivanjem na neki kanal bez da je spremljen
enterom; dok je neki kanal oznacen on se pleja, Enter ga samo trajno zapisuje u bazu"): dok je traka uzeta, preview
odmah pusta oznaceni kanal (strelice gore/dolje ili klik u stupcu); Enter ga zapisuje, Esc vraca spremljeni kanal.
`qnc-lane-choice` `audible`, `qnc-program-segments` `audible_channels` (zapis u pricu i dalje uzima samo spremljeni
kanal `heard_channels`), `qnc-panel-focus`, `qnc-editorial-application`. `qnc-source-preview` `hear_channels`: kanal
promijenjen tijekom playa nastavlja svirati cim je klip ponovno pripremljen (kratki prekid pripreme ostaje).
Dopuna 2026-10-15 (korisnik: "to je sporo, 30 minuta materijala za 10 minuta, to za news TV"): mjereno, ffmpeg za zvuk
originala cita cijelu datoteku brzinom kartice (USB 2.0 citac, ~28 MB/s), a zvuk je oko 4 % datoteke. Nova javna kocka
`crates/qnc-mxf-sound` (+ `contracts/modules/mxf-sound.module.json`): zvucni elementi frame-wrapped MXF-a bez slike i
podatkovnih elemenata, jedno citanje od 40 KB po okviru, ostalo se preskace; samo citanje, nista ne dekodira. Na kartici
bajt za bajt isto kao ffmpeg za sva 4 kanala (test koji se pokrece rucno, `QNC_MXF_SAMPLE`, `QNC_FFMPEG`). Izbor po
zapisanom formatu prije citanja radi `qnc-wave-worker` (MXF, svaki trazeni stream `pcm_s24le` mono; k-ti audio stream je
zvucni element k), sve ostalo ide kroz katalog dekodera u jednom prolazu; katalog i player je ne koriste (granica
playera). Izmjereno (Long GOP original s kartice): 1489 (oko 130 s materijala) 16,6 s, prije 36 s; 60-62 MB/s datoteke.
Granica je vrijeme jednog citanja na kartici (1,5-2,5 ms po citanju).
Dopuna 2026-10-15 (korisnik: "generiranje wave je sporo"): wave ima prednost pred filmstripom (oba citaju istu karticu,
a zvuk montazi treba prvi): `qnc-timeline-artifacts` drzi filmstrip dok wave ima posla. Klip koji preview ima otvoren
(`qnc-playback-activity` `PLAYBACK_CLIP` u bazi projekta) dobiva wave prvi: `qnc-wave-worker` `prefer`,
`qnc-content-artifacts` `prefer`, `tools/qnc-ingest-worker` ga cita dvaput u sekundi. Pravilo da wave i filmstrip
stanu dok player radi ostaje. Granica brzine je citac kartice (USB 2.0, ~28 MB/s; kartica V30 UHS-I cita i do ~90 MB/s).
Dopuna 2026-10-15 (korisnik: "da" na wave cim je zapis klipa gotov): radnik je radio redom filmstrip svih klipova ->
dovrsetak zapisa svih klipova (dok artefakti rade, dovrsavao je samo klip koji preview trazi) -> waveovi u drugom
prolazu. Sada `tools/qnc-ingest-worker`: dovrsetak zapisa ne ceka artefakte; dok traje, prolaz artefakata svake 3 s
ponovno cita bazu i wave klipa krene cim je njegov zapis konacan (`qnc-wave-worker` u red uzima samo klipove s
konacnim zapisom), otvoreni klip prvi; filmstrip ceka dok traje dovrsetak zapisa i dok waveovi imaju posla
(`qnc-timeline-artifacts` / `qnc-content-artifacts` `hold_filmstrip`). Waveovi i filmstrip i dalje stanu dok player radi.
Dopuna 2026-10-15 (korisnik: "treba moci iskljuciti bilo koji kanal, da se ne cuje ni jedan na A1 i na A2; dugme A1 ili
A2 tada crveno"; "mozemo to napraviti kao nivo 0... dobar put da mozemo dodati i podesavanje nivoa"; boja uzete trake
zuta): A1 i A2 imaju kanal i razinu. U stupcu kanala ispod zadnjeg kanala je "0" (`qnc-channel-picker`: mjesto `count`,
`ChannelChoice::silent`): traka zadrzava kanal, a cuje se razina 0; izbor kanala vraca razinu. Strelice gore/dolje,
klik i Enter/Esc rade i za "0" (`qnc-lane-choice`: `silent`, `audible` daje kanale i razine, `choice` daje `count` za
razinu 0; `heard` za zapis u pricu daje i dalje samo kanal). `qnc-timeline`: dugme trake na razini 0 je crveno
(`TimelineTheme.lane_silent`), uzeta traka ima zuti rub (`lane_taken`), val trake na razini 0 je ravan. Razina ide do
playera kao postotak (100 kako je snimljeno, 0 tisina, najvise 400): `qnc-player-input` `lead_audio_levels`,
`with_lead_levels`, `output_audio_levels`; `qnc-broadcast-engine` `InputPlan.audio_levels`, `DeviceSink` mnozi izlaze
nakon preslikavanja kanala (`apply_levels`, test); `qnc-source-preview` `hear_channels` prima kanale i razine.
Vrijedi za source preview. Nije izvedeno: razina u prici (segment ili pokrivalica s A1/A2 na razini 0 u bazi, Wrap,
program, prerender) i podesavanje razine osim 0 i 100.
Izmjena 2026-10-15 (korisnik: "uvodenje novog checkboxa mi se ne svida, ostali checkboxovi postaju mali; radije
strelice lijevo desno: lijevo iskljucivanje, desno ukljucivanje, a sa Ctrl strelice podesavanje nivoa tona"): kucica
"0" iz `qnc-channel-picker` je uklonjena (stupac kanala kao prije). Dok je A1 ili A2 uzeta: gore/dolje kanal, lijevo
iskljucuje traku (razina 0, kanal ostaje), desno ukljucuje, Ctrl+lijevo/desno razina za 1 dB (od -40 do +12 dB);
sve se odmah cuje, Enter sprema, Esc vraca. Nove akcije kataloga `lane_level_down` (Ctrl+lijevo) i `lane_level_up`
(Ctrl+desno) u svim presetima (scope `storyboard`, gdje je i `select_audio_a1/a2`) i u grupi pomoci "Audio A1 / A2".
`qnc-lane-choice` (`Lane`: kanal, `off`, `level_db`; `set_off`, `nudge_level`, `lane`, `audible` daje pojacanje),
`qnc-program-view` (`lane_off`, `lane_level_db`), `qnc-program-segments` (`lane_off`, `lane_level`, poruka s kanalom i
razinom), `qnc-panel-focus`, `qnc-channel-picker` (`ChannelChoice.off`), `qnc-segment-panel`, `qnc-timeline` (iskljucena
traka je crveno dugme, uzeta zuti rub). Razina i dalje ide playeru kao postotak po izlazu (vidi prethodni zapis).
Vrijedi za source preview; razina u prici nije izvedena.
Izmjena izgleda 2026-10-15 (korisnik: "timeline treba biti kocka koja ima svoje kocke, tako da timeline uvijek i svugdje
ima iste mogucnosti; vise mi se svida dizajn timeline iz segmenata"; "video trak ima srednji svjetlo zeleni dio i gore
tamniji dio; takav tamniji dio iste sirine treba dodati i ispod; A1 se seli u gornji tamniji dio, a A2 u donji; ukupna
visina se prilagodava trenutnoj visini"): `qnc-timeline` ima sloj `audio_bands` (`TimelineLayerFlags::with_audio_bands`):
video traka zauzima visinu A1 + video + A2 (`TimelineMetrics::banded_video_height`, ukupna visina ista, test), gore i
dolje je tamni pojas visine `VIDEO_BAND` (17 px, kao dosadasnji gornji), A1 (dugme i val) je u gornjem, A2 u donjem
(`paint_audio_band`; dugme `paint_lane_button` dijele trake i pojasevi), svijetli dio i njegov pogodak klikom su izmedju
pojaseva (`video_middle_insets`), pokrivalice ostaju u svijetlom dijelu. Klik na A1/A2 uzima traku kao i prije; izbornik
kanala i prosirenje vala koriste pojaseve kao trake. Wrap redovi panela Segmenti (`qnc-segment-panel` `layers`) ga
koriste. Source timeline jos nije prebacen (pojasevi bi pokrili dio filmstripa; ceka odluku).
Popravak izolacije 2026-10-15: aplikacija pokrenuta iz izolirane kopije (`QNC_ROOT`) otvorila je stvarnu bazu projekata,
jer `qnc-desktop::resolve_qnc_root` nije citao `QNC_ROOT`, a kopiji je nedostajao `contracts/ui/shell.layout.json`, pa
je trazenje palo na radni direktorij (`C:\Users\miron\Projects\QNC`). Posljedica: u stvarnoj `data/qnc-projects.db`
sistemski templatei (4 izvora, 4 projekta) ponovno su upisani iz seeda s vremenom 20:09:46; projekt "novi test11"
(`project.db`, `-wal`) nije mijenjan. `resolve_qnc_root` sada prvo uzima `QNC_ROOT`, a korijen koji nije QNC korijen je
greska (nema pada na radni direktorij); kopija je dobila `shell.layout.json`; aplikacija se pokrece s radnim
direktorijem u kopiji. Samostalne aplikacije (`apps/qnc-project`, `qnc-story`, MA, `qnc-ingest`) imaju svoje
trazenje korijena bez te provjere.
Dopuna 2026-10-15 (korisnik: "svedi wrap timeline na dimenzije source timeline, neka visina tonova i videa bude ista
kao source timeline"): pojasevi A1 i A2 visoki su kao audio traka source timelinea (`TimelineMetrics.audio_height`,
18 px), a svijetli dio izmedju njih kao njegova video traka (`video_height`, 64 px); obje strane koriste
`TimelineMetrics::default`. `qnc-timeline` (`video_middle_insets`, pojasevi u `show`, test). Snimljeno u izoliranoj kopiji.
Dopuna 2026-10-15 (korisnik: "pokrivalice farbaju cijeli dio video traka"; "playhead ide kroz cijelu visinu wrap-segmenta,
ali u prikazu slota boja se samo video i A2; A1 nije dio slota, on je baza slota"): u Wrap redovima (sloj `audio_bands`)
pokrivalica boji cijeli svijetli dio (prije gornjih ~40 %), slot (crtanje i klik) pocinje ispod pojasa A1 i boji video i
A2, a playhead se crta nakon pojaseva preko cijele visine reda. Source timeline nije mijenjan. `qnc-timeline`
(`cover_rows`, `slot_top`), testovi.
Dopuna 2026-10-15 (korisnik: "zasto audio A1 u segmentu gubi boju pozadine, njegova boja treba biti kao i A2"): pojasevi
A1 i A2 Wrap reda imaju istu pozadinu audio trake (`audio_primary_background`), nacrtanu prije slota i pokrivalica, pa
slot i dalje boji samo video i A2. `qnc-timeline` (`paint_video_row`).
Dopuna 2026-10-15 (korisnik: "nije potrebno prikazivati M markere jer smetaju prikazu A1; marker linija ne treba se crtati
preko A1 audio jer markeri se odnose na pokrivalice"): u Wrap redovima (sloj `audio_bands`) linija markera ide samo preko
videa i A2, bez oznake "M"; pregledni red programa (bez pojaseva) ostaje kakav je. `qnc-timeline` (`paint_markers`).
Dopuna 2026-10-15 (korisnik: "ovolika visina video traka u Wrap-segmentu nema smisla ako nema filmstrip... trebamo ili
filmstrip ili suziti visinu, ili imati obje opcije"; "moze ostati vracanje playheada na pocetak, a uvodimo Ctrl+ za
prebacivanje modova filmstrip/uska video traka"; "A1 i A2 trake trebaju biti iste boje"): Wrap redovi pokazuju filmstrip
onoga sto se tamo vidi (Ton segmenti i pokrivalice preko njih), a Ctrl+klik na V prebacuje na usku video traku (visine
audio pojasa, bez slika) i natrag; obican klik na V ostaje Home. Nova javna kocka `crates/qnc-program-filmstrip` (+ ugovor
`contracts/modules/program-filmstrip.module.json`): spremljeni filmstripovi klipova procitani u pozadini kroz javni citac
artefakata, slozeni na os programa po izvornom IN/OUT (kao `qnc-program-waveform`), isjecak za red; ne dekodira, ne pise.
`qnc-timeline-assets` `read_filmstrip_background`; `qnc-timeline` (`TimelinePicture`, `pictures`, sloj `compact_video`,
namjera `ToggleVideoMode`; preko slika segment ima samo okvir, a pokrivalica je traka od 13 px na vrhu slike; slot boji
samo video dio i uokviruje A2, pa A1 i A2 ostaju iste boje); `qnc-program-view` (`video_compact`, `pictures`,
`picture_placements`); `qnc-program-commands` (`ToggleWrapVideo`, akcija kataloga `toggle_wrap_video` bez tipke);
`qnc-program-segments`; `qnc-segment-panel`; `qnc-ingest-application` (samo krak namjere, C10 isti). Izbor moda vrijedi dok
aplikacija radi. Testovi, conformance.
Popravak 2026-10-15 (korisnik: "ne gradi uopce filmstrip"): filmstripovi su bili spremljeni, ali je svaka slicica crtana
u svom omjeru u sredini svog dijela reda (`paint_rgba_image` cuva omjer), pa se na sirokom segmentu vidjela jedna mala
slicica s prazninom oko nje. `qnc-timeline` (`paint_pictures`): slicica se ponavlja u svom omjeru preko svog dijela reda,
odrezana na njegovim rubovima (kao NLE); odabrani slot preko slicica je samo okvir. Snimljeno u izoliranoj kopiji.
Popravak 2026-10-15 (korisnik: "kad se prikazuje filmstrip u segmentu, u program timelineu ne vidi se A2 audio"): pregledni
red programa na dnu panela Segmenti imao je stalnih 128 px za traku i red, manje od reda s pojasevima, pa je pojas A2
ispadao. `qnc-segment-panel`: visina tog dijela je traka plus stvarna visina reda (filmstrip ili uska traka). Snimljeno u
izoliranoj kopiji.
Dopuna 2026-10-15 (korisnik: "u pokrivalice na segmentu ne ubacivati vise filmstrip slicica nego sto ih moze stati";
"suzeni video trak napravio bi duplo siri"): segment ili pokrivalica crta samo onoliko slicica koliko ih stane u njihovom
omjeru, svaka popunjava svoju celiju (slika njegova klipa na sredini celije; susjedni komadi istog filmstripa su jedan
segment ili pokrivalica). `qnc-ui-kit` `paint_rgba_image_filled` (ista slika, odrezana da popuni pravokutnik),
`qnc-timeline` (`picture_runs`, `TimelinePicture.strip`), `qnc-segment-panel`. Uska video traka je dvije visine audio
pojasa (36 px).
Promjena 2026-10-15 (korisnik: "cim neki klip dobije wave, na drugoj jezgri raditi filmstrip"; "nema smisla da trazi koji
ima wave a koji nema, napravi import control kocku koja ce kad obradi wave nekog klipa dati komandu da drugi worker
pokrene filmstrip za taj klip"; izmjereno: citanje zvuka MXF-a s kartice uzima oko 7 % jedne jezgre): nova javna kocka
`crates/qnc-import-control` (+ ugovor `contracts/modules/import-control.module.json`; samo odlucuje, nista ne cita ni
pise): kad je wave klipa gotov (napravljen, vec spremljen ili nemoguc), daje naredbu filmstrip radniku za taj klip; dok
waveovi ili dovrsetak zapisa imaju posla, filmstrip sam ne trazi klipove; kad oboje zavrsi, filmstrip uzima ostatak
(klip bez wavea). `qnc-wave-worker` (`take_finished`), `qnc-filmstrip-worker` (`request`, `set_on_orders`),
`qnc-timeline-artifacts` (naredbe kocke izmedju dva radnika; filmstrip vise ne ceka sve waveove). Zamjenjuje redoslijed
"filmstrip tek nakon svih waveova". Proba na izoliranoj kopiji projekta (98 klipova, kartica samo citana, waveovi i
filmstripovi obrisani u kopiji): filmstrip prati waveove (npr. 20 s: 10 waveova i 9 filmstripova), svi waveovi i
filmstripovi gotovi za 266 s; prije su waveovi trajali oko 150 s, pa filmstrip jos oko 60 s.
Dopuna 2026-10-16 (korisnik: "ok, napravi jedan prolaz"): MXF original s H.264 slikom cita se s kartice jednom za wave i
filmstrip. `qnc-mxf-sound` (`read_sound_and_pictures`, `h264_gop_start`): u istom prolazu po zvucnim elementima uzima i
kljucnu sliku na ili iza svake od 13 pozicija filmstripa (pocetak slike je vec u citanju koje je naslo njezin kljuc, pa
provjera ne kosta citanje; cijela se cita samo predana slika); `qnc-wave-worker` ih predaje uz gotov wave; `qnc-import-control`
ih nosi u naredbi filmstripu; `qnc-filmstrip-worker` ih zapise u privremenu mapu sustava i dekodira jednim ffmpeg pozivom
s po jednim ulazom za svaku sliku (`qnc-decoder-catalog`/`qnc-ffmpeg-decode` `decode_pictures_with_cancel`; u jednom toku
dekoder je vracao 3 od 13 jer kljucne slike kamere nisu IDR), bez ponovnog citanja medija; ako to ne uspije, filmstrip cita
medij kao prije (razlog u logu). Promjena pravila iz §8 za ovaj slucaj (odluka korisnika): takav filmstrip je iz originala,
ne iz proxyja (`qnc-filmstrip` `plan_from_original`). Ostali formati rade kao prije. Proba na izoliranoj kopiji projekta
(98 klipova, 30 min materijala, kartica samo citana): svih 98 filmstripova iz jednog prolaza, nijedno dodatno citanje
kartice, dekodiranje medijan 0,8 s po klipu, sve gotovo za oko 250 s (prije 266-276 s); waveovi sami traju gotovo toliko
jer je granica citac kartice (USB 2.0), a dugi klipovi (2-4 min) i dalje drze wave 20-30 s.
Dopuna 2026-10-16 (korisnik: "zasto XDCAM i MP4 ne mogu raditi na isti nacin"; "idemo ovo ispeglati"): i XDCAM (MPEG-2
4:2:2 u MXF-u) ide jednim prolazom. `qnc-mxf-sound` (`mpeg2_gop_start`: kljucna slika pocinje oznakom sekvence ili GOP-a;
`key_picture_test` po spremljenom kodeku), `qnc-wave-worker` (`picture_codec`: h264 ili mpeg2video), `qnc-decoder-catalog`
(MPEG-2 slike kroz ffmpeg demuxer `mpegvideo`). Primjer `qnc-mxf-sound/examples/time_read.rs` uzima i 13 slika i pise ih
samo u zadanu mapu. Provjereno na korisnikovom XDCAM exportu `Mironik 2002.MXF` (samo citano): 13 od 13 kljucnih slika
(GOP 12) u istom prolazu sa zvukom, dekodirane jednim ffmpeg pozivom za 0,25 s, slicica ispravna.
Dopuna 2026-10-16 (isto: "idemo ovo ispeglati"): MP4 i QuickTime original s jednom AAC zvucnom trakom ide jednim prolazom
kroz indeks datoteke. Nova javna kocka `crates/qnc-mp4-sound` (+ ugovor `contracts/modules/mp4-sound.module.json`; samo
citanje): cita indeks (`moov`: trake, mjesta uzoraka, kljucni uzorci, AAC konfiguracija, H.264 parametri), pa s kartice
samo uzorke zvuka (uzastopni uzorci jednim citanjem) i kljucnu sliku na ili iza svake od 13 pozicija filmstripa
(`adts_frame`, `avc_annexb`). `qnc-ffmpeg-decode`/`qnc-decoder-catalog` `audio_bytes_with_cancel` (AAC iz procitanih
okvira kroz izabrani dekoder), `qnc-wave-worker` (`mp4_sound`, `mp4_sound_samples`; slike H.264 idu filmstripu kao kod
MXF-a). Ostali MP4 (vise zvucnih traka, PCM zvuk, HEVC slika) rade kao prije. Provjereno na MP4 datotekama s kartice (samo
citano, test koji se pokrece rucno `QNC_MP4_SAMPLE`): zvuk kroz indeks jednak je uzorak po uzorak zvuku koji daje ffmpeg
iz cijele datoteke, 13 od 13 kljucnih slika; 336 MB datoteka 1,2 s kroz indeks prema 4,8 s cijela (moguce djelomicno iz
memorije sustava), prototip mimo memorije sustava 1,13 s za 3:43 (369 MB). Nije provjereno kroz cijeli radni tijek: projekt
s MP4 originalima nije bio dostupan.
Popravak 2026-10-16 (korisnik: "kada se pleya klip u source timeline u Sync/B-roll, playhead ne putuje s playem, pa ako
strelicama produljim ili skratim selektirani dio, ne prikazuju se slicice u monitoru"): (1) za vrijeme Synca source
timeline uz playhead pokazuje IN i OUT pokrivalice: OUT je kraj nacrta i prati play i strelice (`qnc-sync-cover`
`source_view`); (2) strelica na zaustavljenom Syncu pomakne kraj i player prozora Synca pokaze zadnju sliku pokrivalice na
novom kraju (`stopped_window_frame`, `qnc-program-segments` `sync_window_frame`, `qnc-panel-focus`). Kraj pomaknut ispred
pocetka prozora (nakon nastavka Spaceom) nema sliku u tom prozoru. Testovi.
Izmjena izgleda 2026-10-16 (korisnik: "na segment panelu gdje su dugmad sakrij centralnu grupu dugmadi i umjesto nje
postavi pokazivac jacine tona; dugmad trenutno samo sakrij, nemoj brisati"; izbor: stvarni izlaz playera, dvije vodoravne
trake A1/A2): u traci panela Segmenti srednja grupa (prethodni/sljedeci segment, slot, marker, pocetak programa) je
skrivena (`qnc-segment-panel` `bar.rs` `SHOW_NAVIGATION = false`, kod ostaje), na njezinom mjestu je pokazivac razine.
Razina se mjeri tamo gdje uzorci idu na zvucni uredaj, ne u redu (`qnc-audio-output`: vrh svakog izlaznog kanala u
povratnom pozivu uredaja, bez alokacije, `take_peaks`), engine ga daje (`take_output_peaks`), player ga uz svako stanje
salje kao postojeci dogadaj ugovora `AudioLevelChanged` (`output_1` = A1, `output_2` = A2, stotinke dBFS;
`tools/qnc-player-runner`), klijent ga cita (`qnc-player-client` `output_level_dbfs`), preview ga nosi
(`qnc-source-preview` `PreviewView.output_levels`). Nova pasivna kocka `crates/qnc-level-meter` (+ ugovor): trake
-48..0 dBFS, zelena, zuta od -12, crvena od -3, pad 24 dB/s, vrh se drzi 1,2 s. Story ploca predaje razine jednim
argumentom postojeceg poziva. Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-16 (korisnik: "u audio traku oznake (tanke linije) da korisnik vidi je li mu ton u granicama"; "pojacanje
ili smanjivanje nivoa tona u source timeline treba djelovati na prikaz wavea"): svaka audio traka (source timeline, pojasevi
Wrap reda, uvecani val) ima tanke linije iznad i ispod sredine na -18 dBFS (referenca EBU R 68, prigusena), -9 dBFS
(najvisa dopustena razina, zuta) i -1 dBFS (granica, crvena) na istoj linearnoj ljestvici kao val (`qnc-wave-view`
`LEVEL_MARKS_DBFS`, `paint_level_marks`; boje u `TimelineTheme.level_marks`). Razina A1/A2 iz source timelinea
(Ctrl+lijevo/desno, iskljucena traka) mnozi prikazani val (`qnc-channel-picker` `ChannelChoice.level_db`, `gain`;
`qnc-wave-view` `paint_wave_peaks_gain`; `qnc-timeline` `lane_gain`); glasnije od pune skale udara u rub trake. Valovi
price u Wrap redovima ostaju kako su snimljeni (razina segmenta jos se ne sprema u pricu). Testovi, conformance.
Popravak 2026-10-16 (korisnik: "dodao sam jos jedan USB ali ga ne prepoznaje iako ga sistem vidi"; "ne dodaji ni oduzimaj,
to treba biti automatsko"): izvori Ingesta bili su samo diskovi rucno upisani u konfiguraciju racunala
(`data/ingest-transport.json`). Sada je svaki disk koji sustav vidi izvor bez rucnog upisa. Nova javna kocka
`crates/qnc-source-volumes` (+ ugovor; samo citanje: priklјuceni diskovi sa serijskim brojem volumena; Windows, ostali
sustavi zasad prazan popis). `qnc-source-bindings::load` (`with_attached`): priklјuceni disk koji nije upisan postaje izvor
`qnc://local/source/volume-<serijski>` s probeom lokalnih izvora; upisani disk pod drugim slovom dobiva trenutno slovo;
nista se ne brise i datoteka se ne pise. Vrijedi za sve koji citaju izvore (Ingest, preview/player, radnici). Ingest popis
izvora cita ponovno kad se otvori popis diskova (`qnc-ingest-application` `refresh_sources`; C10 isti), pa disk
prikljucen dok aplikacija radi se pojavi bez ponovnog pokretanja. Rucni upis E: u kopiju je vracen.
Popravak 2026-10-16 (korisnik: "to je krivo... G nije prikljucen"): popis izvora u Ingestu nudi samo diskove koji su sada
prikljuceni; upisani lokalni disk sa serijskim brojem koji nije prikljucen se ne prikazuje (iz konfiguracije se ne brise i
vraca se cim se prikljuci). Bez popisa diskova (sustav bez tog adaptera) nude se svi izvori. `qnc-ingest-select`
`SelectionConfig::browser`. Veze izvora za player i radnike nisu mijenjane.
Dopuna 2026-10-16 (korisnik prikljucio XDCAM disk, Sony PMW-300: MPEG-2 HD 35 Mbit/s 1920x1080 50i, 4 kanala LPCM 16-bit,
`XDROOT/MEDIAPRO.XML`; djelomicno kopiran, za 1910 samo XML): proba na izoliranoj kopiji (disk samo citan) - disk je
prepoznat automatski (`volume-7e80f1fb`), Select je primio 7 klipova s tocnim trajanjem iz XML-a (1910 bez MXF-a
preskocen), radnik je napravio konacne zapise, valove, filmstripove i postere. Jedan prolaz sada prima i 16-bitni zvuk
(`qnc-mxf-sound` `s16le_to_f32`; `qnc-wave-worker` `mxf_sound_elements` daje i velicinu uzorka, svi trazeni streamovi
iste velicine): valovi jednaki onima iz ffmpega (7 od 7), filmstripovi iz istog prolaza (7 od 7, dekodiranje ~0,3 s po
klipu), slicica ispravna.
Popravak 2026-10-16 (korisnik: XDCAM klip ne svira, `pixel conversion: Missing("color.primaries")`; "je li to nagadanje
ili cinjenica"; "sto je kamera zapisala"; "da"): kamera (Sony PMW-300, MPEG HD 35 1080i50 4:2:0) u zaglavlju video toka
(MPEG-2 sequence display extension) izricito pise primarne boje, prijenos i matricu = 1 (BT.709), u svih 7 klipova diska;
MXF omot to ne ponavlja, pa sazetak streama koji ffprobe daje bez dekodiranja kaze `unknown` za primarne boje i matricu,
a to je zapisano u bazu. Izvedeno: isti jedini probe (`qnc-media-probe`) cita i prve pakete datoteke (`-show_frames
-read_intervals %+#32`, samo polja boje slike; nije prolaz kroz slike), a `qnc-ffprobe-metadata` boju koju sazetak ne zna
uzima iz prve dekodirane slike istog streama koja je zna (lokator `/frames/<i>/...`); ako je ne zna ni slika, ostaje
nenavedena i player daje jasnu gresku kao prije. Nema pravila "HD znaci BT.709". Test argumenata probea vise ne zabranjuje
`-show_frames`/`-read_intervals`, nego trazi ograniceni broj paketa (`FIRST_PACKETS`); `-count_frames` i `-show_packets`
ostaju zabranjeni. Izmjereno: poziv 0,12 s. Klipovi koji vec imaju konacni zapis zadrzavaju ga (jedan probe po mediju);
novi zapis nastaje u novom projektu. Testovi `qnc-ffprobe-metadata` (17), `qnc-media-probe` (8), conformance.
Popravak 2026-10-16 (korisnik: "kad u source timelineu vise puta ukljucim/iskljucim audio, IN i OUT se postave na
pocetak klipa"): promjena kanala ili razine A1/A2 ponovno priprema isti klip u playeru; dok player ne potvrdi poziciju,
prikaz timelinea je prazan (trajanje 1, bez playheada), a prijenos IN/OUT-a na novi prikaz (`preserving_source_marks_from`)
stisnuo ih je na tu duljinu, dakle na pocetak. `qnc-timeline`: dok novi prikaz nema potvrdjenu poziciju, IN/OUT ostaju
kakvi jesu (test). `qnc-source-preview` `hear_channels`: klip se ponovno otvara na trazenoj ili potvrdjenoj slici, a dok
ni jedne nema, na slici na kojoj je zadnji put otvaran (`opening_at`), ne na pocetku. Testovi, conformance.
Otkljucano i izvedeno 2026-10-16 (korisnik: "ne snima pojacani ton... dodati polje koje bi pamtilo pojacanje, kako bi se i
u konacnom exportu zadrzali parametri tona"; "odkljucaj, ali pazi da sve rjesavas preko kocki"): razina A1/A2 vise nije samo
za slusalice. `qnc-program-db`: segment ima `a1_level_db` (dB, 0 kako je snimljeno, -40..+12) i `a1_off`, pokrivalica
`a2_level_db` i `a2_off`; postojece price dobivaju stupce s 0/ukljuceno pri otvaranju (bez brisanja); `CreateSegment` i
`CreateCover` ih primaju, nove operacije `SetSegmentSound` / `SetCoverSound` (kanal, razina, off) mijenjaju ih kasnije
(jedan Undo korak); podjela pokrivalice markerom ih prenosi. Talking Head / Voice over, Cover slot / Overwrite i Sync
upisuju zvuk spremljen na source timelineu (Enter; `qnc-lane-choice` `kept`). `qnc-program-playlist`: zvucna ruta nosi
`level_db`, iskljucena traka nema rutu (tisina). `qnc-program-input` (i privremena pokrivalica Synca) ih prenosi.
`qnc-broadcast-engine`: traka programa mnozi uzorke razinom; gotov dio prerendera svira na 0 dB jer je razina vec u njemu.
`qnc-render-unit`: `AudioPart.level_db` ulazi u opis i otisak samo kad nije 0 (dijelovi kako je snimljeno zadrzavaju
otisak), `qnc-program-render` je primjenjuje pri renderu (pa i u exportu iz prerendera). `qnc-program-waveform`: val Wrap
reda crta se s razinom (iskljucen ravno). Nova javna kocka `crates/qnc-part-sound` (+ ugovor): zvuk spremljenog segmenta
(A1) ili pokrivalice (A2) uzet na Wrap retku, nad `qnc-lane-choice` (`keep`); `qnc-program-segments` (`part_sound.rs`)
nade segment ili pokrivalicu pod Wrap playheadom (inace izabranu), broj kanala iz procitanog vala klipa
(`qnc-program-waveform` `channels`), Enter pise operaciju. `qnc-panel-focus`: na panelu Segmenti Ctrl+1/Ctrl+2 uzimaju
zvuk segmenta/pokrivalice, gore/dolje kanal, lijevo/desno off/on, Ctrl+lijevo/desno razina, Enter sprema, Esc odustaje;
`qnc-segment-panel` na tom Wrap retku pokazuje stupac kanala. Nacrt na Wrap retku se ne cuje prije Entera (program se
ponovno otvara nakon upisa). Ugovor `story.database.json` (`lane_sound`). Testovi (`qnc-program-db`, `qnc-render-unit`,
`qnc-program-waveform`, `qnc-part-sound`), conformance. Uzivo nije provjereno. Usput: `cargo clean --profile dev`
(49,3 GiB; disk je imao 223 MB slobodno), uz dozvolu korisnika.
Dopuna 2026-10-16 (korisnik: "neka se nacrt na Wrap retku cuje odmah prije Entera"): `qnc-program-input` `SoundDraft`
(zvuk jednog spremljenog segmenta ili pokrivalice kako je nacrtan) i `draft_loader` (prica iz baze s tim nacrtom; nista se
ne pise; dio prerendera koji nacrt mijenja ima drugi otisak pa svira iz izvora). `qnc-source-preview`
`hear_program_draft`: dok svira program (Wrap), program se ponovno otvara s nacrtom na slici koju pokazuje i nastavlja
svirati ako je svirao; `None` vraca spremljeni zvuk (Esc), `forget_program_draft` nakon Entera (upis ponovno otvara
program iz baze). `qnc-panel-focus` slaze nacrt iz `qnc-program-segments` (`part_sound_draft_now`) i predaje ga
previewu pri svakoj promjeni kanala, razine i ukljucenosti. Svaka promjena ponovno slaze program (oko 0,5 s), kao
promjena kanala na source timelineu. Test `a_sound_draft_changes_only_its_own_segment_or_cover`, conformance.
Otkljucano i izvedeno 2026-10-16 (korisnik: "prekida dok mijenjam razinu"; "da, otkljucaj i napravi"): log je pokazao da
svaki korak razine (Ctrl+lijevo/desno, ukljuci/iskljuci) zatvara i ponovno priprema klip (`player-open`), a engine je
razinu primjenjivao kad zvuk ulazi u red (do 4 s unaprijed). Sada razinu primjenjuje uredaj: `qnc-audio-output` drzi
pojacanje po izlaznom kanalu (`set_gains`) i mnozi uzorke u povratnom pozivu uredaja (gdje se mjeri i pokazivac razine),
pa se promjena cuje u sljedecem pozivu. `qnc-broadcast-engine`: `DeviceSink::with_levels` postavlja pojacanja uredaja
(vise ne mnozi red), `Runtime::set_output_levels`. `qnc-player-contract`: naredba `SetOutputLevels { levels_percent }`
(1-8 kanala, 0..400 %), `tools/qnc-player-runner` je izvrsava, `qnc-player-client` `Action::SetOutputLevels([u16; 2])`
(bez cekanja pozicije). `qnc-source-preview`: na source timelineu promjena samo razine ili ukljucenosti salje tu
naredbu (promjena kanala i dalje ponovno priprema klip); na Wrap retku nacrt razine prema spremljenoj (`stored`) ide na
izlaz A1 ili A2 (do Entera vrijedi za cijeli A1/A2), promjena kanala ili dio spremljen iskljucen ponovno otvara program
s nacrtom. `qnc-part-sound` `kept`, `qnc-program-segments` `part_sound_stored`. Uz to zapis tipki Storyja u player log
(`story-key`, commit e4e42cf) za nalaz IN/OUT. Testovi (`a_gain_set_while_playing_is_heard_in_the_next_callback`,
`levels_become_device_gains...`), conformance. Uzivo nije provjereno.
Popravak 2026-10-16 (korisnik: "izaberem kadar u media poolu, pokrenem play, moram prebaciti na source timeline da bi
mogao odrediti IN ili OUT"): I i O oznacavaju izabrani klip i kad je tipkovnica u poolu ili na tabovima poola
(`qnc-panel-focus`); na panelu Segmenti i dalje ne rade nista (rub slota je Ctrl+I/O). Test, conformance.
Izvedeno 2026-10-16 (korisnik: "kotacic radi isto sto i strelice lijevo desno, ovisno iznad kojeg je panela"; "ok,
kreni s kotacicem"): nova javna kocka `crates/qnc-wheel-jog` (+ ugovor): kotacic iznad panela daje akciju kataloga, jedan
zarez jedan korak (touchpad zbraja svoje male pomake, 40 tocaka korak), brzo vrtenje daje jednu akciju po slici, ostatak
u sljedecim slikama; iznad panela uzima pomak pa se popis ispod ne mice. Katalog: akcije bez tipke `wheel_source_back` /
`wheel_source_forward` (source timeline i monitor, uvijek) i `wheel_segments_back` / `wheel_segments_forward` (panel
Segmenti, samo dok je nesto uzeto: marker, slot, rub slota, zvuk segmenta ili pokrivalice; inace kotacic pomice redove).
`qnc-panel-focus`: te akcije stavljaju fokus na taj panel i rade isto sto strelica lijevo/desno. `qnc-program-view`
`holds_edit`. Kotacic prema korisniku je naprijed. Ploca Storyja/MA: tri poziva kocke, C10 354 -> 353. Testovi
(`qnc-wheel-jog`), conformance. Uzivo nije provjereno.
Ispravak 2026-10-16 (korisnik: "kad kotac guram gore playhead ide na pocetak... nivo tona isto ide naopako"): kotacic
od sebe (gore) je naprijed i glasnije, prema sebi natrag (`qnc-wheel-jog`). Dok je A1/A2 uzet (source timeline) ili
zvuk segmenta/pokrivalice (Wrap redak), kotacic je razina kao Ctrl+lijevo/desno, a ne ukljuci/iskljuci
(`qnc-panel-focus`). Testovi, conformance.
Dopuna 2026-10-16 (korisnik: "dodaj postavku... zadrzi kako je, ali ton gore = +, dolje = -"): smjer playheada na
kotacicu je postavka racunala, kao jezik sucelja: `data/mouse-wheel.json` `{"playhead_up": "forward" | "back"}`, bez nje
`forward` (od sebe naprijed); neispravan zapis se zapisuje u player log i vrijedi `forward` (`qnc-wheel-jog`
`playhead_up_forward`, cita se jednom po pokretanju). Razina A1/A2 se ne okrece: gore glasnije uvijek (`qnc-panel-focus`).
Testovi, conformance.
Ispravak 2026-10-16 (korisnik: "nivo tona ne smije ici direktno na timeline, direktno je playhead; nivo tona ide isto kao
tipkovnicom Ctrl+strelice ili kotac"): kotacic sam uvijek pomice playhead (kao strelice); gdje strelice uzetom zvuku
ukljucuju/iskljucuju zvuk (uzeta traka A1/A2, zvuk segmenta ili pokrivalice) kotacic pomice playhead. Razinu mijenja samo
Ctrl+kotacic, kao Ctrl+lijevo/desno, gore glasnije. `qnc-wheel-jog` daje par akcija s Ctrl-om (`WheelActions`), katalog
`wheel_source_ctrl_back/forward`, `wheel_segments_ctrl_back/forward` (bez tipke), `qnc-panel-focus`. Testovi, conformance.
Dopuna 2026-10-16 (korisnik: "prikaz duljine u source timelineu u realnoj poziciji playheada, od IN koliko je playhead
daleko"; "ok, to je ok"): polje Duration source docka dok OUT nije postavljen pokazuje udaljenost od IN-a do playheada i
prati ga; s postavljenim OUT-om IN-OUT kao prije (`qnc-timeline` `shown_source_length`, `qnc-source-dock`). Vrijedi i za
Ingest. Test, conformance.
Dopuna 2026-10-16 (korisnik: "kad je mis iznad segmenata trebao bi raditi kao i u source timelineu, isto i source
timeline u Ingestu; source timeline mora se svugdje ponasati na isti nacin"): kotacic iznad panela Segmenti uvijek radi
kao na source timelineu (Wrap redovi se listaju trakom). Ingest: isti kotacic iznad monitora i source timelinea
(`qnc-focus-walk` prebacuje fokus i daje strelicu, ista postavka smjera; Ctrl+kotacic tamo nema sto uzeti).
`qnc-wheel-jog` `SOURCE_ACTIONS` i `source_step_forward` (jedan skup akcija za sve forme); `qnc-program-view`
`holds_edit` uklonjen. C10 `qnc-ingest-blocks` 862 -> 860. Napomena korisnika (isti dan): sve sto pripada source
timelineu i media poolu mora biti neovisno o formi; ovaj rez jos poziva kotacic iz ploca i ima dva puta fokusa
(`qnc-panel-focus` za Story, `qnc-focus-walk` za Ingest) - predlozeno je premjestanje u kocke.
Izmjena 2026-10-16 (korisnik: "sve sto se odnosi na source timeline mora biti nevezano s formom koja ga koristi, isto i
media pool; kocke i moduli, paneli, forme"; "da, kreni s kotacicem u qnc-timeline"): kotacic je dio kocke `qnc-timeline`.
Svaki timeline (source timeline u Storyju, MA-u i Ingestu, Wrap redovi) sam prepoznaje kotacic iznad sebe i javlja ga
kao svoju namjeru `TimelineIntent::Wheel` (`qnc-wheel-jog` `take`, `Wheel { up, ctrl }`, `Wheel::action`); ploce formi ga
vise ne pozivaju. Source timeline daje skup akcija `SOURCE_ACTIONS` (Story ploca kao i dosad pretvara namjere docka,
Ingest `timeline_projection`), Wrap redovi `SegmentCommand::Wheel` (`qnc-segment-panel`), koji `qnc-panel-focus`
`wheel_action` pretvara u akciju panela Segmenti. Kotacic iznad monitora je uklonjen (nije bio trazen). C10
`qnc-ingest-application` 1676 -> 1674. Ostaje (sljedeci korak, korisnik ga ceka): dva puta fokusa (`qnc-panel-focus`
Story, `qnc-focus-walk` Ingest) spojiti u jednu kocku za sve forme, pa i tipke Media poola. Testovi, conformance.
Izvedeno 2026-10-16 (korisnik: "Ingest u kocke, pa nece biti prevelik"; "racunaj da ce se dodavati jos formi, pa bi i one
trebale moci koristiti postojece kocke, zato ne spajaj kocke ako nije neophodno"): korak 1. Nova javna kocka
`crates/qnc-source-panel` (+ ugovor): tipke source timelinea, iste u svakoj formi (Play, korak, I/O, Ctrl+I/O uzimanje i
pomak strelicama/kotacicem, Esc pusta, Alt+strelice start/IN/OUT, Home); vozi univerzalni `qnc-source-preview` i
prikazani timeline, ne zna za formu ni bazu. Story (`qnc-panel-focus`) za te tipke zove kocku, svoje (Sync, A1/A2, rubovi
segmenata, I/O sa slicicama) zadrzava. Ingest: `qnc-ingest-preview-source` `source_preview_mut`, aplikacijski sloj zove
kocku umjesto svog koda za I/O i korak (Play ostaje uz cuvar playera), Esc s uzetom oznakom pusta oznaku;
`qnc-focus-walk` nove tipke propusta samo na source timelineu i gumbima docka; katalog QNC preseta, Ingest dio:
`select_mark_in/out`, `navigate_prev/next_object`, `playlist_input_start` kao u Storyju. Kocke nisu spajane: fokus Storyja
i Ingesta ostaju zasebne kocke. C10 `qnc-ingest-application` 1674 -> 1667. Testovi, conformance.
Popravak 2026-10-16 (korisnik: "dok skrolam po segment timelineu, source timeline bljeska"): log je pokazao da je svaki
korak kotacica iznad Wrap redova, dok se program jos pripremao, program otvarao iznova (nova sesija, `command_id 1`), pa
su se prikaz i playhead izmjenjivali. `qnc-source-preview` `show_program_frame`: dok se program priprema, trazena slika se
samo pamti (`program_pending`) i program na nju skoci cim je spreman; ponovno otvaranje samo kad se prica promijenila ili
priprema nije uspjela. Testovi, conformance.
Izvedeno 2026-10-16 (Ingest u kocke, korak 2): prikaz preglednika izvora (korijeni ili mapa, stavke, posao u tijeku,
greska, izabrani izvor: naziv, serijski broj, volumen) drzi kocka `qnc-source-browse` (`BrowserView`: `clear`, `apply`,
`choose`; `BrowseLocationEntry` je stavka za sve forme). Ingest prikaz ima jedno polje `browser` umjesto 11, aplikacijski
sloj vise ne puni ni ne brise ta polja; `LocationEntry` je stavka kocke. Project (`qnc-project-session`) ima isti
obrazac i moze ga preuzeti uz otkljucavanje. C10 `qnc-ingest-application` 1667 -> 1584. Testovi, conformance. Nije
prenesen u kopiju dok se trazi uzrok zastoja zvuka (vidi sljedeci zapis).
Popravak 2026-10-16 (korisnik: "program je blokirao kad sam u segmentima isao raditi korekciju slota in/out... tocak
nije radio"): log je pokazao da sucelje nije stalo nego da je rub slota uzet s Ctrl+O (strelice i kotacic su ga
pomicali) ostao uzet kad je korisnik misem otvorio klip u poolu; Esc na source timelineu ga vise nije mogao pustiti.
Sada sve sto je panel Segmenti uzeo (slot, rub slota, marker, pokrivalica, A1/A2 segmenta) odbacuje se kao Esc cim
tipkovnica ode s panela (klik, Tab, Enter); nista se ne zapisuje. Uredivanje segmenta (Ctrl+Shift+I/O, Edit) zivi na
source timelineu i ostaje. `qnc-program-segments` (`leave_panel`, test), `qnc-panel-focus` (`poll`). Kotacic i strelice
na prvom slotu (uz zakljucani pocetni marker) ne pomicu ga, po pravilu od 10. 10. (poruka u statusu).
Popravak 2026-10-16 (korisnik: "i dalje bljeska source timeline kad u segmentu tockom pokrecem playhead"; "odvoji
kocke"): snimke ekrana tijekom kotacica pokazale su da u nekim slikama cijeli source dock nije nacrtan. Ploca
(`qnc-board` `draw`) je nakon prve kocke koja vrati namjeru preskakala crtanje svih kocki iza nje; kotacic iznad panela
Segmenti daje namjeru svaki frame, pa dock (iza njega) nije crtan. Sada se svaka kocka crta neovisno o drugima, a prva
namjera je namjera ploce (jedna po frameu). Test `every_block_is_drawn_even_after_an_earlier_one_answered`,
conformance. Provjereno u izoliranoj kopiji: 20 snimaka tijekom kotacica, dock nacrtan u svakoj.
Otkljucano i izvedeno 2026-10-17 (korisnik: "opet radis posebne verzije source timeline... sve moraju biti iste, znaci
samo jedan source timeline, forme ga samo ugraduju"; "odkljucaj i napravi"; "source timeline panel je modul, source
timeline je kocka, samo source timeline dugmad moze imati drugacije funkcije"; "korisnik mora moci uredivati ton i u
Wrap-segment timelineu"): nova javna kocka `crates/qnc-source-timeline` (+ ugovor
`contracts/modules/source-timeline.module.json`) je jedini source timeline: A1/A2 trake (kanal, off/on, razina, veliki
val; `qnc-lane-choice`), broj kanala i izvorni timecode prikazanog klipa (`qnc-source-channels`,
`qnc-source-timecode`), tipke i kotacic (`qnc-source-panel`, Ctrl+kotacic razina), klik na A1/A2 i na kanal
(`action_of`, `choose_action`), crtanje kroz `qnc-source-dock` (`show`). Kocke se ne spajaju, kocka ih sastavlja.
Forme (Story/MA ploca, Ingest ploca) samo ugraduju `qnc_source_timeline::show` s vlastitim dugmadima zaglavlja.
Uklonjeno: trake iz `qnc-program-segments` (`lanes.rs`, izbor kanala, `audible/heard_channels`; prica dobiva zadrzane
trake i "Enter je zauzet" kroz `SourcePick::with_lanes`), polja traka iz `qnc-program-view`, Story dodatak source
timelineu `qnc-segment-panel::source_lanes` (`.story(..)`), Story i Ingest vlastiti prijevodi radnji timelinea, mrtvi
Ingest `ingest_toggle_audio_lane`. Ingest sada ima iste A1/A2 mogucnosti kao Story (katalog: Ctrl+1/Ctrl+2,
Ctrl+lijevo/desno u Ingest scopeu; akcije klika na kanal `source_choose_a1_channel` / `_a2_`). Uredivanje tona
spremljenog segmenta ili pokrivalice na Wrap retku ostaje (`qnc-part-sound`), a val Wrap reda prati nacrt razine.
Razina se vidi na traci: uz dugme A1/A2 pise "+N dB" ili "OFF" kad nije kako je snimljeno ili dok je traka uzeta
(`qnc-timeline` `paint_lane_level`, `ChannelChoice.heard_db`); poruka statusa Storyja se nigdje nije crtala.
C10: Story application 700 -> 699, Story ploca 353 -> 349, Ingest application 1584 (isto). Testovi, conformance.
Uzivo nije provjereno.
Promjena pravila i izvedba 2026-10-17 (korisnik: "i dalje ne koristis univerzalnu timeline kocku.. to je osnova";
potvrdio: upravljanje trakama A1/A2 prelazi u samu kocku `qnc-timeline` i jednako je na source timelineu, Wrap redovima
i redu programa; Ctrl+kotacic mijenja razinu tek nakon Ctrl+1/Ctrl+2): dopuna pravila 8.1 - timeline i dalje nema
playback sat, playhead, IN/OUT ni bazu, ali pravila A1/A2 traka su njegova: koja tipka sto radi traci (`LaneKey`:
Ctrl+1/Ctrl+2 uzima, gore/dolje kanal, lijevo/desno off/on, Ctrl+lijevo/desno i Ctrl+kotacic razina, Enter zadrzi,
Esc odbaci), rijeci statusa (`lane_message`) i stupac kanala s razinom uz A1/A2 (`lane_choice`, `drafted_choice`).
Stanje traka drzi vlasnik (`qnc-lane-choice`), on kaze cemu traka pripada i zapisuje sto je Enter zadrzao.
`qnc-timeline` (`lanes.rs`, ugovor `timeline.module.json` `audio_lanes_policy`), `qnc-source-timeline` (tipke traka
idu kroz `LaneKey`), `qnc-part-sound` (`lanes_mut`), `qnc-program-segments` (bez vlastitih koraka kanala/off/razine,
`part_sound_lanes`), `qnc-panel-focus` (Wrap redak: jedna grana za tipke traka kroz `qnc_timeline::lane_key`),
`qnc-segment-panel` (`drafted_choice`). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-17 (korisnik: "i dalje nije zajednicka univerzalna... u Storyju ESC zatvara otvoreni audio zoom, u
Ingestu ne... napravi audit source timeline i koristi jedan kod"): audit je nasao da su tipke source timelinea dolazile
iz kataloga po formi: u Ingestu je Esc bio `ingest_dir_cancel` (Odustani preglednika), u Storyju `clear_focus`; I/O iz
poola radio je u Storyju, u Ingestu ne. Sada katalog ima opseg `source_timeline` (svih sest preseta, kljucevi preuzeti
iz istog preseta), `qnc-key-intents::action_ids_in` cita tipku najprije u opsegu panela u fokusu (tipka koju taj opseg
zna samo je njegova), `qnc-focus-walk` / `qnc-panel-focus` `key_scope()` kazu kad source timeline ima tipkovnicu
(`qnc_source_timeline::KEY_SCOPE`). Ingest: Esc na source timelineu je `clear_focus` (odbacuje uzetu traku ili
oznaku), I/O iz kartica oznacava izabrani klip kao u Storyju; uklonjen Ingest prijevod Esc -> `clear_focus`.
Ostalo iz audita (nije izvedeno): Ingest sam cita filmstrip/wave source timelinea (`timeline_artifacts.rs`), Story
kroz `qnc-source-preview`; obje aplikacije same cuvaju IN/OUT pri osvjezavanju prikaza; cue s timelinea ide u Ingestu
kroz `ingest_cue_frame` s cuvarom playera, u Storyju izravno. C10 Ingest application 1584 -> 1583.
Nastavak 2026-10-17 (korisnik: "ocisti i premjesti ostatak u kocku.. nedostaje opcija Ctrl+I/O za kontrolu IN i OUT
strelicama ili tockom"): log je pokazao da je Ctrl+I iz poola bio odbijen u Storyju (kotacic je pomicao playhead) i
Ingestu. Sada Ctrl+I/O iz svakog panela (osim panela Segmenti, gdje su rubovi slota) uzima IN/OUT izabranog klipa i
prebacuje tipkovnicu na source timeline, gdje ga strelice i kotacic pomicu (`qnc-panel-focus`, `qnc-focus-walk`).
Ostatak iz audita premjesten: filmstrip i val source timelinea cita samo zajednicki preview (`qnc-source-preview`
`read_missing_assets`: ponovno citanje jednom u sekundi dok ih radnik radi, za sve forme; `forget_assets`), Ingest
vise ne cita artefakte sam (`timeline_artifacts.rs` samo budi radnika); klik na source timeline ide kroz kocku
(`qnc-source-timeline` `cue`) u obje forme; kocka zapisuje svaku tipku source timelinea u log (`source-key`), isto
za sve forme. Ocisceno: `cargo clean --profile dev` (disk 3 GB -> 46 GB). C10 Ingest application 1583 -> 1567.
Ostaje u aplikacijama: cuvanje IN/OUT pri osvjezavanju prikaza (jedan redak, Story uz to otvara kadar s IN/OUT).
Dopuna 2026-10-17 (korisnik: "Enter ili lijevo dugme misa sprema promjenu edita Ctrl+I/O"; "vec neko vrijeme odgadamo
dodavanje funkcije Ctrl desnom dugmetu misa"): u univerzalnom `qnc-timeline` lijevi klik dok je IN ili OUT uzet je
`KeepMark` (kao Enter: promjena ostaje, oznaka se pusta), a desni klik je Ctrl+ (uzima ono na sto pokazuje): na source
timelineu blizi IN ili OUT (`TakeMark`), na A1/A2 traku (`TakeAudioLane`), na Wrap retku M marker (`TakeMarker`, kao
Ctrl+M), slot (`TakeSlot`, kao Ctrl+S) ili zvuk segmenta/pokrivalice na kliknutom mjestu (`TakeSound`, kao Ctrl+1/2).
`qnc-source-timeline` (`action_of`; `route_input` s Enterom pusta uzetu oznaku, pa Enter sprema i u Ingestu, gdje ga je
prije uzimao krug panela), `qnc-program-commands` (`TakeMarker`, `TakeSlot`, `TakeSound`), `qnc-program-segments`,
`qnc-segment-panel`; pomoc F1 (katalog, pravilo "Desni klik misem"). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-17 (korisnik: "isti sistem trebali primijeniti i na rad na Wrap-segment timelineu"; "desni klik treba
biti opcija Ctrl, isto kao i na timelineu"): Wrap redovi rade kao source timeline. Lijevi klik dok je na retku nesto
uzeto (slot, rub slota, marker, zvuk segmenta ili pokrivalice) sprema kao Enter (`SegmentCommand::Keep`,
`qnc-segment-panel`, `qnc-panel-focus::wheel_action` ga daje kao `activate_focused_item`). Desni klik je Ctrl+ za sve
sto Ctrl radi na retku (`qnc-timeline`): M marker (Ctrl+M), blizu ruba slota taj rub (Ctrl+I/O, `TakeSlotEdge`), u
slotu slot (Ctrl+S), pokrivalica izvan slota (Ctrl+klik, `TakeCover`), A1/A2 zvuk na kliknutom mjestu (Ctrl+1/2),
oznaka V filmstrip/uska traka (Ctrl+klik). `qnc-program-commands` (`Keep`, `TakeSlotEdge`), `qnc-program-segments`.
Testovi, conformance. Uzivo nije provjereno.
Izmjena izgleda 2026-10-17 (korisnik: traka pokrivalice na Wrap retku "s gornjeg polozaja prebacio na donji, visinu
povecao za 50%, ne bi trajno bila vidljiva, vec samo kad je mis iznad nje ili je selektirana"): `qnc-timeline`
(`COVER_STRIP` 13 -> 19,5 px, `cover_rows`: dno slike, `paint_covers`: preko filmstripa samo pod misem ili kad je
pokrivalica odabrana). Uska video traka (bez filmstripa) i dalje uvijek pokazuje pokrivalicu, jer nema svoje slike.
Klik i desni klik na mjestu trake rade kao prije.
Izmjena 2026-10-17 (korisnik: "klik na color oznaku selektira slot, a klik izvan kolor oznake ... to je samo
postavljanje playholdera.. ctrl+ prebacuje playholder na in ili out i nakon toga tockom ili strelicama moze se raditi
isto sto i na source timelineu"): na Wrap retku lijevi klik na kolor traku pokrivalice bira slot koji pokriva (Ctrl+klik
na nju i dalje uzima pokrivalicu), lijevi klik drugdje samo postavlja playhead i pusta izabrani slot koji playhead
napusti (pa Ctrl+I/O uzimaju slot pod playheadom). Desni klik (Ctrl+) na kolor traku uzima slot (Ctrl+S), drugdje u slotu
uzima bliži rub slota kao Ctrl+I/O: playhead ide na taj rub, kotacic i strelice ga pomicu, Enter ili lijevi klik sprema,
Esc odustaje. `qnc-timeline` (lijevi klik u slot vise ne daje `SelectMarkerSlot`; desni klik bez granice od 8 px),
`qnc-segment-panel` (klik na traku -> `SelectSlot`), `qnc-program-segments` (`Cue` pusta napusteni slot). Testovi,
conformance.
Audit 2026-10-17 (korisnik: "napravi audit Wrap-segmenta sa svim dodanim opcijama... na prvom klipu ne prihvaca Ctrl
misem editiranje OUT ni IN; na drugom cijelo vrijeme stoji prikazana traka s vremenom trajanja"): nalazi iz loga i koda,
popravljeno: (1) traka pokrivalice ovisila je o tome jesu li slike filmstripa reda vec procitane, ne o nacinu prikaza;
red bez slika crtao je pokrivalicu preko cijelog video dijela, stalno, i svaki klik ju je pogadao (desni klik uzimao je
slot umjesto ruba, a prvi slot uz pocetni marker se ne mice: "A slot at the start or end M marker does not move"). Sada
je to nacin reda (`qnc-timeline` `cover_strip`: filmstrip nacin, ne uska traka). (2) Pokrivalica odabrana nekad prije
ostajala je odabrana u bazi zauvijek (traka stalno vidljiva, Delete bi je obrisao); klik koji samo postavlja playhead
ili bira slot je pusta (`drop_selected_cover`); traka se vidi i kad je odabran njezin slot (samo crtanje,
`qnc-segment-panel`). (3) Pokrivalica vec dulja od svog klipa nije se dala ni skratiti (strelice i kotacic u oba smjera:
"The clip is at its end"); provjera kraja klipa vraca samo pomak dalje u izvor (`past_clip_end(direction)`).
(4) Lijevi klik na pojasu A1/A2 Wrap reda nije radio nista (pojas je uzimao klik); sada postavlja playhead ili sprema
uzeto kao klik na redu. (5) Jedno uzeto u isto vrijeme: desni klik na marker dok je slot uzet ostavljao je oba, strelice
su micale slot; svako uzimanje (slot, rub slota, marker, zvuk) pusta prethodno bez zapisa (`let_go_edits`; nacrt zvuka
koji se tako pusti vraca spremljeni zvuk, `take_dropped_sound`). (6) Mišje naredbe panela Segmenti pisu se u player log
(`story-click`). Testovi (`qnc-program-segments` 38), conformance. Otvoreno (nije mijenjano, po pravilima): zadnji slot
price nema rubove (iza njega nema slota koji bi preuzeo razliku), slot uz pocetni ili zavrsni marker se ne mice, u uskoj
video traci pokrivalica pokriva cijeli video dio pa desni klik tamo uzima slot (rubovi samo tipkama), A2 desni klik bez
pokrivalice na kliknutom mjestu uzima odabranu pokrivalicu.
Promjena pravila 2026-10-17 (korisnik: "izmedu prvog i drugog klipa postoji rupa kroz koju se vidi klip iz pozadine ili
filmstrip; to se ne bi smjelo dogoditi, jer M marker je zamisljen kao OUT/IN granica"; izbor "odbij promjenu"; "nije
bitan trenutni projekt, bitno je da se to ne moze dogoditi u novom"): zamjenjuje pravilo od 11. 10. ("pokrivalica koja
prelazi kraj klipa pusta se do kraja, ostatak slota pokazuje segment ispod"). Log probe: prvi slot 0-347, pokrivalica od
slike 20 klipa od 314 slika (rupa 53 slike, nastala ranije), skracen na 300 (rupa 6 slika). Sada nijedna izmjena price ne
smije ostaviti pokrivalicu kracom od njezina slota: `qnc-program-db` `execute` prije i poslije svake izmjene price mjeri
koliko slika svakoj pokrivalici nedostaje do kraja slota (`cover_shortfalls`, duljina klipa iz jedinog citaca kataloga
`qnc-content-read::duration_frames_on`); ako je izmjena napravila ili povecala rupu, cijela se vraca (`undo::put_back`, bez
koraka Undo) i odbija porukom `story.cover_hole`. Postojece rupe se ne popravljaju. Test
`no_edit_leaves_a_cover_shorter_than_its_slot`, conformance. Otvoreno: odbijeni CreateCover ostavlja B-roll kadar koji je
pokrivalica vec stvorila (dva upisa, kao i prije kod greske).
Popravak 2026-10-17 (korisnik: "ne radi Ctrl+Delete za selektirani slot"; log: klik na kolor traku izabere slot, Delete i
Ctrl+Delete daju "First take a marker or a B-roll to remove"): Delete/Ctrl+Delete (`delete_marker`, `DeleteFocused`)
nakon uzetog markera i uzete pokrivalice uzima i slot: uzet (Ctrl+S, desni klik na traku) ili izabran (klik na traku);
brise se njegova B-roll pokrivalica, slot i M markeri ostaju; slot bez pokrivalice javlja `story.msg.slot_has_no_broll`.
`qnc-program-segments`, rjecnik, test. Katalog daje istu akciju za Delete i Ctrl+Delete, pa radi i obicni Delete.
Popravak 2026-10-17 (korisnik: "ne mogu dodati B-roll na postojeci prazni slot sa Shift+B ni dugmetom B-roll"; "ako ima
IN-OUT na source timelineu svaki drugi klik misem stvara SubClip"): (1) Log: lijevi klik na source timeline, dok je
timeline crtao IN/OUT uzetim (a tipke ga vise nisu drzale: I/O su ga ostavljali uzetim), bio je Enter; Enter vodi krug
panela na dugmad docka, pa je sljedeci klik pritisnuo prvo dugme, Add SubClip. Sada taj klik ima svoju akciju kataloga
`source_keep_mark` (bez tipke): oznaka ostaje gdje je i pusta se, nikad Enter (`qnc-source-timeline` `KEEP_MARK`,
`qnc-panel-focus`, Ingest isti put kroz `route_input`); I i O pustaju uzetu oznaku i na timelineu. (2) Prazni slot nema
kolor traku pa ga lijevi klik ne bira (pravilo od 17. 10.), a B/Cover slot i Overwrite trazili su izabrani slot: sada
uzimaju izabrani, inace slot pod Wrap playheadom (`qnc-program-view` `slot_at_playhead`). Shift+B (Shift+ dodaje) u QNC
presetu radi isto sto B (katalog, iznimka od qnc_v4 u `tools/qnc-conformance`). Dugme "B-roll" u docku ostaje prekidac
nacina (B-roll | Sync/B-roll), ne dodaje pokrivalicu. Testovi, conformance.
Dopuna 2026-10-17 (korisnik: "Shift+B radi, dugme B-roll ne radi"; log: tri klika na dugme daju samo izbor nacina):
dugme B-roll u docku (i P) uz izbor nacina B-roll stavlja oznaceni izvor (IN/OUT) u prazni slot, izabrani ili pod Wrap
playheadom, kao B; bez IN/OUT-a ili bez praznog slota samo bira nacin. `qnc-program-segments` (`CoverMode`).
Audit 2026-10-17 (korisnik: "opet nije jedinstveni timeline: na Wrap retku desni klik na ton otvara zoom, lijevi vraca
filmstrip, u source to ne radi; izgubilo se pojacavanje tona desnom tipkom misa i kotacicem"): usporedba A1/A2 na source
timelineu i Wrap retku, popravljeno u zajednickim kockama: (1) lijevi klik dok je A1/A2 uzet sprema traku i zatvara
povecanje i na source timelineu (`qnc-timeline` `lane_taken` -> `KeepMark`, `qnc-source-timeline` `KEEP_MARK` zadrzava
traku); (2) kotacic dok je desna tipka drzana je Ctrl+kotacic (razina uzetog zvuka) svugdje, a pustanje te tipke nakon
kotacica nije desni klik (`qnc-wheel-jog` `wheeled_with_right`, `qnc-timeline` `right_click`); (3) lijevi klik na dugme
A1/A2 Wrap retka uzima zvuk onoga sto je pod Wrap playheadom u retku (inace na pocetku retka), kao Ctrl+1/Ctrl+2, a dok
je zvuk uzet, sprema ga (`qnc-segment-panel`). Jednako: desni klik na traku uzima, strelice gore/dolje kanal, lijevo/desno
off/on, Ctrl+strelice i Ctrl+kotacic razina, Enter ili lijevi klik sprema, Esc odustaje. Test
`a_left_click_keeps_a_taken_lane_as_enter_does`, conformance.
Otkljucano i izvedeno 2026-10-17 (vanjski audit, ocjena 6,5/10, nalazi P1-1 i P1-2; korisnik: "da, otkljucaj i kreni s 1
i 2"): otkljucan `qnc-program-db` s ugovorom, testovima i rjecnikom. (1) Starija shema price: citac je provjeravao samo
postoje li tablice, pa je pucao na stupcima koje dodaje pisac (`no such column: a1_level_db`). Sada citac price starije
sheme odbija (`SCHEMA_COLUMNS`, `schema_outdated`, poruka `story.schema_outdated`), a `StoryReader::open` tada najprije
pusta vlasnika da shemu dovede na tekucu (`prepare_schema`: kratka veza za pisanje istog modula, jedna transakcija), pa
cita; citac sam nikad ne mijenja bazu. (2) Atomski upis: svaki upis price je jedna transakcija (`BEGIN IMMEDIATE` u
`execute`, transakcije operacija su savepointi u njoj, Undo/Redo bez vlastite transakcije): izmjena, provjera "bez rupe"
i korak Undo upisuju se zajedno ili nikako. Ukinuto vracanje snimke nakon upisa (`undo::put_back`), koje je drugom
procesu moglo pokazati nevaljano stanje ili pregaziti njegovu izmjenu. Testovi
`a_story_of_an_older_schema_is_not_read_until_the_owner_brings_it_up`, `a_refused_edit_writes_nothing_not_even_an_undo_step`,
svi testovi `qnc-program-db` (35), `qnc-program-segments`, `qnc-program-input`, `qnc-db-broker`, `qnc-virtual-shots`,
`qnc-editorial-application`, conformance. Sljedeci koraci audita (3-9) cekaju odobrenje.
Izvedeno 2026-10-17 (vanjski audit, P1-3; korisnik: "da, otkljucaj i kreni s 3 i 4"): jedna snimka price. Novo citanje
`qnc-program-db` `ReadProgramSnapshot` (`StoryReader::program_snapshot`) cita segmente, pokrivalice, markere, slotove,
odabir i stopu price u jednoj transakciji citanja, uz reviziju price (`story_revision`, zasebna tablica izvan snimke za
Undo: raste sa svakom izmjenom, Undo i Redo, nikad ne ide unatrag; odabir je ne mijenja). Istu snimku koriste player
i prerender (`qnc-program-input`, revizija ide u playlistu umjesto 0) i prikaz price (`qnc-program-segments`). Priča
bez tablice revizije je starija shema (citac je odbija, vlasnik je dovodi na tekucu). Test
`the_program_snapshot_has_a_revision_that_grows_with_every_change_and_never_goes_back`, conformance.
Izvedeno 2026-10-17 (vanjski audit, P1-4): atomski najam "jedan po projektu". Prije: proces je procitao je li najam ziv,
pa ga zasebno upisao "on", bez vlasnika; dva procesa mogla su oba proci, a kraj jednoga upisivao je "off" dok drugi radi.
Sada `qnc-content-store` (vlasnik tablice `ingest_runtime`; otkljucan uz ovaj korak jer je bez njega korak nemoguc) ima
operaciju `Lease` (`LeaseAct::Claim`/`Renew`/`Release`): svaka je jedan uvjetni SQL upis, vrijednost je `on:<vlasnik>`,
preuzima se samo kad nitko ne drzi, drzac ga je pustio, nije obnovljen dulje od isteka ili je vec tog vlasnika; obnova i
otpustanje samo dok je jos tog vlasnika. `qnc-playback-activity` `Lease::claim` (vlasnik = proces i trenutak, obnova
svake sekunde, `held()` postaje false kad ga drugi preuzme) zamjenjuje `Beat`. Koriste ga `tools/qnc-ingest-worker`
(staje i kad izgubi najam), `qnc-ingest-import-worker` (`run_service`) i `tools/qnc-prerender` (isti pogresan obrazac);
`is_fresh` cita i `on:<vlasnik>`. `qnc-ingest-select` novi odgovor pisaca tretira kao neispravan u svom toku. Test
`a_lease_is_one_holders_and_its_end_lets_go_only_of_its_own`, testovi `qnc-content-store`, `qnc-ingest-import-worker`,
`qnc-ingest-worker`, `qnc-ingest-select`, `qnc-prerender-launch`, `qnc-ingest-application`, conformance.
Popravak 2026-10-17 (korisnik: "na Story stranici nije se vidio sadrzaj panela Segmenti, tek kad sam dodao novi segment
pojavili su se i stari"; log: "The story of this project was written by an older version..."): posrednik baze priklјucuje
modul tablica tek pri prvom zahtjevu, ne pri otvaranju, pa se starija shema (bez `story_revision` iz koraka 3) javila
pri citanju, gdje je ponovni pokusaj nije hvatao, a ni nadogradnja vlasnika nista nije radila (samo je otvarala vezu).
Sada `StoryReader` na tu gresku pri citanju pusta vlasnika da shemu dovede na tekucu i cita jos jednom, a
`prepare_schema` jednim citanjem kroz vezu za pisanje priklјuci modul (koji tada nadogradi shemu). Test kroz pravi put
posrednika `the_story_reader_brings_an_older_story_up_through_its_owner_and_reads_it` (pada na prethodnoj verziji).
Izvedeno 2026-10-17 (vanjski audit, P1-5; korisnik: "da, otkljucaj i kreni s 5 i 6"): sucelje ne ceka bazu ni nit.
`qnc-editorial-application`: mapa projekta, ciljevi baze sadrzaja i price i citac artefakata citaju se uz katalog u
pozadinskoj niti (`read_catalog`, polja `Loaded`), a `apply_loaded` na niti sucelja samo ih preuzme. `qnc-playback-activity`:
`PlaybackReporter` pri promjeni projekta ne ceka svoju nit (`stop` bez `join`; nit pise sto je primila i zavrsi sama),
a pisac reportera otvara se u njegovoj niti, ne na niti sucelja. C10 Story aplikacija 699 -> 689 (baseline spusten).
Testovi, conformance.
Izvedeno 2026-10-17 (vanjski audit, P1-6): pisac price provjerava ono sto klijent salje prema spremljenom zapisu klipa.
`qnc-program-db` `require_source_range` (novi segment, zamjena segmenta, pokrivalica, skracivanje segmenta): stopa koju
klijent salje mora biti spremljena stopa klipa, a OUT unutar klipa; klip bez spremljene duljine ili stope se odbija, nikad
se ne pogada. Citanje kroz jedini citac kataloga `qnc-content-read::clip_timing_on` (`public_clips`). Testni klipovi
dobili su spremljenu duljinu i stopu kao stvarni (`set_clip_timing`; `qnc-program-segments` db testovi); testovi stope
price uskladuju zapis klipa prije upisa. Provjereno citanjem kopije projekta: svih 105 klipova ima duljinu i stopu.
Testovi, conformance.
Izvedeno 2026-10-17 (vanjski audit, P2-7; korisnik: "da, otkljucaj i kreni s 7, 8 i 9"): odbijena pokrivalica ne ostavlja
B-roll kadar. B-roll kadar i pokrivalica pisu dva modula tablica sa zasebnim vezama (jedna SQL transakcija nije moguca bez
promjene posrednika), pa se odbijeni drugi korak ponistava prvim: `qnc-virtual-shots` operacija `ForgetCoverShot` brise
B-roll kadar samo dok ga nijedna pokrivalica price ne koristi; `qnc-program-segments` je salje kad je upis pokrivalice
odbijen (`cover_writes`) i za taj kadar ne pravi poster. Otvoreno: pad programa izmedu dva koraka i dalje moze ostaviti
kadar. Test `a_refused_cover_leaves_no_b_roll_shot_behind`.
Izvedeno 2026-10-17 (vanjski audit, P2-8): broj kanala za A1/A2 segmenta ili pokrivalice na Wrap retku cita se iz
spremljenog zapisa medija klipa (`qnc-source-channels::read_clip`, u istoj pozadinskoj niti kao snimka price, samo za
klipove koji jos nisu procitani; `qnc-program-segments` `clip_channels`), ne iz vala; val ostaje prikaz. Test
`a1_of_a_segment_takes_its_channel_count_from_the_media_record_not_the_wave`.
Izvedeno 2026-10-17 (vanjski audit, P2-9): pregled razine nacrta na Wrap retku zvuci kao zapisana izmjena. Izlazna razina
playera drzi najvise 400 % (`SetOutputLevels`), pa je npr. spremljeno -20 dB s nacrtom 0 dB (1000 %) zvucalo tise nego
nakon Entera. Sada nacrt koji izlaz ne moze tocno dati otvara program s nacrtom (kao promjena kanala), bez ogranicenja
(`qnc-source-preview` `draft_output_level`, `OUTPUT_LEVEL_MAX_PERCENT`). Test
`a_draft_level_beyond_what_the_output_holds_opens_the_program_with_the_draft`, conformance. Svi koraci vanjskog audita
(1-9) su izvedeni.
Dopuna 2026-10-17 (korisnik: "svi klipovi bez naknadne oznake po defaultu imaju IN (pocetak) i OUT (kraj); kako sve
klipove tretiramo kao virtualne klipove, trebalo bi ih tako i tretirati; naknadni IN ili OUT koji postavi korisnik ima
prednost; Ctrl+kotacic treba moci voziti defaultni IN ili OUT"): `qnc-source-mark-focus` `take` uzima IN ili OUT koji
timeline pokazuje (korisnikov, inace pocetak ili kraj klipa); zadana oznaka postaje korisnikova kad se pomakne
(`nudge`); `taken_picture` isto. `qnc-timeline`: Ctrl+kotacic (i kotacic s drzanom desnom tipkom) na source timelineu
kad nista nije uzeto uzima IN ili OUT blizi misu; `qnc-source-timeline`: Ctrl+kotacic pomice uzetu oznaku kad A1/A2
nije uzet (uz uzet A1/A2 i dalje mijenja razinu). Vrijedi za sve forme (Story, MA, Ingest). Test
`without_marks_ctrl_i_and_ctrl_o_take_the_clips_start_and_end_and_moving_one_sets_it`, conformance.
Izvedeno 2026-10-17 (drugi vanjski audit, ocjena 7/10, nalaz P1-2; korisnik: "da, otkljucaj i kreni s 1 i 2"): ID B-roll
kadra (i shorta) vise ne broji retke. Brojanje je s brisanjem kadra odbijene pokrivalice (moj korak 7) sljedecem kadru
davalo vec iskoristen ID, pa je upis padao. `qnc-virtual-shots`: tablica `virtual_shot_numbers` (brojac po klipu i vrsti,
samo raste, nikad ispod najveceg broja koji ID te vrste vec ima), `next_index`; ugovor baze dopunjen tablicom. Test
`a_b_roll_shot_never_takes_the_id_of_one_that_was_or_is_still_there`.
Izvedeno 2026-10-17 (drugi vanjski audit, P1-1): rad pod najmom staje kad najam vise nije njegov. `qnc-playback-activity`
`Lease`: obnova koja ne uspijeva dulje od isteka najma (greska baze) znaci gubitak najma; zastavica `lost()` je prekid
posla pod njim; `confirm()` u bazi potvrduje da je najam jos tog vlasnika. `qnc-ingest-import-worker`: uvoz je vezan uz
svoj posao (`ImportRun`: projekt za koji je najam uzet, najam, prekid); kad je aktivan drugi projekt, ne radi nista;
ne preuzima klip i ne objavljuje ishod uvoza ni rezultat workera dok `confirm()` ne potvrdi najam (`RunQueue`).
`tools/qnc-ingest-worker` predaje uvozu svoj projekt, najam i prekid; `tools/qnc-prerender` ne objavljuje dio ni ne brise
stare bez potvrdenog najma. Test `a_lease_another_owner_took_is_not_confirmed_and_its_work_is_cancelled`.
Izvedeno 2026-10-17 (drugi vanjski audit, P2-3; korisnik: "da, otkljucaj i kreni s 3 i 4"): kontekst previewa gradi se
izvan niti sucelja. `qnc-editorial-application` `read_catalog` gradi `PreviewContext::new` (lokacija projekta, citaci,
cilj baze sadrzaja) u pozadinskoj niti kataloga, a `apply_loaded` ga samo preda previewu; `qnc-source-preview`
`PreviewContext` nosi i razrijesen cilj za javljanje rada playera, pa `configure` na niti sucelja ne cita bazu.
Testovi, conformance.
Izvedeno 2026-10-17 (drugi vanjski audit, P2-5): reporter playera ne utihne nakon greske. Pisac baze sadrzaja otvara vezu
tek pri upisu, pa greska moze doci i tijekom rada; prije su se greske upisa tiho preskakale s istim pokvarenim piscem.
`qnc-playback-activity`: nit reportera na neuspio upis odbaci pisac i otvori novi pri sljedecem upisu, a stanje playera
i zadnji klip zadrzi dok ih ne upise (`tell`); greska ide u player log. Ako nit ipak stane, `PlaybackReporter` je za isti
projekt pokrece ponovno nakon 2 s i ponovno salje stanje i klip (`reporting`, `retry_at`). Test
`a_reporter_whose_writes_failed_tells_its_state_once_the_database_is_back`, conformance.
Popravak 2026-10-17 (korisnik: "lijevi klik na source timeline ne postavlja marker, nego i dalje zadrzava Ctrl skrol
kotaca"; log Ingesta: dok je IN uzet, klikovi su postali `ingest_cue_frame`): `qnc-timeline` je oznaku spremao samo na
cisti klik; pritisak drzan trenutak ili s pomakom od piksel-dva egui racuna kao povlacenje, pa je klik pomicao playhead,
a IN je ostajao uzet. Sada dok je IN/OUT ili A1/A2 uzet svaki pritisak lijeve tipke (brz ili spor) sprema i pusta oznaku
(`KeepMark`), a do pustanja tipke ne pomice nista (`qnc_timeline_kept_press`); desna tipka to ne radi. Vrijedi za sve
forme.
Dopuna 2026-10-17 (korisnik: "programi za montazu imaju i povlacenje IN/OUT oznake misem; mi imamo skrolom kotaca; mozemo
li dodati i povlacenje"): na source timelineu (sve forme) pritisak lijeve tipke do 6 piksela od IN ili OUT oznake (bez
korisnikove oznake: pocetak ili kraj klipa) i povlacenje vuce tu oznaku; nikad preko druge ni izvan klipa, playhead
pokazuje sliku na rezu, pustanje tipke je sprema. `qnc-timeline` (`DragMark`, `mark_drag_intent`; sam nista ne mice),
`qnc-source-mark-focus` (`drag`), `qnc-source-timeline` (`drag`, `timeline_input`, akcije kataloga bez tipke
`source_drag_in` / `source_drag_out`; `action_of` i `route_input` nose vrijednost i64 za kanal ili sliku). Story i
Ingest samo predaju namjeru kocki (C10 Story aplikacija 689 -> 687). Test
`a_dragged_mark_follows_the_mouse_but_never_crosses_the_other_or_leaves_the_clip`, conformance.
Izmjena izgleda 2026-10-17 (korisnik: "trebas dodati skrol u segment panel"): kotacic iznad Wrap reda pomice njegov
playhead (pravilo od 16. 10.), pa se redovi nisu imali cime listati (plutajuca traka gotovo se ne vidi). Wrap redovi
panela Segmenti imaju punu traku za listanje uz desni rub, uvijek vidljivu (`qnc-segment-panel`); lista se kotacicem iznad
nje ili povlacenjem. Redovi su zato uzi za sirinu trake.
Dopuna 2026-10-17 (korisnik: "napravi vidljiviji vertikalni skrol sirine cijele desne margine"): traka za listanje Wrap
redova ispunjava desnu marginu panela Segmenti (10 px, `PANEL_MARGIN`), pa redovi zadrzavaju svoju sirinu; boje su
kontrastne (podloga boja ruba panela, rucka prigusena boja teksta, pod misem boja teksta, dok se vuce boja playheada).
`qnc-segment-panel`. Kotacic iznad Wrap reda i dalje pomice playhead.
Dopuna 2026-10-17 (korisnik: "i strelice gore-dolje bi mogle skrolati"): strelice gore/dolje na panelu Segmenti (kad
nista nije uzeto) i dalje biraju prethodni ili sljedeci segment i stavljaju Wrap playhead na njegov pocetak, a panel se
sada sam lista do novo odabranog reda (`qnc-segment-panel`, `scroll_to_rect`). Dok je nesto uzeto, gore/dolje ostaju
skok na kljucnu sliku ili izbor kanala.
Otkljucano i izvedeno 2026-10-18 (treci vanjski audit, koraci 1 i 2; korisnik: "da, obrisi i kreni s 1 i 2"):
(1) Upis pod najmom provjerava vlasnik baze u istoj transakciji kao upis. Nova javna kocka `crates/qnc-write-lease`
(+ ugovor `contracts/modules/write-lease.module.json`): najam (kljuc, vlasnik, svjezina) i provjera u bazi (samo
citanje `ingest_runtime`, sat baze), te `begin_write` (vlastita immediate transakcija ili savepoint unutar transakcije
koju je otvorio posrednik). `qnc-db-broker`: zahtjev nosi `lease`; upis pod najmom posrednik izvodi kao BEGIN IMMEDIATE,
provjera najma, upis modula, COMMIT (inace ROLLBACK, nista se ne upise); `ProjectDbWriter::leased`,
`ProjectDbClient::execute_leased`; `TableModule::connection`. Isto u `qnc-content-store` (`Request.lease`,
`ContentClient::leased`, `ContentWriteTransport::start_leased`). Moduli `qnc-artifact-db`, `qnc-prerender-db`,
`qnc-media-record-db`, `qnc-source-index-db` pisu kroz `begin_write`. `qnc-playback-activity` `Lease::write_lease`.
Pod najmom radnika (`tools/qnc-ingest-worker`) pisu: uvoz (`qnc-ingest-import-worker`: red, rezultat, posteri),
filmstrip i wave (`qnc-content-artifacts` `set_write_lease`, `ArtifactWriter::leased`), dovrsetak zapisa
(`qnc-record-completion`: zapisi medija, izvorni indeks, objava kataloga; `Run.lease`) i prerender
(`PrerenderWriter::leased`). (2) Prerender: posao se prekida i kad najam preuzme drugi (`lease.lost()`), jedinica se
renderira u vlastitu datoteku posla (`<otisak>.partial-<proces>-<trenutak>.mov`) i tek cijela premjesta pod svoje
ime, pa objavljuje pod najmom; ostaci nedovrsenih poslova brisu se s ciscenjem starih jedinica. Conformance: posrednik i
`qnc-write-lease` dodani na popis javnih DB pisaca (nova granica: posrednik otvara transakciju upisa pod najmom).
Testovi: `qnc-write-lease`, posrednik i spremiste sadrzaja (upis pod tudim najmom odbijen, nista upisano, citanje
radi), svi testovi dotaknutih kocki, conformance.
Otkljucano i izvedeno 2026-10-18 (treci vanjski audit, ostali koraci; korisnik: "da, otkljucaj i kreni s ostalim
koracima"): (1) Broj audio kanala klipa za A1/A2 na Wrap retku: procitan broj se pamti, a klip ciji broj nije procitan
(zapis jos nije konacan ili citanje nije uspjelo) cita se ponovno pri sljedecem citanju price i odmah kad se uzme
njegov zvuk (`qnc-program-segments` `counted`, `part_sound`; test). (2) Test reportera playera vise ne mice datoteku
baze (preimenovanje otvorene datoteke na Windowsima je padalo): baza privremeno pripada drugom projektu pa svako
otvaranje pisaca odbija, zatim se vraca (`qnc-playback-activity`; 3 od 3 prolaza). (3) B-roll kadar pokrivalice
(odluka: zapis u bazi, jer jedna transakcija kroz posrednika trazi da moduli dijele vezu i ruse svoju zastitu tablica):
`qnc-virtual-shots` stupac `cover_pending` (stari kadrovi 0); `CreateCoverShot` pise kadar s 1, `ConfirmCoverShot`
ga potvrduje kad je pokrivalica upisana (`qnc-program-segments` nakon upisa pokrivalice), `ForgetCoverShot` kao prije za
odbijenu; `ForgetUnconfirmedCoverShots` pri otvaranju price potvrduje nepotvrdjene kadrove stare preko minute koje
pokrivalica koristi, a ostale brise (pad programa izmedu dva upisa vise ne ostavlja kadar). Nepotvrdjen kadar nije u
tabu B-roll (`public_b_roll_clips`). Ugovor `virtual-shots.database.json`. Testovi `qnc-virtual-shots`,
`qnc-program-db`, `qnc-program-segments` (`a_written_cover_confirms_its_b_roll_shot`), conformance.
Popravak 2026-10-18 (korisnik: "filmstrip se kreira sporo"; "jedan prolaz radi i jedno i drugo, moze i s dvije
jezgre"): provjera nije nasla promjenu jednog prolaza (u logu su val i filmstrip svakog klipa nastajali zajedno, 98
klipova za oko 5 min, kao 16. 10.). Filmstrip otvorenog klipa kasnio je jer je njegov prolaz (2002, 3,4 min) citao karticu
uz drugi dugi prolaz (2676), pa su trajali 55 s i 45 s. `qnc-wave-worker` (`preferred`, `preferred_alone`, `start_one`):
prolaz klipa otvorenog u previewu cita karticu sam; ostali prolazi se prekidaju i vracaju u red (kao za player) i
cekaju da on zavrsi. Dekodiranje slika filmstripa i dalje ide na drugoj jezgri. Test
`the_preferred_clip_reads_the_card_alone`, conformance. Uzivo nije izmjereno.
Popravak 2026-10-18 (korisnik: "pokvario si kocku kreiranja postera ako ne postoje posteri kreirani na kartici"): od
15. 10. (`2eeadaa`: artefakti idu uz dovrsetak zapisa) `tools/qnc-ingest-worker` je postere klipova bez postera s
kartice radio tek nakon svih valova i filmstripova (u izoliranoj kopiji 7 XDCAM klipova nakon 79 s, na vecoj kartici
minute), a Ingest prati postere najvise 3 min. Sada se rade odmah nakon dovrsetka zapisa, u istoj niti, prije valova i
filmstripova preostalih klipova; pod najmom radnika kao i prije. Testovi `qnc-ingest-worker`, conformance.
Dopuna 2026-10-18 (korisnik: "kreiranje postera u svoju kocku"; "sve u kockama, kojima upravlja modul, a module koriste
forme; nema prepisivanja ni dupliranja koda; podjela rada i hijerarhija"; "ingest worker (procedura) pokrece kocke"):
nova javna kocka `crates/qnc-missing-posters` (+ ugovor `contracts/modules/missing-posters.module.json`) je jedino
mjesto koje radi poster s pocetka klipa: `make_missing_posters` (klipovi aktivnog projekta bez postera, poster u
`products/thumbnails` projekta, upis kroz pisca baze pod najmom pozivatelja), `poster_from_start`, `poster_file`,
`inside_project` (jedina provjera da se pise samo u mapu projekta). Gdje je medij na ovom racunalu daje pozivatelj,
pa kocka ne ovisi o Ingestu. Iz `qnc-ingest-import-worker` uklonjeni su `make_missing_posters`, `create_missing_poster`
i vlastiti `inside_project`; poster pri Uvezi radi ista kocka. `tools/qnc-ingest-worker` (procedura) je pokrece odmah
nakon dovrsetka zapisa. Testovi `qnc-missing-posters`, `qnc-ingest-import-worker` (22), conformance.
Popravak 2026-10-18 (korisnik: "posteri se kreiraju kad ne postoje na kartici"; to i dalje ne radi): klip uvezen prije
nego mu je zapis dovrsen (XDCAM disk: zapis s kartice jos bez broja slika i brzine) pri Uvezi nije dobio poster, a
pozadinski radnik je postere radio samo za neuvezene klipove, pa takav klip nikad nije dobio poster.
`qnc-content-read` `clips_without_poster` daje svaki klip bez postera, uvezen ili ne; `tools/qnc-ingest-worker`
(`make_posters`) pokrece kocku `qnc-missing-posters` nakon dovrsetka zapisa i jos jednom nakon uvoza. Testovi,
conformance. Uzivo nije provjereno (treba Select XDCAM diska).
Izmjena 2026-10-18 (korisnik: "cim se odabere kartica (select) provjerava se imaju li postere, ako nema odmah ih
kreira i kopira"; "sve to treba biti u kockama, select pokrece kocke"): kocka `qnc-missing-posters` je jedino mjesto
koje unosi poster u projekt: poster koji kartica ima kopira se u `products/thumbnails/<klip>/poster.jpg` projekta
(kartica se samo cita, `copy_card_poster`), klip bez postera dobiva poster s pocetka klipa (`poster_from_start`), a
zapis pokazuje na kopiju u projektu. `PosterProject` (projekt na kojem radi), `start` (vlastita nit). Select
(`qnc-ingest-select`, `SelectTarget.posters`) je pokrece cim objavi klipove i ne ceka je; pozadinski radnik je pokrece
jos nakon dovrsetka zapisa i nakon uvoza (klip ciji zapis pri Selectu nije imao brzinu i broj slika).
`qnc-content-store` `SetPoster`: poster iz projekta zamjenjuje prazno mjesto ili poster s kartice, poster koji je vec u
projektu se ne zamjenjuje; ponovni Select zadrzava poster koji klip vec ima. `qnc-content-read` `clip_posters` umjesto
`clips_without_poster`. Testovi `qnc-content-store`, `qnc-missing-posters`, `qnc-ingest-select`, conformance.
Popravak 2026-10-18 (korisnik: "najprije XDCAM klipovi, njih Space pokrece; nakon njih XAVC, njih Space ne pokrece";
"stvari rjesavati na nivou kocki"): log "player input: NotFinal". Drugi Select (XAVC) dosao je dok je pozadinski proces
prvog (XDCAM) jos radio; drugi proces se nije pokrenuo (najam), a kocka `qnc-record-completion` je popis klipova za
dovrsetak procitala samo jednom, pa 98 uvezenih klipova nikad nije dobilo konacni zapis. Kocka sada nakon svakog kruga
ponovno cita bazu i radi dok ima klipova koji cekaju (klip ciji dovrsetak nije uspio ne uzima ponovno u istom radu);
`tools/qnc-ingest-worker` nakon rada ponovno pogleda bazu i radi jos jednom kad zapisi cekaju, ali ne kad cekaju isti
zapisi kao prije (dovrsetak im nije uspio). Testovi `qnc-record-completion`, conformance. U izoliranoj kopiji radnik je
pokrenut za projekt `kjhfdtz`: dovrsava zapise koji su cekali.
Popravak i smanjenje 2026-10-18 (korisnik: "zasto trebam cekati, nije ti procedura u redu, klipovi se ne kopiraju";
"stvari rjesavati na nivou kocki"; "smanji aplikaciju kroz kocke"): klik na klip ciji zapis ceka dovrsetak nije budio
pozadinskog radnika, pa se zapis nije dovrsavao, a Space je bio odbijen ("NotFinal"). Nova javna kocka
`crates/qnc-worker-wake` (+ ugovor): budi radnika aktivnog projekta osim dok mu je najam ziv (kod premjesten iz
`qnc-ingest-import-worker`, koji ga sada samo koristi). Univerzalni preview (`qnc-source-preview`: `RecordWaker`,
`wake_worker_from`) budi radnika cim zapis prikazanog klipa ceka i svakih 5 s dok ceka, a radnik taj klip dovrsi prvi;
Space dok klip ceka zapis pamti Play i pusta klip cim je zapis gotov. Ingest i Story/MA aplikacija samo predaju korijen.
Smanjeno: iz `qnc-ingest-application` uklonjen nekoristen objekt artefakata (filmstrip i val radi radnik, prikaz cita
preview), C10 1567 -> 1552; iz `qnc-editorial-application` uklonjena kopija trazenja korijena QNC-a (koristi kocku
`qnc-dev-diagnostics::locate_qnc_root`), C10 687 -> 660. Testovi, conformance.
Audit i popravak 2026-10-18 (korisnik: "kreiranje filmstripa i wavea je presporo, napravi audit"): projekt `jjf653257`
(105 klipova, 9,9 GB s kartice u USB 2.0 citacu): prolaz za val citao je karticu oko 30 MB/s po datoteci (medijan
31,7), kao da cita cijelu datoteku; 1489 (1 GB) 34 s, a 14. 10. 16,6 s. Uzrok u kocki `qnc-mxf-sound` od jednog prolaza
(16. 10.): za filmstrip je provjeravao je li slika kljucna na pocetku svake slike, dakle dva citanja po kadru umjesto
jednog, a kartica je ogranicena trajanjem citanja. Sada se pocetak slike cita samo od cilja filmstripa do prve kljucne
slike iza njega (i do 60 slika prije zadnjeg cilja, za klip koji zavrsava prije sljedece kljucne slike). Izmjereno na
kartici: 1489 sa svih 13 slika 15,0 s (67 MB/s). Filmstrip ide uz val (isti prolaz), pa se ubrzava jednako. Testovi
`qnc-mxf-sound`, `qnc-wave-worker`, conformance.
Izmjena 2026-10-18 (korisnik: "Podaci klipa se pripremaju nije prihvatljivo, play mora ici odmah"; "daj najbolje
rjesenje, za konfor rada bitna je brzina, dozvoljeno vise jezgri i niti, da korisnik moze poceti montirati"): (1)
`qnc-record-completion` uz radnike u redu ima jos jednu nit samo za klip koji preview trazi (`next_wanted`), pa njegov
zapis ne ceka iza probeova koji vec rade i ne ceka player. (2) `tools/qnc-ingest-worker`: prvo zapisi, pa valovi i
filmstrip; kartica nakon Selecta cita samo za dovrsetak zapisa (svi klipovi se mogu pustiti prije valova). (3)
`qnc-source-preview`: klip koji ceka zapis otvara se ponovno svakih 250 ms (prije 1 s), poruka je obicna priprema playera
(`player.player_getting_ready`, prije hrvatski tekst u kodu), Space se pamti i klip krene cim je spreman. Jezgra playera
i pravilo jednog probea nisu mijenjani. Testovi, conformance. Uzivo nije izmjereno.
Popravak 2026-10-18 (korisnik: SD kartica kopirana na USB stik; "uvozi i proxy i original", "iako u projektnim
postavkama pise link original", Select "10 puta sporije"): katalog kamera trazi Sonyjevu karticu kao `PRIVATE/XDROOT` u
korijenu nositelja (`card_relative`), a kopija na stiku je u `E:\SDXC\PRIVATE\XDROOT`. Kartica nije prepoznata, Select je
svaku video datoteku uzeo kao pojedinacnu datoteku: 98 proxyja postalo je zasebnih klipova (pa su i uvezeni), a svaka
datoteka dobila je probe u samom Selectu (19 s za 173 datoteke). `qnc-camera-detector` `detect_at` i `qnc-scanner`
`scan_roles_at` traze korijene kartice od zadane mape; Select (`scan.rs`) trazi od izabrane mape prema korijenu
nositelja i uzima prvu na kojoj je kartica. Provjereno na stiku (samo citanje): `E:\SDXC` daje 98 klipova s 98 proxyja,
bez ijednog problema, 169 ms. Test `a_card_copied_into_a_folder_is_found_from_that_folder`, conformance.
Popravak 2026-10-18 (korisnik: "i dalje poruka player input NotFinal"): log novog projekta: radnik se pokrenuo vec pri
otvaranju projekta (prije Selecta), dovrsetak zapisa nije nasao klipove i zavrsio je; klipove Selecta zatim je uzeo
filmstrip (citanjem kartice), a zapisi su krenuli tek ~52 s nakon Selecta, kad se radnik pokrenuo ponovno.
`tools/qnc-ingest-worker`: dok radnik radi, dovrsetak zapisa svake sekunde gleda ceka li koji zapis i odmah ga radi;
filmstrip za to vrijeme stoji (`completing`). Conformance. Uzivo nije provjereno.
Ubrzanje 2026-10-18 (korisnik: "provjeri je li taj nacin adekvatan ili postoji bolje i brze rjesenje"; "necemo
racunati USB 2.0 kao radni medij, racunajmo na USB 3 i noviji"): okidanje kroz bazu (radnik svake sekunde pita ceka li
koji zapis) ostaje, jer je baza jedina veza procesa; provjera je sada lagani upit: `qnc-content-store` javni prikaz
`public_clips` dobio je stupac `final`, `qnc-content-read` `records_waiting` (bez citanja zapisa). Probe
(`qnc-media-probe`): sazetak streamova uvijek, prvi paketi (boja slike) samo kad sazetak ne zna boju (XDCAM MXF); izmjereno
na stiku za FX6 MXF 0,53-0,70 s -> 0,26-0,28 s; i dalje jedan probe po mediju. `tools/qnc-ingest-worker` 6 istodobnih
probeova (uz nit klipa koji preview trazi); izmjereno na stiku 8 istodobno 0,22 s po klipu, 2 istodobno 0,37 s. Test
`the_wanted_clip_is_completed_first` prilagoden (vlastita nit, ne nuzno prvi zavrseni). Testovi, conformance.
Izvedeno 2026-10-18 (korisnik: "svakim korakom kvaris jedno dok popravljas drugo"; "da, kreni s 1 i 2" na prijedlog
rastavljanja na kocke): (1) tipke po panelu. Prije rastavljanja zlatni testovi zakljucavaju odgovor svake tipke kataloga
na svakom panelu: `crates/qnc-panel-focus/tests/route_matrix.*` (Story/MA, 830 slucajeva) i
`crates/qnc-focus-walk/tests/walk_matrix.*` (Ingest, 588), zapisani iz koda prije izmjene i nepromijenjeni poslije. Nove
javne kocke (s ugovorima modula): `qnc-key-row` (red dugmadi/tabova: strelice biraju, krajevi drze, klik bira) i
`qnc-key-list` (tipke popisa kao numerirani koraci, Enter koji otvara karticu) koje sada koriste i Story
(`qnc-panel-focus`) i Ingest (`qnc-focus-walk`) umjesto dviju kopija; `qnc-segments-keys` (tipke panela Segmenti: zvuk
segmenta/pokrivalice na Wrap retku, Enter, strelice, Ctrl+I/O rub slota, Ctrl+Shift+I/O rub segmenta, I/O bez ucinka).
(2) `qnc-sync-session`: sve tipke Sync/B-roll na jednom mjestu (Space start/nastavak ili play/pauza, strelice
zaustavljenog Synca, Enter, Delete, O, I naoruzava); stanje i upisi Synca ostaju u `qnc-program-segments`.
`qnc-panel-focus` je modul koji drzi samo redoslijed kocki (prva koja uzme tipku odgovara) i krug panela; 827 -> 745
redaka (prava funkcija preusmjeravanja i dalje oko 240 redaka: tipke source timelinea vezane uz rub segmenta).
Ponasanje se ne mijenja. Testovi, conformance, release gradnja. Uzivo nije provjereno.
Popravak 2026-10-18 (korisnik: "promjena audio kanala nije moguca misem, samo strelicama gore-dolje"; "timeline mora biti
zajednicki za sve, jednom pisi koristi svugdje"; "wrap-segment je isti timeline samo ima dodan jos jedan lajer,
pokrivalica... tako je zamisljen jos od verzije 5"): na Wrap retku klik na kanal u stupcu uzetog A1/A2 kocka
`qnc-timeline` je javljala (`ChooseA1Channel`/`ChooseA2Channel`), ali ga panel Segmenti nije primao. Sada je izbor kanala
klikom jedan kljuc traka svih timelinea: `qnc-timeline` `LaneKey::Pick(kanal)` (isti put koriste source timeline i Wrap
redovi; source timeline vise nema svoj `set_draft`), `qnc-program-commands` `SegmentCommand::PickChannel`,
`qnc-segment-panel` ga daje, `qnc-panel-focus` `segment_command` ga predaje kocki `qnc-segments-keys` (`pick`: nacrt se
odmah cuje, kao strelice). Testovi, conformance, zlatni testovi. Uzivo nije provjereno.
