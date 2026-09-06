# QNC - root pravila

Ovaj file je globalni pravilnik za novu QNC obitelj aplikacija.

Root projekta: `C:\Users\miron\Projects\QNC`  
Stari referentni projekt: `C:\Users\miron\Projects\qnc_v4`

Ako pojedina aplikacija kasnije dobije svoj `AGENTS.override.md`, taj override
smije dodati stroza lokalna pravila, ali ne smije oslabiti ova root pravila.

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
  Project komponente, Project store ili Project workflow. Ako trebaju raditi po
  postavkama projekta, smiju citati samo dogovorene postavke za rad iz baze
  aktivnog projekta.
- Oznaka aktivnog projekta mora biti podatak u bazi, ne UI stanje i ne argument
  koji aplikacija izmisli. Ingest cita bazu, pronalazi koja projektna baza ima
  oznaku aktivnog projekta i iz nje cita samo postavke za rad.
- QNC baza mora biti prenosivi poslovni artefakt. Na bilo kojem racunalu kojem
  korisnik da valjanu QNC bazu, Ingest mora moci raditi bez Project aplikacije,
  bez QNC.app shella i bez bilo kojeg drugog QNC aplikacijskog procesa.
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
- Windows adapter koristi ACL deny delete/delete-child za trenutnog korisnika.
- macOS adapter koristi file flags gdje je dostupno.
- Linux/POSIX adapter mora zakljucati parent `projects_root`, jer POSIX delete
  direktorija kontrolira write permission na parent direktoriju.
- Hidden je samo dodatna zastita od slucajnog korisnickog diranja, nije glavna
  lock zastita.
- Samo Project aplikacija smije privremeno otkljucati projektni direktorij, i
  to samo za vlastito pisanje ili za potvrdeno brisanje iz Project aplikacije.
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

## 5. Ingest kao prvi konkretni primjer

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

- Filmstrip je modul za generiranje artefakta, ne aplikacija, ne scanner i ne
  probe.
- Filmstrip cita samo podatke iz baze.
- Filmstrip koristi proxy ako postoji, a original ako proxy ne postoji.
- Filmstrip mora generirati stvarne frameove, ne ponavljati poster.
- Filmstrip ima 14 slicica.
- Za clipove do 10 sekundi koristi se brza stara strategija iz QNC ingest
  ponasanja.
- Za clipove duze od 10 sekundi trajanje se dijeli na 14 pozicija.
- Filmstrip ne smije raditi `ffprobe` niti drugi probe fallback.

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
- Ingest trenutni runtime rez je samo source browser + `source.select` +
  registry/session DB zapis. `Odaberi` jos nije puni Ingest dok ne pokrene
  source scan, original/proxy grouping, jedini probe prolaz i clip/probe DB
  upis kroz odobrene module.
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
