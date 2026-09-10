# QNC - root pravila

Ovaj file je globalni pravilnik za novu QNC obitelj aplikacija.

Root projekta: `C:\Users\miron\Projects\QNC`  
Stari referentni projekt: `C:\Users\miron\Projects\qnc_v4`

Ako pojedina aplikacija kasnije dobije svoj `AGENTS.override.md`, taj override
smije dodati stroza lokalna pravila, ali ne smije oslabiti ova root pravila.

**Temeljna odrednica razvoja: sve QNC poslovne aplikacije rade u okviru
projekta i moraju poznavati njegove postavke kroz bazu. Projects ih zapisuje
u bazu svakog projekta; sve ostale aplikacije ih citaju i primjenjuju.**
To ukljucuje odredista direktorija i baza te pravila rada i pohrane potrebna
pojedinoj aplikaciji. Pravilo vrijedi za sve postojece i buduce aplikacije,
ne samo za Ingest.
Jedina poslovna veza je DB zapis, ne Projects aplikacija, shell ni zajednicko
runtime stanje. Citanje je read-only kroz javni DB/transport ugovor, jednako
za Local/LAN/Intranet. Potrosaci ne uvode vlastite zamjenske projektne postavke.
Prije zahvata u bilo kojoj aplikaciji obvezna je provjera iz odjeljka 4.1.

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
  projektnu bazu zapisuje postavke projekta.
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
- Projects je vlasnik zapisa projektnih postavki. Ostale aplikacije citaju
  potrebne postavke read-only kroz javni DB/transport ugovor; ne prepisuju ih,
  ne nasljedjuju ih od druge aplikacije i ne zamjenjuju ih lokalnim defaultima.
  Poslovne rezultate i dalje zapisuju samo u vlastitu bazu ili shemu.
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
- Projects (Project aplikacija) zapisuje lokaciju konkretnog projekta i njegove
  radne postavke te kreira standardni projektni raspored. Ingest, Filmstrip,
  Wave i drugi potrosaci te odluke NE MIJENJAJU.
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
- Filmstrip slicice spremaju se kao JPEG datoteke na odrediste koje je zadao
  Projects (v4 referenca: `filmstrip/<clip_id>/`); baza cuva vezu s klipom,
  redoslijed, vremenske polozaje, status i reference na artefakte. Ne zamjenjivati
  ovaj raspored JPEG BLOB-ovima u novoj `filmstrip.db` bez izricitog odobrenja.
- Wave se sprema kao niz amplituda/peaks po kanalu u bazu koju je zadao Projects.
  Ne zahtijeva posebnu wave mapu, PNG datoteke niti novu `wave.db` samo zato sto
  je generator zaseban modul. U v4 su to `a1_peaks` i `a2_peaks` u tablici
  `audio_waveforms`.
- Referentni v4 `qnc_project.db` sadrzi tablice `filmstrips`,
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
- Wave prikaz je takodjer pasivan: crta vec pripremljene amplitude. Ni jedan
  prikaz ne generira artefakte, ne radi probe, ne odredjuje pohranu i ne pise DB.
- Generator filmstripa i generator wavea odvojeni su javni moduli; pohrana ide
  kroz javnu DB/storage komponentu. Prikaz, generator i pohrana nisu jedna
  komponenta niti poslovna logika forme.
- Sljedeca pravila odnose se na GENERIRANJE filmstripa. Generator je modul za
  izradu artefakta, ne aplikacija, ne scanner i ne probe.
- Filmstrip cita samo podatke iz baze.
- Filmstrip koristi proxy ako postoji, a original ako proxy ne postoji.
- Filmstrip mora generirati stvarne frameove, ne ponavljati poster.
- Filmstrip ima 14 slicica.
- Za clipove do 10 sekundi koristi se brza stara strategija iz QNC ingest
  ponasanja.
- Za clipove duze od 10 sekundi trajanje se dijeli na 14 pozicija.
- Filmstrip ne smije raditi `ffprobe` niti drugi probe fallback.
- Ponavljanje postera u starom Ingest UI-ju nije generirani filmstrip i ne moze
  zamijeniti zahtjev za 14 stvarnih frameova. Pasivni UI obrazac i stvarni
  generirani sadrzaj moraju se promatrati odvojeno.

### 8.1. Timeline je pasivni prikaz i UI remote

- Timeline je samostalna javna pasivna UI komponenta. Prikazuje stanje
  Broadcast Playera i sluzi kao njegov UI daljinski upravljac; nije player,
  playback engine, vlasnik vremena niti vlasnik prikazanog stanja.
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
- Zabranjeno je uvoditi fiksni `60 Hz`, `16 ms`, monitor refresh ili OS repaint
  kao playback pravilo. Player cadence mora dolaziti iz spremljenog source
  timebasea konkretnog klipa i player/audio clocka. Podrska za source koji je
  stvarno 60 fps je dopustena samo kao source timebase, ne kao globalna
  pretpostavka aplikacije.
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
  izjava, A2 ambijent B-rolla. Te uloge nisu stereo par kamere.
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
   odvojeni. Broadcast A1/A2 nisu stereo fallback. Svaki nepodrzani format
   mora dati jasnu gresku ili stvarnu dogovorenu konverziju u javnom adapteru.
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
  izdvojiti kao javni modul ili javnu komponentu, npr. `qnc-ui-kit`,
  `qnc-dir-browser`, keyboard, resolver, media browser ili timeline paint.
- Javni UI modul, ukljucujuci `qnc-ui-kit`, smije sadrzavati samo pasivne
  paint/layout/intent obrasce i genericke UI-state pomocnike, npr.
  ekskluzivno otvaranje panela. Ne smije imati aplikacijski workflow, DB ownera,
  filesystem ownership, browser session ownership, media scan/probe/player/
  render/export logiku niti centralni registry poslovnog stanja.
- Javni modul ili komponenta ne smije se siriti dodavanjem posebnih pravila za
  Project, Ingest, Story ili Media Assist. Ako mu treba takvo znanje, modul je
  postao monolit i mora se razbiti na uzi contract.

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

Zapisano 2026-09-04, azurirano 2026-09-05. Vrijedi samo dok se odstupanja ne
zatvore.

- Shell runtime footer prikazuje samo aplikacije s `apps/*/qnc-app.json`.
  Layout contract i dalje nabraja `project`, `ingest`, `media_assist` i
  `storyboard`.
- Close project nije u footeru dok ne postoji workspace close contract.
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
  i source/media DB upis kroz javne module (docs/34). Postojeci zavrseni zapisi
  citaju se bez novog probea. Prikaz karticnih slicica koristi eksplicitne DB
  veze i read-only transport (docs/35). To ne znaci da su kopiranje medija,
  filmstrip, waveform, playback ili sve camera sheme implementirani.
- Ingest application manifest smije deklarirati samo module i capabilityje koji
  imaju stvarnu runtime ovisnost ili implementirani javni adapter u trenutnom
  rezu. Scanner, camera detector, Media Probe, Media Browser, Filmstrip i Wave
  ne smiju biti u Ingest runtime manifestu dok ne postoje kao stvarni moduli u
  runtimeu.
- Project keyboard dispatch smije krenuti od `project_open_selected`. Nove tipke
  ne smiju ici mimo kataloga.
- Ingest klik akcije moraju imati action_id u
  `contracts/qnc-keyboard-shortcuts.json` prije nego ih forma posalje. Tipke za
  Ingest smiju se dodati samo kroz taj katalog.
- Soft/HighContrast theme varijante smiju ostati samo dok se ne prebace u UI
  contract.

Sve ostalo iz ovog filea ostaje na snazi. Ova lista nije dozvola za redizajn
ili novi monolit.
