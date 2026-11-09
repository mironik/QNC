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
11. **Forma je samo layout: mreza mjesta na kojima se prikazuju paneli; paneli sadrze module i kocke.**
    (Korisnik 2026-10-20.) Hijerarhija je kocka -> modul -> panel -> forma. Kocka ima jednu
    odgovornost i napisana je jednom; modul slaze kocke; panel je najmanja jedinica koju layout
    smjesta i jednak je u svakoj formi; forma (ploca, desktop i aplikacijski sloj iste aplikacije,
    npr. Ingest, Story, Media Assist, Project) samo kaze koji panel ide na koje mjesto mreze i daje
    mu svoje mjere, tekstove i stanje.
    - Forma i njezin aplikacijski sloj NE RASTU. Svaka nova mogucnost, i svaki prijevod podataka
      za panel, ide u postojecu ili novu kocku/modul, nikad u formu ni u aplikacijski sloj. Izvlacenje
      logike iz forme ne smije je preseliti u aplikacijski sloj: ako mapiranje podataka za panel
      treba svoj tip, taj tip zivi u kocki, ne u formi ni u aplikaciji.
    - Ne pise se kod koji vec postoji u kocki; ne kopira se izmedu formi. Dvije forme s istim
      crtanjem ili istom logikom su greska koja se rjesava kockom.
    - Granice velicine formi i aplikacijskih slojeva (conformance C10) samo padaju; povecanje se
      ne nadoknaduje premjestanjem drugog koda, nego se novi kod stavlja u kocku.
12. **Timeline i zoom imaju iste alate; razlika je samo graficki prikaz.** (Korisnik 2026-11-04,
    ZAKLJUCANO.) Zoom je uvecani isjecak timelinea oko playheada, nije zaseban alat. Svaki
    timeline (source, Wrap redovi Storyja i S-Storyja, slozena izjava, program) i njegov zoom
    rade istim radnjama: isti klik, desni klik (Ctrl+), dvoklik, povlacenje, kotacic, tipke,
    J/L, IN/OUT, M marker, slot, rijeci prijepisa i iste sistemske radnje kataloga. Alat se
    pise jednom, u zajednickoj kocki, i oba prikaza ga zovu; zoom ne smije imati vlastitu
    radnju koju timeline nema, ni timeline radnju koju zoom nema. Vlasnik prikaza daje samo
    os (koje slike prozor pokazuje), ne vlastita pravila. Ovo pravilo je zamrznuto: promjena
    trazi izricitu korisnikovu odluku zapisanu ovdje.

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
Izvedeno 2026-10-18 (korisnik: "sve radnje pod sistemskom sifrom, kako bi se mogle koristiti razne shortcut mape i vise
ulaznih uredaja: tipkovnica, mis, touchpad, programabilna tipkovnica, edit konzola, geste... to trebaju biti kocke"; "da,
kreni s a i b"): (a) nova javna kocka `qnc-action-event` (+ ugovor): jedna sistemska radnja s bilo kojeg uredaja
(`action_id` iz kataloga, neobavezna vrijednost: slika, kanal; neobavezan cilj: marker, slot, pokrivalica; panel na kojem
je bio pokazivac, tipka ga nema). Mapa misa svih timelinea je jedna funkcija u `qnc-timeline` (`event`): klik je ista
radnja kao njegova tipka gdje postoji (desni klik je Ctrl+: na markeru `select_marker`, u slotu
`select_current_marker_slot`, uz rub slota `select_mark_in/out`, na A1/A2 `select_audio_a1/a2`; kotacic su strelice),
pokazivac dodaje cilj i sliku. Nove radnje kataloga bez tipke: `timeline_cue_frame`, `timeline_select_marker`,
`timeline_select_slot`, `timeline_select_cover`, `timeline_take_cover`, `timeline_drag_marker`. Source timeline
(`event_of`, umjesto `action_of`), panel Segmenti (Wrap reci daju `SegmentInput::Event`, gumbi trake i popisa ostaju
naredbe) i Ingest koriste istu mapu. (b) `qnc-panel-focus::route` prima sistemsku radnju: tipka ide panelu u fokusu,
pokazivac panelu na kojem je bio (klik u panel stavlja fokus na njega; desni klik na source timeline dok je fokus na
Segmentima vise ne uzima rub slota); radnja koja imenuje objekt ili sliku na Wrap retku ide kroz jedno preslikavanje
`qnc-program-commands::SegmentCommand::from_event`; `EditorialIntent::Timeline` i `segment_command`/`wheel_action`
uklonjeni. Zlatni testovi tipki nepromijenjeni; novi testovi za mapu misa i za panel pokazivaca. C10 Editorial
application 660 -> 654, ploca 349 -> 348. Nije izvedeno (c): kocke za touchpad/geste i vanjske konzole; gumbi trake i
popisa panela Segmenti jos su naredbe, ne sistemske radnje. Uzivo nije provjereno.
Izvedeno 2026-10-18 (korisnik: "prebaci i gumbe panela Segmenti na sistemske radnje"): svi gumbi trake i popisa panela
Segmenti salju sistemsku radnju na tom panelu (`qnc-action-event`), panel daje samo sistemske radnje
(`EditorialIntent::Segment` i `SegmentInput` uklonjeni). Gumb koji ima tipku salje istu sifru kao tipka (M `add_marker`,
Cover slot `quick_overwrite_cover`, Overwrite `overwrite_cover`, Undo/Redo `undo_object`/`redo_object`, pocetak programa
`playlist_input_start`) i ide istim putem kao tipka. Gumbi bez tipke dobili su sifre u katalogu s ciljem (segment):
`segment_select`, `segment_exclude`, `segment_include`, `segment_purge`, `segment_replace`, `segment_edit`,
`segment_move_up`, `segment_move_down`, te `program_prev_marker`/`program_next_marker`, a skrivena navigacija koristi
postojece `step_prev/next_segment` i `step_prev/next_marker_slot`; preslikavanje u naredbu je ono isto
`SegmentCommand::from_event`. Razlika: gumbi M, Cover slot i Overwrite rade tocno kao njihove tipke (npr. dok je slot uzet).
Zlatni testovi nepromijenjeni, novi test za gumbe. C10 Editorial application 654 -> 649. Uzivo nije provjereno.
Izvedeno 2026-10-18 (korisnik: "kreni s c, touchpad i geste kao kocke"): svaki ulazni uredaj je kocka sa svojom mapom u
ugovoru. Nalaz: na Windowsima touchpad ne salje vlastite geste: pomak s dva prsta stize kao kotacic u djelicima zareza,
pinch kao Ctrl uz njih (winit samo na macOS-u daje pinch kao zasebnu gestu); `qnc-wheel-jog` je svaki djelic brojao kao
cijeli korak. Nove javne kocke (s ugovorima): `qnc-gesture-steps` (uloge geste `jog` = kotacic panela, `level` =
Ctrl+kotacic, `none`; fini pomak zbrojen u cijele korake, korak po slici; citanje mape uredaja, nevaljana mapa = uredaj ne
radi nista, zapis u logu), `qnc-touchpad` (mapa `contracts/input/touchpad.json`: vodoravno i okomito s dva prsta `jog`,
pinch `level`; Windows djelici zareza i Ctrl, macOS tocke i zoom) i `qnc-touch-gestures` (zaslon na dodir, mapa
`contracts/input/touch.json`: povlacenje dva prsta vodoravno `jog`, okomito `none`, pinch `level`; jedan prst ostaje
pokazivac). `qnc-wheel-jog` broji samo cijele zareze misa (`is_notch`). Svaki timeline poziva sve tri kocke, pa sve
daju iste sistemske radnje kao kotacic. Promjena ponasanja: pomak touchpadom na Windowsima je sporiji (korak po cijelom
zarezu umjesto po svakom djelicu). Pocetne vrijednosti mapa (uloge, koraci) su prijedlog, mijenjaju se u ugovoru. Testovi,
conformance. Uzivo nije provjereno (touchpad i zaslon na dodir).
Izvedeno 2026-10-18 (korisnik: "za geste i misa trebam cut (skare) za odredivanje reza"; "na mjestu playheada napravim
cut rez, a razmak izmedu dva cut reza odreduju IN i OUT na source timelineu"; "nije bitno koji je prvi a koji drugi,
razmak je bitan"; "tipkovnicom postoji I ili O, trazim rjesenje za mis i pad"): skare na source timelineu (sve forme).
Rez je uvijek na mjestu playheada (i dok svira): prvi rez je IN bez OUT-a, drugi oznacava razmak (raniji IN, kasniji
OUT, bez obzira na redoslijed), treci pocinje novi par; par pripada klipu. Mis: srednja tipka (kotacic) iznad source
timelinea; touchpad: dodir s tri prsta kad je u Windows postavkama srednji klik; zaslon na dodir: kratki dodir s dva
prsta na mjestu (`contracts/input/touch.json` `two_finger_tap`: `cut` ili `none`). Radnja kataloga `source_cut` (bez
tipke; tipke ostaju I i O). `qnc-timeline` (`TimelineIntent::Cut`, mapa misa `source_cut`,
`with_first_cut_at_confirmed`, `with_second_cut_at_confirmed`), `qnc-source-timeline` (`cut`, pamti prvi rez,
`route_input`, pa isto u Storyju i Ingestu), `qnc-touch-gestures` (`tapped`), `qnc-panel-focus` (rez naoruzava
Sync/B-roll kao I). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-18 (korisnik: "srednji klik ne radi"; "ta srednja tipka misa treba raditi i na pokrivalicama"): log
nije imao nijedan `source_cut`: srednja tipka je ujedno pritisak na traku, a namjera tog pritiska (cue) imala je
prednost pred rezom. Sada je rez (srednja tipka ili dodir s dva prsta) uvijek ono sto znaci, na svakom timelineu
(`qnc-timeline`). Na Wrap retku (pokrivalice) rez je M marker na Wrap playheadu, kao tipka M (`add_marker`): dva reza
omeduju slot za pokrivalicu (`qnc-segment-panel`). Uzivo nije provjereno.
Popravak 2026-10-18 (korisnik: "sad ne rade shortcut ni gumbi za kreiranje segmenta"): tipka i gumb su stizali do price,
ali je baza odbijala novi segment ("pokrivalica bi zavrsila prije svog M markera"): novi segment pomice zavrsni marker,
pa je zadnji slot s kratkom pokrivalicom rastao i pravilo "bez rupe" (17. 10.) odbijalo je izmjenu. `qnc-program-db`
`create_segment`: kad zadnji slot ima pokrivalicu, njegov zavrsni marker ostaje na mjestu kao obican M marker, a novi
zavrsni ide iza novog segmenta, koji dobiva prazan slot; bez pokrivalice kraj se pomice kao u v5. Poruka odbijanja nije
se vidjela jer je statusna traka prvo pokazivala poruku playera; sada poruka price ima prednost
(`qnc-editorial-application` `footer_status`). Proba na kopiji baze projekta: segment upisan, pokrivalica u svom slotu
(671-777), novi prazan slot (777-1227). Test `a_new_segment_keeps_the_last_slot_of_a_cover_and_gets_an_empty_slot`.
Popravak 2026-10-18 (korisnik: "desna tipka misa (Ctrl+) trebala bi mi omoguciti pomicanje tog markera"): marker na rubu
Wrap reda (kraj treceg segmenta, 777) misem se nije dao uzeti: pogodak markera bio je 5 piksela od crte, a klik jednu
sliku lijevo (oko 9 piksela) bio je samo pomak playheada. `qnc-timeline` `marker_hit`: 8 piksela, uzima se najblizi
marker. Desni klik na marker uzima ga (kao Ctrl+M), kotacic i strelice ga pomicu, lijevi klik ili Enter spremaju.
Popravak 2026-10-18 (korisnik: "ne radi dugme B-roll, ni Shift+B"): novi projekt ima jedan slot preko cijele price
(0-3695), a B-roll od 222 slike pokrio bi samo njegov pocetak; pravilo "bez rupe" odbijalo je pokrivalicu (u bazi je nije
bilo, iako je status rekao "saved"). `qnc-program-db` `create_cover` (`split_slot_for`): B-roll kraci od slota postavlja
u istoj transakciji M marker na svom kraju (kao Sync), pokrivalica pokriva prvi dio, ostatak je prazan slot; jedan Undo
korak. Testovi `a_b_roll_shorter_than_its_slot_splits_it_with_an_m_marker`; dva starija testa koja su kratkom
pokrivalicom izazivala odbijanje sada pokrivalicom pune duljine, odnosno OUT-om iza kraja klipa.
Dopuna 2026-10-18 (korisnik: "B-roll treba staviti playhead na svoj kraj, kako bi se sljedeci B-roll mogao nastaviti...
bitno je maksimalno brzo pokriti offove, a prethodno imamo napravljene SubClipove"): kad baza potvrdi pokrivalicu
(Cover slot/B, dugme B-roll, Overwrite), Wrap playhead ide na njezin kraj (pocetak slota + duljina izvora u slikama
price, najvise kraj slota), pa sljedeci B puni sljedeci prazni dio. Odbijena pokrivalica ne mice playhead.
`qnc-program-segments` (`cover_end`, `cover_end_of`). Test `after_a_b_roll_the_wrap_playhead_is_at_its_end`.
Dopuna 2026-10-18 (korisnik: "markeri za povlacenje trebali bi biti virtualno siri kako korisnik ne bi morao precizno
dolaziti misem, ako je prethodno postavljen desnom tipkom (Ctrl+)"; "kad korisnik dode misem u tu zonu osjetljivosti da
oznaka promijeni boju u crvenu"; "sve to u kockama"): u kocki `qnc-timeline` zasebna podkocka `marker_reach.rs` (jedno
pravilo za klik, povlacenje i crtanje): marker se uzima 8 piksela od crte, uzeti ili izabrani marker
(`TimelineMarkerPin.taken`) povlacenjem i do 40 piksela; marker u dosegu misa crta se crveno i deblje
(`TimelineTheme.marker_hover`). `qnc-segment-panel` predaje `taken` (izabrani marker). Uzivo nije provjereno.
Dopuna 2026-10-18 (korisnik: "kada radim Sync/B-roll ne vidi se da se pokrivalica pise... linija koja mijenja boju
ovisno koliko traje pokrivalica"): pokrivalica koju Sync pise crta se na Wrap retku dok raste (od markera do trenutne
slike, i kad je zaustavljena i strelice joj pomicu kraj): boja po trajanju (iste granice kao spremljene pokrivalice,
`cover_lengths`), trajanje u brojkama, okvir boje fokusa dok nije spremljena (Enter) ili odbacena (Delete).
`qnc-sync-cover` `draft_span`, `qnc-program-view` `SegmentsView.sync_draft`, `qnc-program-segments` (`sync_frame`),
`qnc-segment-panel` (nepotvrdjena pokrivalica), `qnc-timeline` (nepotvrdjena pokrivalica u svojoj boji, s oznakom i
okvirom). Test u `qnc-program-segments`. Uzivo nije provjereno.
Popravak 2026-10-18 (korisnik: "povlacenje markera i dalje ne radi... povlacenje mora raditi u svim timeline"): log je
pokazao da je povlacenje IN-a na source timelineu radilo, a na Wrap retku pritisak oko 66 slika od markera (1061 prema
1127, marker nije bio uzet) bio je scrub playheada: doseg neuzetog markera bio je 8 piksela. `qnc-timeline`
`marker_reach.rs`: svaki marker 16 piksela (klik i povlacenje), uzeti 40; svaki pocetak povlacenja na redu s markerima
zapisuje u player log pogodak i najblizi marker s udaljenoscu u pikselima (`timeline-drag start`). Uzivo nije provjereno.
Dopuna 2026-10-18 (korisnik: "zona prihvata je preuska"; log: povlacenje radi, marker uhvacen 1-12 piksela od crte):
doseg markera 24 piksela, uzetog 60 (`qnc-timeline` `marker_reach.rs`).
Izmjena 2026-10-18 (korisnik: "hvatanje markera i zona osjetljivosti mogu biti isti... korisnik ne mora traziti marker,
cim korisnik ude u tu zonu marker postaje crven i korisnik ga moze povlaciti"): jedna zona za sve markere i sve radnje:
40 piksela od crte, najblizi marker; ista zona crveni marker pod misem, uzima ga klikom, desnim klikom i povlacenjem
(nema posebne zone za uzeti marker). `qnc-timeline` `marker_reach.rs` (`MARKER_REACH`, `marker_in_reach`),
`TimelineMarkerPin.taken` uklonjen.
Dopuna 2026-10-18 (korisnik: "to treba biti i na source timeline"): IN i OUT na source timelineu (sve forme) imaju istu
zonu kao M markeri: 40 piksela, bliza oznaka, crvena cim je mis u zoni, povlacenje iz cijele zone (prije 6 piksela).
`qnc-timeline` `marker_reach.rs` `source_mark_in_reach`, test.
Izvedeno 2026-10-18 (korisnik: "mozemo li jos ubrzati filmstrip i wave, s vise niti i jezgri"; "da"; "broj jezgri i niti
odrediti kratkim testom kako bi bilo prilagodljivo brzini citaca"): izmjereno na USB 3 stiku (svaka skupina druge
datoteke, bez cachea): jedan prolaz zvuka i slika filmstripa 1 istodobno 62 MB/s datoteke, 2 istodobno 94, 4 istodobno
158. Nova javna kocka `crates/qnc-read-pace` (+ ugovor `contracts/modules/read-pace.module.json`; samo odlucuje):
prvi prolazi su test, bez dodatnog citanja; razina se mjeri radom (sekunde medija) zavrsenim u sekundi dok su svi
citaci zauzeti; jos jedan citac ostaje dok donosi barem 10 % vise, zatim run ostaje na najboljoj razini; mjerenje
krece iznova kad citaci ustupe (player, otvoreni klip cita sam) ili nemaju sto citati. `qnc-wave-worker`: pocinje s 2,
najvise 8 citaca, odluke u `wave.log` (`qnc-wave-worker-pace`); `qnc-filmstrip-worker`: dekodiranja slika istodobno pola
niti procesora, 2 do 4. Testovi, conformance. Uzivo nije izmjereno.
Popravak 2026-10-18 (korisnik: "pogledaj log"): dva nova projekta na stiku: test se jednom zaustavio na 3, jednom na 2
citaca (83,6 s i 94,6 s za 98 klipova), jer je razinu sudio po radu zavrsenom u prozoru od 3 s s 4-6 prolaza, pa su
odlucivali dugi klipovi (2679 do 30 s, 2002 27 s) i prolazi zapoceti prije promjene razine. `qnc-read-pace`: svaki
prolaz daje svoju brzinu (sekunde medija kroz svoje trajanje), razina je medijan brzina prolaza zapocetih na njoj puta
broj citaca, sudi se nakon najmanje 6 takvih prolaza (2 po citacu); prolaz zapocet prije prekida ili promjene razine ne
broji se (`Round`). `qnc-wave-worker` pamti krug svakog prolaza. Testovi (dugi spori prolaz ne odlucuje razinu),
conformance. Uzivo nije izmjereno.
Izmjena 2026-10-18 (korisnik: "ne djeluje mi brze... moras imati svoj kratki test brzine citanja medija i na osnovu
njega odrediti broj jezgri i niti; nakon toga ne testiras vise"; log: novi projekt opet na 2 citaca, 89,7 s): mjerenje iz
prolaza je zamijenjeno jednim kratkim testom medija. `qnc-read-pace` `measure`: blokovi od 64 KB svakih 1 MB kroz jednu
datoteku (kao sto prolaz cita zvuk i preskace sliku) s 1, 2, 4 i 8 citaca po 0,3 s, svaka razina u svom dijelu
datoteke; bira najmanje citaca unutar 5 % najboljeg; samo citanje, na Windowsima mimo cachea sustava
(`FILE_FLAG_NO_BUFFERING`; bez toga je stik procitan ranije davao brzinu memorije). `qnc-wave-worker`: dok medij nije
testiran radi jedan prolaz, koji ga prvi testira (datoteka manja od 64 MB ostavlja test sljedecem), rezultat vrijedi do
kraja rada (najvise 8, nikad vise od niti procesora), zapis `qnc-wave-worker-pace: readers=.. test=..`. Izmjereno na
USB 3 stiku mimo cachea: 1 citac 34-56 MB/s, 2 citaca 53-59, 4 i 8 isto ili manje: stik je zasicen s 2 citaca (oko 850
citanja u sekundi), pa su ~90 s za 98 klipova granica stika, ne jezgri. Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-10-18 (korisnik: "do 10 sekundi ubrzano citanje je bolje, nakon toga duljina fajla se dijeli na 13 dijelova i
gadaju se najblize intra slicice"; "taj kod smo imali... u QNC"): vracen prag iz QNC-a uklonjen 30. 9. (`afc4607`):
`qnc-filmstrip` klip do 10 s `RandomSeek`, dulji `KeyframeSeek` (13 jednakih dijelova, najbliza intra slika).
Redoslijed (zamjenjuje 15. i 16. 10.: filmstrip iz prolaza vala): zapisi, pa filmstrip koji sam cita svojih 13 slika,
pa valovi; val otvorenog klipa ne ceka. `qnc-import-control` (`Order::FilmstripRuns`, `Order::WavesRun`),
`qnc-wave-worker` (`set_held`; prolaz cita samo zvuk, slike filmstripa i `take_finished` uklonjeni),
`qnc-timeline-artifacts`. Probe u izoliranoj kopiji: `probe_size_bytes` 2 MB umjesto 8 MB (postavka racunala;
izlaz ffprobea isti za XAVC, MP4 proxy i XDCAM, s 512 KB nije); stvarni `data` nije diran. Testovi, conformance.
Uzivo nije izmjereno.
Izvedeno 2026-10-18/19 (korisnik "da": MXF indeks za filmstrip i val u dva prolaza; "valove mozes citati u vise prolaza:
prvo grubo citanje samo intra slike za sve klipove, nakon toga sve slike"): mjereno, filmstrip kroz ffmpeg trajao je
2,3-2,9 s po klipu i kad su datoteke u memoriji (13 ulaza, svaki otvara MXF), 93 s za 98 klipova. Nova javna kocka
`crates/qnc-mxf-index` (+ ugovor `contracts/modules/mxf-index.module.json`; samo citanje): indeks koji original nosi
(XAVC kamere: puni indeks iza zaglavlja, jedna particija s esencijom), polozaj paketa svake slike, koje su intra
(SMPTE 377 zastavice), elementi slike i zvuka jednog paketa; bez upotrebljivog indeksa `None` i sve ide kao prije.
Provjereno na stiku: 13 intra slika klipa od 11178 slika za 23 ms, sve se dekodiraju. `qnc-filmstrip-worker`: klip
dulji od 10 s uzima intra sliku najblizu svakoj od 13 pozicija kroz indeks i dekodira ih jednim pozivom (do 10 s ostaje
ubrzano citanje); `qnc-filmstrip` `original_picture_codec`. Val u dva prolaza: klip bez vala najprije dobiva grubi val
(zvuk samo intra slika kroz indeks, svaki zadrzan do sljedece; oko jedno citanje na 24 slike), zatim val svake slike
tiho ga zamjenjuje; otvoreni klip dobiva puni val prvi. Zapis vala `qnc-wave` `WaveArtifactRecord.coarse` (u istom
JSON-u, bez promjene sheme; puni zapis bez polja), `WaveformPeaks.coarse`; `qnc-wave-worker` (grubi zapis nije
"ready", klip se vraca u red), `qnc-source-preview` i `qnc-program-waveform` citaju grubi val ponovno dok ga puni ne
zamijeni. Testovi, conformance. Uzivo nije izmjereno.
Popravak 2026-10-19 (korisnik: "novi disk ne ucitava s njega, rutina ima problem"): disk D: je FX6 CFexpress kartica
(`XDROOT` u korijenu, 122 klipa s proxyjima); katalog kamera poznavao je Sony XAVC samo kao `PRIVATE/XDROOT` (SD), pa
Select nije nasao nijedan original i nista nije javio. Nova revizija kataloga `catalogs/camera-patterns/revisions/
2026.10.19.1.json` -> `camera-patterns-2026.10.19.1.sqlite` (alat `qnc-camera-catalog revise`; stara objava ostaje):
`sony-xdroot-sd` dobiva korijen `XDROOT` uz `PRIVATE/XDROOT`, datoteke, uloge, grupiranje i probe isti. Provjereno citanjem
diska: 122 originala, 122 proxyja, 122 XML zapisa s tocnim brojem slika; stari katalog 0. `sony-xdcam-mxf` (Clip u
korijenu) se ne preklapa. Izolirana kopija koristi novi katalog; stvarni `data/ingest-transport.json` jos pokazuje staru
objavu (postavka racunala, nije dirana). Otvoreno: Select koji ne nade ni karticu ni datoteke ne javlja razlog.
Popravak i mjerenje 2026-10-19 (korisnik: "pogledaj log i izmjeri brzinu, ovo je citac"; CFexpress citac, 122 klipa
originala, proxy nebitan): (1) grubi val je provjeravan kao da mora biti potpun, pa je svaki odbijen ("The wave result is
not ready") i nijedan klip nije dobio val; `qnc-wave-worker` `wave_artifact_valid` (grubi ili puni) i `wave_artifact_ready`
(samo puni). (2) Test medija: uredaj koji spava prvim citanjem daje 0-2 MB/s (bude se oko 2 s); `qnc-read-pace` jednim
citanjem probudi uredaj prije mjerenja. Izmjereno mimo cachea: citac uzastopno 421 MB/s, razbacano 1 citac 77 MB/s, 4
citaca 320, 8 citaca 354 MB/s; probe MXF-a 0,3 s (prvi 2,4 s, budenje). Filmstrip je ogranicen procesorom: 13 intra
slika XAVC 4:2:2 10-bit dekodira se 1,3-1,7 s po klipu; 4 klipa istodobno 0,92 s, 8 istodobno 0,81 s po klipu (oko
100-110 s za 122 klipa). Jedan tok svih 13 slika u jednom dekoderu proban i odbacen: na vecini klipova daje 2-7 slicica.
Razmak izmedu zapisa i filmstripa u probi bio je pustanje klipa (radnici se sklanjaju playeru). Testovi, conformance.
Izmjena 2026-10-19 (korisnik: "korisnici ce imati raznih medija i citaca"; mjereno na CFexpress citacu s klikom na Select
u korijenu: zapisi 3-18 s, filmstrip 22-118 s, grubi valovi 3 s, puni valovi 121-137 s, sve 122 klipa; `PRIVATE/DATABASE/
DATABASE.BIN` je kamerina binarna baza s putanjama u `XDROOT`, ne koristi se, `MEDIAPRO.XML` ostaje indeks): redoslijed
prema mediju. `qnc-import-control` (`Work`): zapisi, pa grubi valovi svih klipova (prvi prolaz testira medij), pa
filmstrip; puni valovi citaju uz filmstrip kad test medija pusti 4 ili vise citaca (CFexpress, SSD), inace nakon
filmstripa (stik, SD na USB 2.0); val otvorenog klipa nikad ne ceka. `qnc-wave-worker` `coarse_pending` (klipovi bez
ikakvog vala; klip ciji val ne moze nastati ne zadrzava filmstrip), `medium_parallel`; `qnc-timeline-artifacts`.
Testovi, conformance. Uzivo nije izmjereno.
Izmjena 2026-10-19 (korisnik: "neupotrebljivo sporo... pokazuj postere dok nemas napravljen filmstrip"; "koliko ti treba
za prvu i zadnju sliku"; "filmstrip radimo u dva reda: prvi samo prvu i zadnju, a kad se svi naprave tako, u drugom
krugu ostale"): (1) `IntraOnce` (4dae5f8) vracen (5448daa): 10,5 s po klipu, jer intra slike usred XAVC klipa nisu
oznacene kao tocke za skok (otvoreni GOP), pa dekoder pri svakom skoku krece od pocetka klipa. (2) Poster klipa stoji u
traci filmstripa dok filmstrip ne postoji, ne samo kad je filmstrip iskljucen (`qnc-timeline-assets` `with_clip_poster`
bez postavki; Ingest i Story/MA samo poziv). (3) Mjereno na mirnom procesoru: prva i zadnja intra slika kroz indeks
5-16 ms citanja, dekodiranje obje 0,38 s (pokretanje ffmpega 0,14 s); 13 slika 1,4 s. Filmstrip u dva kruga
(`qnc-filmstrip-worker`): klip bez ikakvog filmstripa najprije dobiva prvu i zadnju intra sliku (zapis s 2 slike,
indeksi 0 i 1, pa svaka ispuni pola trake), a kad ih svi klipovi imaju, drugi krug daje svih 13; spreman je samo zapis
s 13 slika; `qnc-source-preview` ponovno cita filmstrip dok nema svih 13. Bez indeksa sve kao prije. Testovi,
conformance. Uzivo nije izmjereno. Otvoreno: Wrap redovi (`qnc-program-filmstrip`) jednom procitan filmstrip ne citaju
ponovno.
Ispravak 2026-10-19 (korisnik: "prvu stavi na pocetak timelinea, zadnju na kraj"): filmstrip prvog kruga prikazuje se
prvom slikom na prvom i zadnjom na zadnjem mjestu trake, sredina prazna do drugog kruga (prije: svaka pola trake).
Zapis u bazi ostaje s rednim brojevima 0 i 1 (`qnc-artifact-db` trazi redom); polozaj daje citac `qnc-timeline-assets`
(`filmstrip_pictures`), pa vrijedi za source timeline i Wrap redove. Proba: prvi krug za 122 klipa 24-66 s nakon
Selecta (uz pune valove istodobno, 23-69 s), drugi krug od 72 s.
Popravak 2026-10-19 (korisnik: "blokirao je, nije prihvacao mis"; "mis je bio pobjegao na vanjski monitor, to treba
onemoguciti"): log: aplikacija je radila (crtala, odgovarala), ali od 19:44 nije dobila nijedan unos; pokazivac je bio na
HDMI ekranu, gdje ga prozor programskog izlaza skriva. `qnc-program-output` (`screens::keep_pointer_off`, Windows
`ClipCursor`): dok radi programski izlaz i QNC je aktivan, pokazivac je zadrzan na ostalim ekranima; cim QNC nije
aktivan, nema izlaza ili aplikacija zavrsava, pokazivac je slobodan. Ostali OS bez promjene. Testovi, conformance.
Izmjena 2026-10-19 (korisnik: "napravio prvu i zadnju i nije nastavio... ovo nije dobar put; idemo probati prioritet
kliknutog klipa"): log je pokazao 675 dvoslikovnih prolaza za 122 klipa (prvi krug se ponavljao), drugi krug gotov tek
359 s nakon Selecta; filmstrip u dva kruga uklonjen (e8b67a2, 54b37a3), poster u traci dok filmstrip ne postoji ostaje.
Izmjereno po klipu: indeks + 13 intra slika 9,7 s klip ~2,3 s, 68 s klip ~2,0-2,2 s (duljina ne utjece); stari put
(ffmpeg, 13 skokova) 4,2-5,3 s. `qnc-filmstrip-worker` `prefer`: klip otvoren u previewu (`PLAYBACK_CLIP`) ide prvi u
red i pokrece se odmah uz radnike koji vec rade (jedan radnik vise); `qnc-timeline-artifacts` `prefer` daje prednost i
valu i filmstripu. I dalje ceka dok player radi. Testovi, conformance. Uzivo nije izmjereno.
Izmjena 2026-10-19 (korisnik: "kako je nestao Auto i On za monitor.. sve moras drzati u kockama"; "1"): prekidac Monitor
Auto | On nikad nije bio u Ingestu (8. 10. dodan samo kao gumbi rasporeda Storyja). Sada je dio kocke monitora:
`qnc-monitor` (`MONITOR_AUTO`, `MONITOR_ON`, `paint_switch`) ga crta u gornjem desnom kutu monitora samo dok radi
programski izlaz na vanjskom ekranu i vraca kliknutu akciju; `paint_source_monitor` vraca tu akciju. Akcije vozi
zajednicka kocka tipki source panela (`qnc-source-panel`: `monitor_auto`/`monitor_on` -> `set_monitor_mode`), pa ih
svaka forma s preview monitorom dobiva istim putem; `qnc-source-preview` ih uzima iz `qnc-monitor`. Ingest prikaz nosi
`monitor_auto` iz previewa. Iz `contracts/ui/editorial.layout.json` uklonjeni su gumbi `monitor_auto`/`monitor_on`,
sifre teksta `monitor.auto`/`monitor.on`. Usput: pet ponovljenih "odbij i osvjezi" u `qnc-ingest-application` je jedna
funkcija (`rejected_shown`); C10 Ingest application 1552 -> 1547. Testovi, conformance. Uzivo nije provjereno (treba HDMI).
Ispravak 2026-10-19 (korisnik: "to je pogresna lokacija, mora biti gdje je bio i prije"): prekidac Auto | On vise nije
na slici monitora nego opet u redu ispod monitora, desno uz `>` (gdje je bio u Storyju od 8. 10.), i sada i u Ingestu
(uz `>`, `[`, `]`). Mjesto daje ugovor rasporeda (`contracts/ui/editorial.layout.json` Story, `contracts/ui/ingest.layout.json`,
`transport_right`, sifre `@monitor.auto`/`@monitor.on`); koji je ukljucen kaze `qnc-monitor::switch_on` (Story preko
`qnc-source-preview::monitor_mode_on`, Ingest `pool_head`), klik vozi `qnc-source-panel`. Prekidac se vidi uvijek, kao prije.
`qnc-monitor` vise ne crta prekidac. Media Assist gumbi nisu dirani (predlosci). Testovi, conformance.
Plan i korak 1, 2026-10-19 (korisnik: "ne govori o Storyju i Ingestu, govori o kocki, modulu i panelu; jednom napisana
kocka radi svugdje, moze biti u raznim modulima i panelima, ne moze biti u formama"; "da, kreni s korakom 1"): nalaz:
monitor, red ispod njega, source timeline s dugmadima i media pool slagali su se dvaput (`qnc-ingest-blocks` i
`qnc-editorial-blocks`), preglednik lokacija dvaput (Ingest i Project), krug fokusa dvaput (`qnc-focus-walk`,
`qnc-panel-focus`). Plan: kocka (jedna odgovornost) -> panel (javni modul koji slaze kocke u dio zaslona, sa svojim
tipkama, jednom napisan) -> forma (samo raspored: koji panel na koje mjesto ploce i koja dugmad). Koraci: 1 panel
monitora, 2 panel media poola, 3 panel source timelinea, 4 panel preglednika lokacija (trazi Project), 5 jedan krug
fokusa. Korak 1: nova javna kocka-panel `crates/qnc-monitor-panel` (+ `contracts/modules/monitor-panel.module.json`):
`show` crta monitor iz stanja univerzalnog previewa (`qnc-source-preview::MonitorState`, `PreviewView::monitor`),
`poster`, `controls` (dugmad reda ispod monitora koja raspored navodi i uvijek prekidac Auto | On s ukljucenim). Obje
ploce samo pozivaju panel; Auto | On uklonjen iz ugovora rasporeda (`transport_right`), pa ga dobiva svaka forma s
monitorom, i Media Assist. Ingest prikaz nosi `monitor` (stanje previewa) umjesto `monitor_auto`; poruka monitora u Ingestu
je sada ista kao drugdje (cekanje zapisa ili greska playera). Usput u Ingest aplikaciji jedna `refuse_shown` umjesto
cetiri ista mjesta. C10: `qnc-editorial-blocks` 348 -> 311, `qnc-ingest-blocks` 860 -> 826, Ingest application 1547 ->
1546. Red ispod monitora (tabovi + kontrole) jos slazu ploce dok tabovi ne prijedu u panel media poola (korak 2).
Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-19 (korisnik: "ako je ON, ON je u svim formama; glupo je sto puta postavljati"): Auto | On je jedan
izbor za cijelu aplikaciju u kocki `qnc-monitor` (`auto`, `choose`), ne stanje svakog previewa; `qnc-source-preview` ga
samo prosljeduje (`set_monitor_mode`), `MonitorState` i `PreviewView` vise nemaju svoj `auto`. Vrijedi dok aplikacija radi.
Preuredjen korak 1, 2026-10-19 (korisnik: "da, preuredi po strukturi kocka-modul-panel, ali to ne dozvoljava razlicit
razmjestaj panela unutar forme"; "dobar put: play i markere in/out mozemo koristiti i na drugoj lokaciji bez ponovnog
pisanja"): panel je najmanja jedinica koju raspored smjesta. Red ispod monitora su dva mjesta ploce: tabovi poola
(`pool-head`) i panel transporta (`transport`); `qnc-board` `Layout::split_place` dijeli mjesto, sirina mjesta transporta
je u ugovoru rasporeda (`pool_head.transport_width`: Ingest i MA 290, Story 180), `board_layout()` u
`qnc-editorial-layout`/`qnc-ingest-layout`. Nova javna kocka-panel `crates/qnc-transport-panel` (+ ugovor): dugmad iz
rasporeda (Play, IN, OUT) i uvijek Auto | On; `qnc-monitor-panel` crta samo monitor (bez dugmadi). Pogledi daju prikazani
klip (`shown_clip` u Ingest i Story/MA pogledu), forme samo smjestaju panele po imenima mjesta. Stil reda u
`qnc-editorial-layout` (`row_style`). C10: `qnc-editorial-blocks` 311 -> 302, `qnc-ingest-blocks` 826 -> 825, Ingest
application 1546 -> 1542. Testovi (`qnc-board` split, `qnc-transport-panel`), conformance. Uzivo nije provjereno.
Popravak 2026-10-19 (korisnik: "zasto promjena audio kanala na source timelineu blica na HDMI i preview monitoru";
"odabir je priprema, tek kad play krene primjenjuju se postavke"): log: svaka promjena kanala uzete trake otvarala je
novu sesiju playera (oko 0,7 s bez slike na monitoru i HDMI-ju). `qnc-source-preview` (`hear_channels`, `opened_lead`,
`lead_waits`, `reopen_with_lead`): dok player stoji, izbor kanala mijenja samo prikaz i pamti se; Play ga primjenjuje
(player se tada priprema na istom mjestu s novim kanalom i svira). Dok svira, promjena se cuje odmah kao prije. Razina i
dalje ide bez ponovnog otvaranja. Medukorak: korisnik je odredio cilj "player svira sve kanale, a na izlaz idu samo
odabrani" (preusmjeravanje bez ponovnog otvaranja), ceka otkljucavanje playera. Testovi, conformance.
Izmjena pravila i izvedba 2026-10-19 (korisnik: "player svira sve kanale, ali na izlaz idu samo odabrani"; "ok, tako se
rjesava problem"; otkljucano: `qnc-broadcast-engine`, `qnc-audio-output`, `qnc-player-contract`, `qnc-player-client`,
`tools/qnc-player-runner`, `qnc-source-preview`, `qnc-program-output`): zamjenjuje pravilo "dekodira se samo onoliko
kanala koliko projekt ima izlaza". Player klipa dekodira sve spremljene kanale izvora; projekt (`audio.channels`)
odreduje broj izlaza, a karta kanala (`ChannelMap`) koji kanal svira na kojem izlazu. `qnc-audio-output`: red nosi sve
kanale (`Config.source_channels`), poziv uredaja usmjerava (`set_routes`, atomski, bez alokacije; kao razina 16. 10.),
test. `qnc-broadcast-engine`: `project_audio_layout` bira sve kanale i rute iz `lead_audio_channels`, `DeviceSink` vise ne
usmjerava pri upisu u red, `set_output_routes`; testovi pravila promijenjeni. Nova naredba ugovora `SetOutputRoutes`
(runner, klijent `Action::SetOutputRoutes`). `qnc-source-preview` `hear_channels`: promjena kanala je ruta, cuje se odmah
i u pauzi i dok svira, bez nove pripreme (medukorak e5e6f7c uklonjen). Program (prica) nije mijenjan. Uz to
`qnc-program-output`: HDMI zadrzava zadnju sliku do 1,5 s dok monitor nema sliku (bljesak pri pripremi playera).
Testovi, conformance. `qnc-player-acceptance` nije pokrenut (projekti zive kopije su izvan njezina korijena). Uzivo nije
provjereno.
Ispravak 2026-10-19 (korisnik: "zasto u Story formi monitor nema in/out oznake"; "Story ne koristi, samo kocke, to nije
sastavni dio Storyja"; "nema ponovnog pisanja koda, samo se kocke slazu u module i panele"): popis dugmadi transporta bio
je u rasporedu svake forme (`pool_head.transport_right`), a Story ga je imao svoj, samo s Play. Popis je premjesten (ne
kopiran) u panel `qnc-transport-panel` (`BLOCKS`: Play, IN, OUT, Auto, On; natpisi u rjecniku `transport.*`), a iz oba
ugovora rasporeda i modela (`qnc-editorial-layout`, `qnc-ingest-layout`) je uklonjen; raspored daje samo mjesto i sirinu
(`transport_width`). Panel je isti u svakoj formi. C10: `qnc-editorial-blocks` 302 -> 301, `qnc-ingest-blocks` 825 -> 824.
Testovi, conformance.
Prijenos u stvarnu aplikaciju 2026-10-19 (korisnik: "dodaj u aplikaciju da je testiramo, dosad smo radili u zasticenom
direktoriju"; odabrao "da, oboje" i dijagnostiku): stvarni `data/ingest-transport.json` uskladen s izoliranom kopijom
(katalog kamera 2026.10.19.1: CFexpress s `XDROOT` u korijenu; probe 2 MB), prethodni spremljen izvan repozitorija;
`data/dev-diagnostics.json` ukljucuje logove playera, vala i filmstripa. Pokrenuta stvarna aplikacija iz `target/release`.
Izmjena 2026-10-19 (log stvarne aplikacije, CFexpress, 122 klipa: filmstrip 22-193 s nakon Selecta; 78 klipova do 10 s
citanih kroz sve slike prosjecno 4,3 s po klipu, 44 dulja kroz indeks 5,3 s; korisnik: "da, i kratki klipovi neka
uzimaju intra slike"): zamjenjuje pravilo od 18. 10. "do 10 s ubrzano citanje". `qnc-filmstrip-worker`: svaki klip ciji
original ima indeks uzima intra sliku najblizu svakoj od 13 pozicija (kod kratkog klipa slika se moze ponoviti); bez
indeksa sve kao prije. Testovi, conformance. Uzivo nije izmjereno.
Popravak 2026-10-19 (log stvarne aplikacije: filmstrip 22-253 s, 96 od 122; korisnik: "koliko traje filmstrip"): grubi
val kliknutog klipa napravljen je 95 puta (drugog 20), petlja je trosila karticu i procesor. Utrka u `qnc-wave-worker`:
grubi val ide na upis u pozadini, a klip se odmah vraca u red (kliknuti prvi); sljedeci prolaz procita bazu prije upisa,
ne vidi val i opet radi grubi. Sada radnik predaje prolazu da je grubi val vec napravljen (`coarse_made` iz `coarse`), pa
taj prolaz radi puni val. Filmstrip s intra slikama i za kratke klipove: prosjecno 4,2 s po klipu uz 4 radnika i petlju
valova. Testovi, conformance. Uzivo nije izmjereno. Otvoreno: test medija opet nesiguran (14/29/28/15 MB/s, izabrao 2
citaca).
Izmjena 2026-10-19 (korisnik: "ceka li filmstrip valove? filmstrip treba doci sto prije bez cekanja; ti procesi moraju
biti samostalni workeri, kao kocke"): zamjenjuje redoslijed od 15., 18. i 19. 10. (zapisi -> grubi valovi -> filmstrip ->
puni valovi). Filmstrip, valovi i dovrsetak zapisa su samostalni radnici; nitko nikoga ne drzi, svi i dalje ustupaju
playeru. Kocka `qnc-import-control` (+ ugovor) i lanac `hold_filmstrip` (`qnc-timeline-artifacts`,
`qnc-content-artifacts`, `tools/qnc-ingest-worker`) uklonjeni. U radnicima filmstripa i vala ostaju neiskoristeni
`set_on_orders`/`set_held` (zadano: rade). Testovi, conformance. Uzivo nije izmjereno.
Izmjena 2026-10-20 (korisnik: "u source timelineu OUT treba zaustaviti play"; "kad je ukljucen Sync/B-roll playhead se
pozicionira na najblizi marker, gleda polozaj playheada; IN u letu dok traje play i Sync/B-roll pise pokrivalicu; O
zaustavlja play, tada se jos moze korigirati OUT strelicama ili misem, Enter snima pokrivalicu"): (1) O na source
timelineu postavi OUT i zaustavi play, u svakoj formi (`qnc-source-panel`; Story kroz `qnc-panel-focus`;
`qnc-source-preview::pause`). (2) Ukljucen Sync stavlja Wrap playhead na M marker najblizi njegovu polozaju
(`qnc-program-segments` `toggle_sync`). (3) I pritisnut dok izvor svira postavlja IN i odmah pokrece Sync od tog markera
(`arm_sync(in_flight)`, `take_sync_flight`, `qnc-sync-session::poll`). (4) O za vrijeme Synca samo zaustavlja (prije je
zatvarao slot); kraj pokrivalice i dalje pomicu strelice, a sada i povlacenje OUT-a misem na source timelineu
(`qnc-sync-cover::drag_end`, `qnc-sync-session::dragged`); Enter zapisuje, Delete odbacuje. Testovi (novi:
`an_in_pressed_while_the_source_plays_starts_sync_once_it_is_armed`), zlatni testovi tipki, conformance. Uzivo nije provjereno.
Popravak 2026-10-20 (log probe Synca): (1) klik na Wrap red ponistavao je i zaustavljeni Sync, pa povlacenje OUT-a misem
vise nije imalo sto pomicati (OUT izvora otisao na 1018). Sada O izricito zaustavlja Sync (`qnc-sync-cover` `stop`,
`stopped`; `qnc-program-segments` `sync_stop`), a zaustavljeni nacrt ostaje do Entera ili Deletea: klik na Wrap red tada
samo kaze sto ceka (`cue_program`). (2) Prica je imala 175 slika i prvi Sync je pokrio zadnji slot (98-175); drugi Sync
krenuo je u isti slot i tek je Enter rekao da slot ima B-roll. Sada Space odmah odbija Sync u slot koji vec ima
pokrivalicu (`sync_space`). Disk ima ~6 GB slobodno: puna debug gradnja pada na PDB datotekama (LNK1318), gradi se s
manje poslova istodobno. Testovi, conformance.
Popravak 2026-10-20 (korisnik: "izgubio si kontrole tockom.. pokvario si.. audit napravi"): audit loga pokazao je da
tocak nije pokvaren sam po sebi: u 12:09 Edit segmenta zavrsen je s Replace umjesto Enterom, edit je ostao otvoren, pa je
svaki kasniji Enter isao njemu ("The segment is edited on its own clip"). Sync zaustavljen s O zato se nikad nije
spremio; ostao je zaustavljen, klik na source timeline nije pomicao playhead (source timeline pokazuje kraj Synca), a
tocak i strelice micali su samo kraj Synca. `qnc-program-segments`: Enter na source timelineu najprije sprema Sync, pa tek
edit segmenta (`confirm_source`); Replace, Exclude, Purge, novi segment, Undo i Redo zatvaraju otvoreni edit bez upisa
(`end_edit`). Test `replace_exclude_or_a_new_segment_end_an_open_edit`. Uzivo nije provjereno.
Popravak 2026-10-20 (korisnik: "kraj i pocetak su aktivni za povlacenje bez da prethodno dam komandu Ctrl ili desni
klik misa"): na source timelineu (sve forme) pocetak i kraj klipa, koji se pokazuju kao IN/OUT dok korisnik nije
postavio oznaku, vise se ne hvataju misem ni ne crvene u zoni dok nisu uzeti (Ctrl+I/O, desni klik, Ctrl+kotacic).
Oznaka koju je korisnik postavio i dalje se vuce izravno iz zone. `qnc-timeline` (`grabbable_marks`). Testovi. Uzivo nije
provjereno.
Popravak 2026-10-20 (korisnik: "pogledaj log"): kotacic (i strelice) iznad Wrap redova, kad nista nije uzeto a zadnje je
koristen source timeline, nije radio nista (`routed=Some(false)` desetke puta: `WrapSession::step` radi samo u Wrapu).
Sada je korak tada klik na Wrap red jednu sliku od Wrap playheada, pa se Wrap vraca (`qnc-segments-keys` `step`); prazna
prica i dalje ne radi nista. Zlatna tablica tipki nepromijenjena. Nalazi loga bez izmjene: testovi pisu `story-key` u
stvarni `data/diagnostics/player.log` (1,97 GB, bez rotacije); frameovi sucelja Storyja 25-89 ms dok radi prerender.
Otkljucano i izvedeno 2026-10-20 (korisnik: "pogledaj sto se sve moze iz formi izvuci u kocke i module"; "da, otkljucaj
Project i kreni s 1 i 2"; Project otkljucan samo za preglednik lokacija): (1) nova javna kocka-panel
`crates/qnc-dir-browser-panel` (+ `contracts/modules/dir-browser-panel.module.json`): jedini preglednik lokacija (izvori,
Gore, Diskovi, putanja / koraci putanje / diskovi racunala, mape, greska, ucitavanje, okvir fokusa); crta neutralne retke
koje forma da i vraca radnju. Ingest (`qnc-ingest-blocks` `location_browser.rs`) i Project (`qnc-project-blocks`
`location_browser.rs`) samo predaju svoje stanje i tekstove; `browser_entries.rs` i pomocni linkovi Ingesta uklonjeni.
Boja greske je jedna (`qnc-desktop-theme::danger`), pa je u Projectu sada ista kao u Ingestu. (2) Mreza media poola:
`qnc-media-card` `show_pool` sam slaze kartice, tockice stanja, oznaku i natpis spremanja iz svojstva `PoolItem` koje
daju klip Ingesta (`ClipView`), klip Storyja/MA (`EditorialClip`) i short/B-roll (`qnc-virtual-short-cards`); forme vise
ne grade `CardRow`. Da aplikacijski slojevi ne narastu, duplikati su izasli u kocke: zadrzavanje ucitanih postera i popis
postera koji nedostaju (`qnc-media-thumbnail` `LoadedThumbnail`, `keep_loaded`, `missing`, test) i citaci izvora
(`qnc-source-bindings` `TransportBindings::readers`, `SourceBinding::reader`; Story je imao svoju kopiju). C10 spusteno:
Ingest ploca 824 -> 561, Story ploca 301 -> 206, Project ploca 1932 -> 1714, Ingest aplikacija 1542 -> 1538, Story
aplikacija 649 -> 643; uklonjene iznimke C1 (oba preglednika), C2 boja (Project preglednik), C3 (obje mreze). Testovi,
conformance. Uzivo nije provjereno. Project je ponovno zamrznut.
Izvedeno 2026-10-20 (korisnik: "da" na plan kocka -> modul -> panel -> forma, korak 1: samostalnost komponenti):
`GridStep` i `step_focus` (koraci tipki popisa) iz `qnc-media-card` u kocku `qnc-key-list` (kartice ih koriste, ne
obrnuto); `qnc-focus-walk` vise ne ovisi o mrezi kartica ni o panelu source timelinea (naziv opsega tipki source
timelinea je `qnc-key-intents::SOURCE_TIMELINE_SCOPE`); tipovi stanja monitora (`MonitorFrame`, `MonitorState`) iz modula
playera u kocku `qnc-monitor`, a kocka monitora vise ne poziva HDMI izlaz (to radi panel monitora), pa panel monitora ne
povlaci player; nova kocka `crates/qnc-still-locator` (+ ugovor): gdje ovo racunalo cita sliku klipa, izdvojeno iz
`qnc-source-preview`, pa skok na kljucnu sliku (`qnc-keyframe-nav`) ne ovisi o playeru; panel Segmenti ovisi o kocki
prikaza `qnc-program-view`, ne o modulu price s bazom. Nije izvedeno (trazi Project): `qnc-project-session` jos ovisi o
Project aplikaciji. Testovi, conformance.
Izvedeno 2026-10-20 (isti plan, korak 2: redak klipa): nova kocka `crates/qnc-clip-row` (+ ugovor; bez sucelja): jedan
redak klipa za sve forme (`ClipRow`: naziv, trajanje, poster, stanje uvoza i spremanja, kvacica) sa svojstvom kartice
`PoolItem` (premjesteno iz `qnc-media-card`, koji ga samo koristi) i posterima (`LoadedThumbnail`). `ClipView` Ingesta i
`EditorialClip` Storyja/MA samo su to ime za `ClipRow`; katalog (`qnc-ingest-catalog` `CatalogClipItem`) i Select
dogadjaji (`qnc-ingest-clip-list` `ClipListItem`) ga sami pune, Story ga gradi iz sazetka kataloga
(`ClipRow::from_summary`). Sto kvacica kartice oznacava (nista, izabrani, odabrani za uvoz) je `qnc-media-card` `Ticks`
u postojecem polju znacajki, pa forme nisu narasle. C10 spusteno: Ingest aplikacija 1538 -> 1378, Story aplikacija
643 -> 609. Uz to obrisan debug build (`cargo clean --profile dev`, 51 GiB, uz dozvolu korisnika; disk je bio pun).
Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-20 (isti plan, korak 3: paneli Ingesta i Storyja): ploce `qnc-ingest-blocks` i `qnc-editorial-blocks`
samo smjestaju panele (C10 Ingest ploca 561 -> 155, Story/MA ploca 206 -> 101). Nove kocke i paneli (s ugovorima):
`qnc-theme-styles` (izgled svake kocke iz jedne teme: timeline, dock, red, gumb, traka potvrde, monitor, kartice; kopije
u formama i u `qnc-editorial-layout` uklonjene), `qnc-media-pool-panel` (mreza poola na svom mjestu), `qnc-pool-tabs-panel`
(tabovi poola), `qnc-ingest-import-bar` (traka uvoza u zaglavlju source timelinea). Paneli primaju temu: transport,
monitor (sam uzima poster prikazanog klipa kroz `PoolItem`), source timeline (`header_actions`: gumbi zaglavlja s
okvirom fokusa), preglednik lokacija (`show_with_confirm`: preglednik s trakom Select/Cancel; Project i dalje koristi samo
preglednik). Jedan okvir fokusa `qnc-ui-kit::paint_focus` (tri kopije uklonjene). Conformance "shared UI patterns":
traka potvrde preglednika se trazi u panelu preglednika, a Ingest forma je ne smije crtati sama. Testovi, conformance.
Uzivo nije provjereno.
Izvedeno 2026-10-20 (isti plan, korak 4: jedan krug fokusa i domacin forme): Story fokus (`qnc-panel-focus`) vise ne vodi
svoj redoslijed panela, redove tabova i gumba ni korake po karticama: koristi krug `qnc-focus-walk` (Ingestov; javni
koraci `go_to`, `next_panel`, `reachable`, `chosen_in`, `select_in`, `choose_in`, `press_list`, `last_step`,
`take_opening`), a svoje zadrzava samo ono sto je price (Wrap, Sync, segmenti); `Panel::name` daje ime panela iz ugovora.
Zlatna tablica tipki (830 slucajeva) nepromijenjena. Nova kocka `crates/qnc-form-host` (+ ugovor): jedna petlja forme
(budjenje playera, poll, ritam crtanja, tipke kao akcije kataloga, slanje radnji) preko svojstva `FormApplication`;
Ingest i Story desktop samo omotaju svoju aplikaciju (`Hosted`). C10 Ingest desktop 76 -> 70, Story desktop 86 -> 82.
Project desktop (zamrznut) jos ima svoju petlju. Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-20 (korisnik: "ako vec ima kreiran filmstrip zasto se najprije pokaze poster, pa filmstrip"; "treba
gledati fokus na media poolu, filmstrip se ucita u memoriju ako postoji, pa ako korisnik klikne odmah prikaze"; "to treba
biti samostalna kocka"): nova kocka `crates/qnc-pool-pointer` (+ ugovor): mreza kartica (`qnc-media-card`) javlja klip pod
misem, inace karticu na kojoj je tipkovnica; form host ga preuzima jednom po frameu i predaje aplikaciji u `poll`;
`Follow` daje svaki novi klip jednom. Univerzalni preview (`qnc-source-preview` `poll_pointed`) tada u pozadini cita
filmstrip i val tog klipa u memoriju, pa klik pokazuje filmstrip odmah. Klip koji nije bio pokazan i dalje kratko pokaze
poster. Ingest i Story isto; forme nisu narasle. Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-10-20 (korisnik: "player radi, IN odreduje od kojeg timecodea pocinje pokrivalica, to je samo broj, play
tece; korisnik klikne OUT ili Enter i cita broj; ne treba otvarati nikakve playere kad player vec radi"; izmjereno: IN u
letu do spremne Sync sesije 1,24 s): zamjenjuje v5 nacin (Sync otvara prozor price s privremenom pokrivalicom u novoj
sesiji playera). Sync/B-roll svira na playeru izvora: IN u letu ne dira player, nacrt pokrivalice prati potvrdjenu sliku
izvora (`qnc-sync-cover` `source_window_frame`, `qnc-program-segments` `sync_frame(program, clip)`,
`qnc-source-preview` `clip_frame`); Space bez playa pusta izvor od IN-a (ili od kraja zaustavljenog nacrta); dolazak do
M markera ili OUT-a izvora zaustavlja izvor na zadnjoj slici nacrta; strelice i povlacenje OUT-a pokazuju kraj na
playeru izvora (`qnc-sync-session`). Pauza se ne salje dvaput dok player jos svira (`qnc-source-preview` `pausing`).
Posljedica: za vrijeme Synca cuje se zvuk izvora, A1 price ispod se ne cuje. Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-20 (korisnik: "ne cuje se A1"; "treba se cuti A1 price, kako bi korisnik mogao znati do kuda treba drzati
klip"): A1 price ispod Sync slota ne ovisi o IN-u, pa se priprema unaprijed. Nova kocka `crates/qnc-sound-bed` (+ ugovor):
svoj Broadcast Player za A1 price ispod Sync prozora, pripremljen dok je Sync ukljucen (prozor od M markera na ili prije
Wrap playheada do sljedeceg; `qnc-program-segments` `sync_window_ahead`), na IN samo krene (bez otvaranja playera);
staje kad Sync stane. Program prozora je samo A1 (`qnc-program-input` `sound_bed_loader`, `sound_only`: bez slike i bez
A2). `qnc-source-preview` ga drzi (`prepare_sound_bed`, `play_sound_bed`, `stop_sound_bed`) i dok svira, izlaz 1 playera
izvora je utisan (A2 izvora ostaje). Odstupanje od 8.2 uz zahtjev korisnika: za vrijeme Synca rade dva playera (izvor i A1
price), svaki svojim satom. Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-10-20 (korisnik: "A1 se cuje, ali to ocito nije broadcast kvaliteta"; log: A1 drugog playera 3-4 slike iza
slike izvora, dva sata i dva zvucna izlaza; "da, otkljucaj i kreni, napravi to kao dio sync kocke"): zamjenjuje prethodni
zapis (`qnc-sound-bed`, drugi player, uklonjen). Otkljucano i izvedeno: `qnc-audio-output` (`bed.rs`: mono sloj ispod
reprodukcije, zvucni uredaj ga pusta na izlazu 1 od zadanog apsolutnog uzorka, u istom pozivu i satu; zakasnjeli uzorci se
odbacuju, test), `qnc-broadcast-engine` (`ProgramSound` izdvojen iz `ProgramAudio`; `sync_bed.rs`: A1 Sync prozora
dekodira se unaprijed u sloj uredjaja klipa; `Runtime::with_sync_bed`, `start_sync_bed`, `stop_sync_bed`),
`qnc-player-contract` (naredbe `StartSyncBed { source_frame, bed_frame }`, `StopSyncBed`), `qnc-player-client`
(`LaunchInput::Clip.sync_bed`, akcije), `qnc-player-launcher` (`prepare_launch_with_sync_bed`), `tools/qnc-player-runner`
(`sync_bed` u pokretanju klipa), `qnc-program-input` (`sound_bed_loader`, `sound_only`), `qnc-source-preview` (sesija klipa
nosi A1 prozora sljedeceg Synca: `want_sync_bed`, ponovno otvaranje samo dok klip miruje; `play_sync_bed`,
`stop_sync_bed`), `qnc-sync-session` (IN/Space salje `StartSyncBed` od slike IN-a, Sync koji stane salje `StopSyncBed`).
Jedan player, jedan sat, jedan zvucni izlaz; A1 price pocinje na uzorku slike IN-a. Testovi, conformance. Uzivo nije
provjereno.
Otkljucano i izvedeno 2026-10-20 (korisnik: "idemo testirati koliko treba procesor jezgri za montazu i koliko bi mogli
koristiti za filmstrip i wave paralelno"; "da, otkljucaj i kreni sa svim tockama"; "ako je PAL vise od 25 osvjezavanja u
sekundi mozda i ne treba"): mjereno na laptopu (i5-1135G7, 4 jezgre / 8 niti): montaza bez pozadine sucelje 0,85 i player
0,97 jezgre (medijan; 90 % vremena ukupno 3,5 od 8); pozadina kad radi medijan 2,4, do 5,8 jezgri, ali se dosad za vrijeme
playa (i 3 s nakon svakog koraka) uopce nije pokretala. Player uz N paralelnih dekodera originala (izolirana kopija,
`qnc-player-acceptance`): 1 jezgra bez gubitka, 2 jezgre pocinju trzaji, 4 jezgre prica 90 % slika i A/V do 1,8 slike.
Izvedeno: `qnc-playback-activity` tri razine (`PlaybackLevel`: svira `on`, montira se `on:editing`, miruje; tko ceka bilo
koji rad playera i dalje ceka obje), `playback_level`, `background_cap` (svira 1, montira se 2, miruje sve),
`PlaybackReporter::update_level`; `qnc-source-preview` javlja razinu; `qnc-filmstrip-worker` i `qnc-wave-worker` dok
player svira rade samo klip otvoren u previewu, sam (ostali se prekidaju i vracaju u red), inace najvise `set_cap` radnika;
`qnc-timeline-artifacts`, `qnc-content-artifacts` (`set_cap`), `tools/qnc-ingest-worker` (cita razinu). Crtanje sucelja:
dok player svira crta se samo uz njegovu novu sliku (stopa izvora: 25, 30, 50), nikad uz ritam aplikacije
(`qnc-paint-pace`; mjereno 61 crtanje u sekundi za 50p); `qnc-level-meter` sam trazi crtanje samo dok mu razina pada bez
novih razina. Testovi, conformance. Uzivo nije provjereno.
Audit i ciscenje 2026-10-20 (korisnik: "moras napraviti audit, srediti kod, sve svesti na kocke, ne raditi paralelni
kod"; "nastavi" na prijedlog faza 0-4, sve osim Projecta). Audit (cetiri citanja koda): paralelni kod (raspored radnika
filmstripa i vala dvaput, fokus Story/Ingest dvaput, veze izvora 4+, otvaranje baze projekta 3, citanje artefakata 2+3,
timecode/trajanje 4-5, PTS/dB/prvi video tok 3/7/12, pomocni kod tablica baze 5, raspored/tema/korijen/pokretanje
pomocnih programa/izbor kodera vise puta), logika u aplikacijskim slojevima i posao pri svakom crtanju, putovi unosa.
Faza 0 (greske): I, O i rez vise ne "uzimaju" oznaku (`qnc-timeline`; sljedeci klik je pomicao playhead tek drugi put);
klik na Wrap redak dok je nesto uzeto je nova radnja kataloga `segments_keep_edit` (sprema, nikad ne prelazi na drugi
panel); isti zastoj playera zapisuje se jednom (`qnc-broadcast-engine`, prije 2,7 milijuna redaka); testni programi
(`target/*/deps` bez `QNC_ROOT`) ne pisu u stvarnu dijagnostiku (`qnc-dev-diagnostics`); cue dok se player priprema
ceka kao kod programa, jedno polje (`qnc-source-preview` `cue_waiting`). Faza 1 (mrtvi kod): stari Sync kroz prozor price
(`open_program_window`, `TransientCover`, `window_loader`, `load_program_window`, `with_transient_cover`,
`sync_window_frame`, `stopped_window_frame`, `SyncPreview.timebase`), filmstrip "po narudzbi" i predane slike
(`set_on_orders`, `request`, `given`), zadrzavanje vala (`set_held`, `coarse_pending`, `medium_parallel`, `without_wave`),
`card_busy` u dovrsetku zapisa, kocka `qnc-ingest-runtime` (prosljedjivala `qnc-playback-activity`). Testovi,
zlatne tablice tipki, conformance.
Pravilo unosa (korisnik 2026-10-20, ZAKLJUCANO; "da, odobravam, kreni"): svaki ulaz poziva sifru radnje iz
`contracts/qnc-keyboard-shortcuts.json`, po istom pravilu kao tipke. Tipka radi odmah; Ctrl+ uzima kontrolu nad
postojecim (IN, OUT, M marker, slot, A1/A2; M je OUT i IN u jednoj oznaci); Shift+ dodaje; Alt+ skace; Enter potvrda,
Esc odustani, Delete brise uzeto. Mis: lijevi klik = playhead (na traci u boji pokrivalice bira slot; dok je nesto uzeto
klik ga najprije sprema kao Enter, pa postavlja playhead); desni klik = Ctrl+ (bliži IN ili OUT, najbliži M marker bez
obzira na udaljenost, nikad zakljucani pocetni ni zavrsni; na traci u boji slot; A1/A2); povlacenje vuce samo uzeto;
Shift + lijevi klik = Shift+M (M marker na mjestu klika); srednja tipka = rez; kotacic = strelice lijevo/desno,
Ctrl+kotacic i kotacic s drzanom desnom tipkom = Ctrl+strelice. Izvedeno: nova kocka `crates/qnc-mouse-map` (+ ugovor
`contracts/modules/mouse-map.module.json`) jedina cita mapu misa `contracts/input/mouse.json` (pokazivac na panelu ->
sifra); `qnc-timeline` kaze samo sto je pokazivac napravio (`Keep(frame)`, `AddMarkerAt`, `LaneLabel`,
`TakeCoverOnStrip`; uklonjeni `SelectMarker`, `TakeSlotEdge`, `slot_hit`), `TimelineInput.holding` (domacin drzi nesto
uzeto na redu), `TimelineMarkerPin.taken`/`locked`; panel Segmenti i Ingest vise ne odlucuju o kliku. Jedan vlasnik po
sifri: IN/OUT u Storyju oznacava `qnc-source-panel` (slike IN/OUT prati `qnc-virtual-short-stills::follow`), Play u
Ingestu ide kroz isti panel (Ingest dodaje samo svoju zastitu; uklonjen `PreviewCommand`/`send_command`), klik na
source timeline je svugdje `timeline_cue_frame` (uklonjen `ingest_cue_frame`), Story Ctrl+kotacic pomice uzeti IN/OUT.
Story i MA vise ne citaju opseg tipki `off` (ostaje u katalogu kao preslika qnc_v4); uklonjene mrtve sifre
`timeline_select_marker`, `ingest_cue_frame`, `ingest_toggle_audio_lane`, `shell_close_active_project`. Zakljucano
testovima: zlatna tablica misa `crates/qnc-timeline/tests/mouse_matrix.golden`, zlatna tablica tipki (izmijenjeni samo
retci `mark_in`/`mark_out` i Ctrl+kotacic bez uzetog), `marker_reach` i `qnc-mouse-map` testovi. Promjena bilo kojeg
odgovora trazi korisnikovu odluku. Uzivo nije provjereno.
Promjena pravila i izvedba 2026-10-21 (log zive probe; korisnik: "odbij uzimanje markera i otkljucaj player, ujedno ako
se pokusa uzeti nesto novo daj upozorenje koje je uocljivo; ton bi se trebao cuti kad god se pomice playhead, unatrag ili
unaprijed, slicica po slicica, nekad se montira po zvuku"). Nalaz: dok je Sync cekao Enter, desni klik je uzeo M marker
i kotacic ga je pomicao; Enter je spremio Sync ciji slot vise nije postojao, pa je Sync visio ("The Sync slot is not there
yet") do Deletea; svaki korak kotacica po Wrap retku otvarao je zvuk programa iznova (577 pokretanja ffmpega u 10 min),
player je na skok odgovarao 60-500 ms, a program pri koraku nije imao zvuk. (1) Zamjenjuje pravilo od 17. 10. ("svako
uzimanje pusta prethodno"): dok nesto ceka Enter (Sync koji svira ili ceka Enter, uredivanje segmenta, M marker, slot, rub
slota, zvuk segmenta ili pokrivalice, uzeta pokrivalica), novo uzimanje necega drugog se odbija; ponovno uzimanje istoga
prolazi. Odbijanje pokazuje nova pasivna kocka `crates/qnc-alert` (+ ugovor): crvena traka na vrhu sredine ekrana s
tekstom velikim slovima 3,5 s (`story.msg.take_refused`). `qnc-program-segments` (`confirm.rs` `held`, `refused`,
`take_of`: provjera prije svake promjene odabira), `qnc-program-view` (`SegmentsView.alert`), `qnc-segment-panel` je crta.
Uzimanja na source timelineu (Ctrl+I/O, A1/A2) nisu mijenjana; izlazak s panela Segmenti i dalje pusta uzeto kao Esc.
(2) Otkljucano i izvedeno: `qnc-broadcast-engine` - odsvirani zvuk ostaje u memoriji 2 s (`PcmTrack` `history`), skok
unatrag ili do 1 s unaprijed sluzi se njime i otvorenim dekoderom (`can_aim`, `aim`, `settle`), skok izvan toga otvara
dekoder 1 s prije trazene slike (`restart`), pa i koraci unatrag nakon njega nalaze zvuk; isto za klip i za trake programa
(trake otvorene unaprijed za sljedecu stavku ne citaju unatrag); program pri skoku pusti zvuk te slike kao klip
(`audition_at`). `qnc-player-client`: niz skokova ide playeru kao zadnji (`next_action`), ostale naredbe redom. Testovi:
`a_cue_back_or_forward_within_the_sound_in_memory_needs_no_new_decoder`, `a_run_of_cues_goes_as_its_latest...`,
`a_new_take_is_refused_with_a_warning_while_another_waits_for_enter`, `a_marker_is_not_taken_while_a_sync_waits`.
Uzivo nije provjereno.
Pravilo tipki 2026-10-21 (korisnik: "Backspace brise zadnju radnju, a Delete brise objekt"; "slazem se s tvojim
prijedlozima"; "staro pravilo" za Delete): (1) Esc je jedna radnja `clear_focus` (`close_player` nema tipku u QNC presetu i
nijedna kocka ga ne obraduje; u drugim presetima Esc i dalje daje obje sifre, ali radi samo `clear_focus`). Esc odustaje
od onoga sto nije zapisano, jedno po pritisku, najblize prvo: pusti uzeto (IN/OUT, A1/A2, marker, slot, rub, zvuk,
pokrivalica), odbaci nacrt Synca koji nije spremljen (prije Delete), odustani od uredivanja segmenta, zatvori uvecani slot.
(2) Delete brise objekt, samo ono uzeto (pravilo 25. 9. ostaje; klik sam ne naoruzava Delete): uzeti M marker, uzeti
B-roll, B-roll uzetog ili izabranog slota; nacrt Synca nije objekt (poruka `story.msg.sync_draft_is_escape`). Jedna
sifra `delete_marker` (Delete; `delete_part` bez tipke u QNC presetu), `qnc-program-segments` `delete.rs`. Ctrl+Delete
je `segment_purge`: odabrani iskljuceni segment trajno (`PurgeSelected`). (3) Backspace je Undo (`undo_object`),
Shift+Backspace Redo; Ctrl+Z, Ctrl+Y i Ctrl+R ostaju. U polju za tekst Backspace brise slovo, u Ingestovom pregledniku
lokacija znaci Gore. Katalog (QNC preset `storyboard`, `source_timeline`), pomoc F1 (pravila Esc, Delete, Backspace),
conformance iznimke od qnc_v4 (`close_player`, `delete_part`, `delete_marker`, `undo_object`, `redo_object`; oznake akcija
ostaju v4). Zakljucano testovima `backspace_is_undo_and_delete_and_escape_have_one_action_each` i
`delete_leaves_a_sync_draft_alone_escape_drops_it`. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-21 (korisnik: "ton pokrivalice da udje u J na ton-izjavu, da ne bi skok slike i A2 bio
odjednom"; news praksa: slika rez, ambijent pokrivalice nikad ne skace; "da, otkljucaj Project i kreni"): (1) Project
postavka `audio.crossfade_frames` (sistemski templatei 4; Advanced "Audio editing" 0/2/4/6/8; stariji projekt je dobiva
pri otvaranju u Projectu; `qnc-work-settings::audio_crossfade_frames` cita strogo, najvise 50). Project je ponovno
zamrznut. (2) Svaki rez A2 pokrivalice ima crossfade: zvuk pokrivalice krece `crossfade_frames` prije svoje slike (J,
koliko izvor ima slika prije IN-a i koliko ima mjesta), raste, a A2 prije nje (ako ide do reza) pada preko istih slika;
bez rucke zvuk raste preko svojih prvih slika; A2 bez pokrivalice iza pada preko svojih zadnjih. A1 price se nikad ne
mijesa. `qnc-program-playlist` (`crossfade.rs`; ruta zvuka nosi `fade_in`/`fade_out` u slikama programa, dvije rute
smiju dijeliti izlaz samo dok se mijesaju; `a2_crossfade_frames`, `a2_lead_frames` pokrivalice, zasad 0),
`qnc-broadcast-engine` (trake se zbrajaju s rampom; granice rampe su rezovi jedinica prerendera),
`qnc-render-unit` (`AudioPart.fade`, u otisku samo kad postoji), `qnc-program-render`, `qnc-player-input`
(`ClipSession::settings`), `qnc-program-input` (`ClipInputs::crossfade_frames`). Aktivni projekt bez postavke ne otvara
program (kontrolirana greska) dok se ne otvori u Projectu. Nije izvedeno: vlastiti J pokrivalice u bazi (Ctrl+J) i slika
govornika prije njegova tona. Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-21 (korisnik: "da, kreni s J i L za svaku pokrivalicu"): svaka pokrivalica ima svoj J (zvuk A2 ulazi prije
slike) i L (zvuk A2 ostaje nakon slike), u slikama price, 0-500. `qnc-program-db`: stupci `story_covers.a2_lead_frames` i
`a2_tail_frames` (stare price dobivaju 0 pri otvaranju), operacija `SetCoverOverlap` (jedan Undo korak), podjela pokrivalice
markerom ostavlja J prvom dijelu i L drugom; ugovor `story.database.json` (`cover_overlap`). `qnc-program-playlist`
(`crossfade.rs`): na rezu dviju pokrivalica prozor je od J sljedece prije reza do L prethodne poslije reza, preko njega
prethodni zvuk pada a sljedeci raste; rez bez J i L dobiva `audio.crossfade_frames` kao J; svaka strana koliko izvor ima
slika (prije IN-a za J, poslije OUT-a za L) i koliko ima mjesta; L bez pokrivalice iza ide pod izjavu i pada.
`qnc-program-input` predaje oba. Tipke (QNC preset, scope `storyboard`): Ctrl+J uzme J, Ctrl+L uzme L pokrivalice pod Wrap
playheadom (inace izabrane ili one izabranog slota); lijevo/desno (i kotacic) dulje/krace, bijeli trim playhead pokazuje
gdje zvuk pocinje ili zavrsava, Enter sprema, Esc odustaje; dok je uzet nista drugo se ne uzima (`qnc-program-segments`
`overlap.rs`, `Take::Overlap`). Tablica tipki zna J i L. Nije izvedeno: nacrt se cuje tek nakon Entera (program se ponovno
otvara iz baze), J/L se ne crta na valu Wrap reda, slika govornika prije njegova tona. Testovi, conformance. Uzivo nije
provjereno.
Dopuna 2026-10-21 (korisnik: "neka se J i L cuje odmah, prije Entera"): dok je J ili L pokrivalice uzet, svaka promjena
(strelice, kotacic) ponovno otvara program s nacrtom na slici koju pokazuje, kao nacrt zvuka segmenta; Esc vraca spremljeni
J/L, Enter ga zapisuje. `qnc-program-input` `SoundDraft.overlap`, `qnc-program-segments` `overlap_draft`,
`qnc-segments-keys` `overlap`, `qnc-panel-focus` (poziva ga uz `sound`), `qnc-source-preview` (nacrt J/L nikad ne ide
kao razina izlaza). Svaki korak ponovno slaze program (oko 0,5 s). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-21 (korisnik: "sada bi trebali i J sliku"; "ne nekoliko slicica, nego da moze proizvoljno"): J slika Ton
segmenta: slika govornika pocinje prije njegova tona, preko kraja prethodnog segmenta (Off pokazuje govornika umjesto crnog,
Ton se reze ranije); zvuk oba segmenta ostaje gdje je, pokrivalice prethodnog segmenta ostaju iznad. `qnc-program-db`:
`story_parts.v_lead_frames` (stare price 0), `SetSegmentPictureLead` (samo Ton, jedan Undo korak); ugovor `picture_lead`.
`qnc-program-playlist` (`picture_lead.rs`): granica je koliko izvor ima slika prije IN-a i duljina prethodnog segmenta.
Duljina J/L pokrivalice i J slike vise nije ogranicena na 500 slika (360 000, dva sata pri 50p; granice daju izvor i
susjedi). Tipka (QNC preset): Ctrl+Shift+J uzme J sliku Ton segmenta pod Wrap playheadom; strelice/kotacic dulje/krace,
odmah se vidi i cuje, Enter sprema, Esc odustaje (`qnc-program-segments` `overlap.rs` `Kind`, `OverlapDraft`;
`qnc-program-input` `SoundDraft.picture_lead`). Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-10-21 (korisnik: "M marker je rez cijelog klipa, sluzi kad pomicemo cijeli marker; ako ne pomicemo cijeli
marker nego samo njegov ton ili njegovu sliku koristimo I/O i 1 ili 2"; "tipkovnica Ctrl+ I/O/M/1/2; desna tipka misa je
Ctrl+, iznad slike ekvivalent I, iznad tona ekvivalent tona; tocak pomice"): zamjenjuje Ctrl+J / Ctrl+L za J/L pokrivalice
(uklonjeni iz kataloga). Rez M markera ima dijelove (`qnc-program-segments` `cut_part.rs`, `CutPart`): Ctrl+M uzme
najblizi marker i strelice/kotacic pomicu cijeli rez (`MoveMarker`); dok je uzet Ctrl+I ili Ctrl+O pomice samo sliku, ton
ostaje (`MoveMarkerPicture`: marker se pomakne, J/L pokrivalica s obje strane preuzimaju razliku), Ctrl+2 samo ton A2
(`SetCutSound`: kasnije L pokrivalice prije, ranije J pokrivalice poslije; trim playhead pokazuje ton; nacrt se odmah
cuje, `SoundDraft.overlaps`), Ctrl+M opet cijeli rez, Ctrl+1: A1 nema rez na markeru (poruka). Mis (`contracts/input/mouse.json`,
promjena zakljucane mape uz odluku korisnika): desni klik iznad slike Wrap reda = najblizi marker, samo slika
(`select_mark_in` s ciljem markera); desni klik iznad A2 = najblizi marker, samo ton (`right_click_marker_a2` ->
`select_audio_a2` s ciljem; `qnc-mouse-map`, `qnc-timeline` `TakeMarkerSound`); desni klik na A1 ostaje zvuk segmenta.
Zlatna tablica misa: promijenjena 2 retka i dodan 1. J slika segmenta ostaje Ctrl+Shift+J (granica segmenata nije M
marker). `qnc-program-db` (`MoveMarkerPicture`, `SetCutSound`, jedan Undo korak), `qnc-program-commands`
(`SegmentCommand::CutPart`), `qnc-segments-keys` (`overlap` prije `sound`), pomoc F1. Testovi, conformance. Uzivo nije
provjereno.
Ispravak 2026-10-22 (korisnik: "desni klik misem treba uzimati cijeli marker, to se najcesce radi; za izbor
samostalne slike ili tona treba dodati nesto"; "Ctrl+M moze raditi u segmentu ili u zumu segmenta, a kontrola
posebno tona i slike u J i L verziji samo u zoom verziji"): desni klik na Wrap retku i u uvecanom slotu uzima
najblizi M marker cijeli (`right_click_marker` -> `select_marker`, kao prije 21. 10.). Samo slika ili samo ton reza
rade samo u uvecanom slotu: tamo drugi desni klik na vec uzeti marker iznad slike daje sliku
(`right_click_taken_marker` -> `select_mark_in`, `qnc-timeline` `TakeMarkerPicture`), iznad A2 njegov ton
(`right_click_marker_a2`, sada samo za uzeti marker; inace desni klik na A2 je zvuk pokrivalice kao prije);
na Wrap retku drugi desni klik ponovno uzima cijeli marker (`qnc-segment-panel` `cut_parts`). Ctrl+I/O i Ctrl+2
s uzetim markerom izvan uvecanog slota samo javljaju poruku `story.msg.cut_part_zoom_only`
(`qnc-program-segments` `take_cut_part`). Zlatna tablica misa: 1 redak promijenjen. Testovi, conformance.
Uzivo nije provjereno.
Popravak i dopuna 2026-10-22 (korisnik: "iz Storyja si izbacio dugmad za Talking Head, Voice over, B-roll i
Sync/B-roll"; "ako je uzet Ctrl+M cijeli marker je crven, ako je narednim Ctrl+ uzeta slika crven je samo marker
slike, a ako ton samo marker tona"): uvecani slot (od `d1a1506`) zamjenjivao je cijeli source dock pa i dugmad
zaglavlja. Sada se crta ispod zaglavlja na mjestu timelinea, dugmad i IN/OUT ostaju (`qnc-source-dock`
`show_timeline_dock_with`, `qnc-source-timeline` `show_with`/`Instead`, `qnc-segment-panel` `zoom_body`). Uzeti M
marker je crven: cijeli, samo dio slike (video) ili samo dio tona (A2) prema uzetom dijelu reza
(`qnc-timeline` `TimelineMarkerPin.part`, `qnc-program-view` `TakenPart`). Testovi, conformance (C10 ploce
Storyja ostaje 101). Uzivo nije provjereno.
Dopuna 2026-10-22 (korisnik: "pomaknuti crveni rez bi trajno trebao biti vidljiv"; "isto i za video koji je
samostalno pomaknut, ako je cijeli marker onda nije potrebno"; "isto treba vrijediti i za kanal A1"): uzeti marker
ostaje bijel na spremljenom mjestu, crveno je ono sto se mice. Spremljeni rez M markera ciji je ton A2 odmaknut od
slike (J ili L pokrivalice) crven je stalno: dio preko slike na rezu slike, dio preko A2 na rezu tona, spojeni na
rubu; rez koji nije razdvojen ostaje obican (`qnc-program-view` `MarkerPin.origin`/`sound_at`, `qnc-timeline`
`TimelineMarkerPin.origin`/`sound_at`). Isto za A1: J slike Ton segmenta stalno crveno, rez slike preko slike i rez
tona preko A1 (`SegmentsView.a1_splits`, `TimelineInput.a1_splits`, `qnc-program-segments` `picture_leads`).
Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-22 (korisnik: "kad je zoom pohranjen, ako nakon toga korisnik klikne na media pool zatvara se
zoom i prikazuje source"): kad tipkovnica prijede na media pool ili njegove tabove (klik, Tab, Enter), uvecani
slot se zatvara i source timeline je ponovno vidljiv (`qnc-panel-focus` `poll`, `SegmentCommand::CloseZoom`).
Uzeto u zoomu i dalje se odbacuje odlaskom s panela kao do sada. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-22 (korisnik: "kad se timeline prebaci u prikaz audio zoom ne prikazuju se markeri"; "bolje je
da ispod markera imas dva sloja prikaza, slike i zooma: prikazuje se ili filmstrip ili ton"): markeri su sloj iznad
prikaza; ispod njih je ili slika (filmstrip) ili zoom tona. Dok je zoom tona prikazan, markeri i razdvojeni rezovi
crtaju se jednom, iznad njega (`qnc-timeline` `lane_zoomed`). Vrijedi za source timeline i Wrap retke. Testovi,
conformance. Uzivo nije provjereno.
Popravak 2026-10-22 (korisnik: "na segmentu kad se klikne na A2 prikaze se prazan zoom tona"): Wrap redak dobiva
val A1/A2 vec izabranog kanala svakog segmenta i pokrivalice, a timeline ga je ponovno birao po broju kanala
(kanal 1 -> val A1 reda, kanali 3 i 4 -> prazno). Na retku s pojasevima (Wrap) traka i zoom crtaju svoj val;
nacrt kanala uzetog segmenta ili pokrivalice slaze val programa iz izabranog kanala prije crtanja
(`qnc-timeline` `lane_peaks`, `qnc-program-segments` `view_with_waves`). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-22 (korisnik: "zoom tona radi samo na prvom segmentu, treba raditi na svim segmentima"): zoom tona
i stupac kanala uzete trake bili su vezani uz oznake traka, koje ima samo prvi Wrap redak. Sada rade na svakom
retku na kojem je uzet zvuk segmenta ili pokrivalice (`qnc-timeline`). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-22 (korisnik: "i dalje samo prvi segment, a trebao bi sve Wrap segmente"): dok je uzet A1 ili A2
segmenta ili pokrivalice, zoom te trake prikazuje se na svim Wrap retcima; stupac kanala ostaje samo na retku
uzetog dijela (`qnc-segment-panel` `wave_over_video`). Conformance. Uzivo nije provjereno.
Dopuna 2026-10-22 (korisnik: "da bi ton bio funkcionalan trebamo i njemu dodati slot polje; klik na slot polje
jedne pokrivalice: glasnoca i ulazni kanal samo za tu pokrivalicu, ostali zadrzavaju svoju postavu"): dok je zoom
A2 otvoren, na dnu zooma svakog Wrap retka je traka svake pokrivalice (polje slota tona), uvijek vidljiva; uzeta
je uokvirena crveno. Lijevi klik na traku uzima ton te pokrivalice; prethodno uzeta se najprije sprema kao Enter
(`qnc-timeline` `zoom_strip_rows`/`paint_zoom_strips`, `qnc-program-segments` `take_part_sound`). Kanal i razina
vec su zapis svake pokrivalice (`a2_source_channel`, `a2_level_db`, `a2_off`); globalne postave A2 u prici nema
(nova pokrivalica uzima kanal i razinu sa source timelinea). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-22 (korisnik: "krivo, ovo je podesavanje glasnoce cijelog projekta, treba opcija samo za audio
odabranog slota"; log: `SetOutputLevels [100, 112..200]`): nacrt razine zvuka segmenta ili pokrivalice na Wrap
retku vise ne ide na razinu cijelog izlaza A1/A2 (to je mijenjalo cijeli program). Program se otvara s nacrtom te
jedne pokrivalice ili segmenta (`qnc-source-preview` `draft_output_level`), a val programa slaze se s nacrtom
kanala, razine i iskljucenosti samo tog dijela (`qnc-program-segments` `view_with_waves`; `qnc-timeline` na Wrap
retku ne skalira cijeli val). Zamjenjuje precac iz 16. 10. (razina izlaza do Entera). Svaki korak ponovno slaze
program (oko 0,5 s). Testovi, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-22 (korisnik: "da, otkljucaj i kreni s rjesenjem 1"; log: svaki korak kotacica
ponovno je pokretao player programa, ~1 s + 1,8 s, cuo se samo prvi ili zadnji): otkljucani `qnc-player-contract`,
`qnc-player-client`, `tools/qnc-player-runner`, `qnc-broadcast-engine` i, nuzno, `qnc-audio-output`. Nova naredba
playera `SetPartLevel { output, from_frame, to_frame, gain_db, off }`: jedan dio programa (raspon slika pokrivalice na
A2 ili segmenta na A1) cuje se `gain_db` glasnije od spremljenog ili utisan, odmah; zvucni uredaj ga primjenjuje u
povratnom pozivu samo na uzorke tog raspona i tog izlaza (`qnc-audio-output` `set_part_gain`, test), pa vrijedi i za
prerenderirane dijelove; `from == to` ga brise. `qnc-source-preview` `hear_part_level`; `qnc-segments-keys`: promjena
samo razine ili ukljucenosti uzetog zvuka ide tom naredbom (kontinuirano s kotacicem), promjena kanala i dalje otvara
program s nacrtom; Esc, odlazak s panela i drugo uzimanje je brisu. J i L oko pokrivalice ne prate nacrt do Entera.
Testovi, conformance. Uzivo nije provjereno. Player je ponovno zakljucan.
Popravak 2026-10-22 (korisnik: "stari projekt je zatvoren, novi otvoren, a u monitoru stoji slika iz starog
projekta"): Ingest je na promjenu projekta (i kad se sakrije) zatvarao player i brisao stanje playa i timelinea, ali
ne i sliku monitora u svom prikazu, pa je slika zatvorenog projekta ostajala. `qnc-ingest-application`
`stop_player` brise i monitor. Story/MA monitor cita stanje izravno iz previewa, koji ga pri promjeni projekta
brise. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-22 (korisnik: "nova kartica, nova kamera"; "ne vezati konfiguraciju s brojem kartice;
konfiguracija je vezana s kamerom"): Sony ILME-FX3A SD kartica (`PRIVATE/M4ROOT`, MEDIAPRO.XML MediaProfile 2.30,
CLIP/*.MP4 AVC50 4:2:2 s *M01.XML, SUB/*S03.MP4 HEVC proxy, THMBNL) nije dala nijedan klip iz dva razloga.
(1) Katalog kamera: uzorak `sony-m4root-sd` nije imao indeks ni Sony selektore, pa ga nijedan citac indeksa nije
slagao. Nova revizija `catalogs/camera-patterns/revisions/2026.10.22.1.json` -> `camera-patterns-2026.10.22.1.sqlite`
(alat `qnc-camera-catalog revise`; stare objave ostaju): MEDIAPRO.XML kao indeks i isti MediaProfile/NonRealTimeMeta
selektori kao XDROOT, korijen `PRIVATE/M4ROOT`. (2) Konfiguracija racunala `data/ingest-transport.json` imala je
izvor G: vezan uz serijski broj stare kartice, pa je nova kartica na istom slovu dala "source volume serial
changed". Upis stare kartice uklonjen (kopija prijasnje datoteke u scratchpadu), postavka racunala pokazuje na
novu reviziju kataloga. Kod nije mijenjan (pokusaj u `qnc-source-bindings` vracen). Provjera na izoliranoj kopiji
projekta `novi-test-5` (kartica samo citana, stvarni projekt nediran): Select 9 klipova s trajanjima iz XML-a,
bez probea, 2,1-2,3 s. Otvoreno (ceka odluku korisnika): izvori i URI medija u bazi i dalje nose serijski broj
volumena (`qnc://local/source/volume-<serijski>`), pa svaki disk koji sustav vidi postaje izvor po broju.
Popravak 2026-10-22 (korisnik: "trenutni zapis FX3A ne prepoznaje MP4 originalni video"; dvije kartice u istom
uvozu namjerno): zapisi FX3A originala nikad nisu postali konacni ("record-completion" sukob): vrijeme snimanja iz
XML-a kamere (`2026-10-19T16:56:10+02:00`) i iz ffprobea (`2026-10-19T14:56:10Z`) isti je trenutak u drugoj zoni,
a spajanje zapisa usporedivalo ih je kao tekst. `qnc-media-metadata-compose` `equivalent`: `creation_time` se
usporeduje kao trenutak (`instant_ms`), zapis kamere s njezinom zonom ostaje; test. `tools/qnc-ingest-worker`
gresku dovrsetka zapisa pise i u player log (`record-completion failed`), ne samo na nevidljivi stderr.
Provjera na izoliranoj kopiji (kartica samo citana, stvarni projekt nediran): svih 9 FX3A zapisa konacno,
9 valova, 9 filmstripova, 9 postera. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-23 (korisnik: "sada ingest dugme ne vodi na sljedecu formu"; "ti si pokvario u prethodnom koraku"):
nakon popravka od 22. 10. (FX3A zapisi postaju konacni) 9 FX3A zapisa bilo je `final` ali `partial`: proxy (HEVC Main 10)
nije imao `scan_mode`, jer ffprobe sazetak kaze `field_order=unknown`. Red uvoza takav zapis odbija (konacan, probe-an i
nepotpun), pa je Uvezi javio gresku i nije presao na Story. Sada isti jedini probe (`qnc-media-probe`) cita prve slike i
kad sazetak ne zna redoslijed polja (`summary_lacks_picture_facts`, polja slike `interlaced_frame`, `top_field_first`),
a `qnc-ffprobe-metadata` (`frame_scan`) uzima nacin snimanja iz prvih slika toka kad se sve slazu (inace ostaje
nenaveden). Granica prvih paketa 32 -> 128 (MP4 slaze zvuk u dugim nizovima: 32 paketa FX3A proxyja bila su samo zvuk).
Izolirana kopija (kartica samo citana, stvarni projekt nepromijenjen): cisti Select FX3A kartice i radnik daju 9 zapisa
`final/complete` (prije `partial`). Zapisi stvarnog projekta koji su vec konacni ostaju (jedan probe po mediju); novi
projekt ih daje ispravno. Testovi `qnc-ffprobe-metadata` (novi
`scan_unknown_in_the_summary_comes_from_the_first_pictures_when_they_agree`), `qnc-media-probe`, conformance.
Otkljucano i izvedeno 2026-10-23 (korisnik: "da, otkljucaj i napravi; ako je postavka projekta u koliziji sa stanjem
kartice, to treba dati u info box i ponuditi rjesenje s opcijom izbora"): uvoz provjerava samo ono sto projekt koristi.
Nova kocka `crates/qnc-import-fit` (+ ugovor; cisto pravilo nad spremljenim zapisom): original se uvijek koristi (zvuk i
export), proxy samo kad ga projekt pusta (`playback.input` proxy, ili proxy if available kad ga klip ima) ili kopira
(`storage.ingest_media` proxy); praznina u reprezentaciji koju projekt ne koristi ne zaustavlja uvoz; zapis kartice
(faza kamera) i dalje se dovrsava u pozadini. `qnc-content-store`: `QueueSelected { needs }`, red uvoza odbija samo klipove
koji ne odgovaraju projektu i imenuje ih. `qnc-ingest-selection-write`: prije upisa provjeri odabrane klipove; kad su neki
na putu nista ne pise i daje izbor (`start_import`, `choice`, `answer`; akcije kataloga `ingest_import_skip_conflicts` i
`ingest_import_cancel`): uvezi ostale ili odustani, uz objasnjenje i rjesenje u postavkama Projecta (Playback input ili
Ingest media). Nova pasivna kocka `crates/qnc-choice-box` (+ ugovor): info prozor u sredini ekrana, crta ga traka uvoza
(`qnc-ingest-import-bar`, polje `detail`). Provjera odabira premjestena iz Ingest aplikacije u pisca odabira (C10 Ingest
aplikacija 1363 -> 1354); ploca Ingesta ista (155). Testovi `qnc-import-fit` (stvarni FX3A zapis), `qnc-content-store`,
`qnc-ingest-selection-write`, `qnc-ingest-import-worker`, `qnc-ingest-select`, `qnc-program-db`, `qnc-ingest-application`,
conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-23 (korisnik: "za svaku kameru posebna kocka, kako bi mogli dodavati nove kamere bez
diranja koda; kocka koja cita podatke s kamere i usporeduje ih s json datotekama u kojima je zapisana procedura za tu
kameru; inicijalni podaci u tekstu, koje aplikacija moze procitati i zapisati trajno u bazu"; "da, otkljucaj i kreni"):
kamera je tekstualni profil, jedna JSON datoteka po modelu (`catalogs/cameras/<proizvodac>/<model>.json`, README u mapi):
id i verzija, proizvodac, model, format zapisa (`reader`), rasporedi kartice iz kataloga kamera, pravila prepoznavanja
(atribut elementa u zapisima kartice, `equals` ili `prefix`) i postupak (`declared_when_complete` ili `probe_once`).
Prvi profili: Sony ILME-FX6V, ILME-FX3A, ILCE-7M4, PMW-300 i zadani profil formata Sony MediaProfile (kartica cijeg modela
nijedan profil ne zna). Nova javna kocka `crates/qnc-camera-profiles` (+ ugovor): cita profile, ispravne trajno upisuje u
bazu kamera ovog racunala `data/qnc-cameras.db` (jedini pisac; samo nove ili promijenjene datoteke, po otisku teksta),
datoteka s greskom se ne uzima i ide u log (`camera-profile refused`), zadnji ispravni zapis ostaje; prepoznaje kameru iz
zapisa kartice. Nova javna kocka `crates/qnc-camera-profile-adapter` (+ ugovor): adapter jednog formata zapisa po
profilima; zapis svakog klipa dobiva profil kojim je procitan (`qnc.camera:profile` = `<id>@<verzija>`, lokator
`camera-profile:recognised` ili `camera-profile:default`), pa projekt zna kako je klip procitan i bez tog profila.
`qnc-camera-adapter`: sucelje formata `RecordFormat` (format zna format, ne kameru), `pattern_ids` vraca popis (rasporedi
iz profila). `qnc-sony-metadata`: `SonyMediaProfile` je format (citac indeksa, sidecari, poster, zapis), kamere u njemu
nema. `qnc-ingest-cameras::registry(root)` slaze formate s profilima. Uklonjen `qnc-camera-sony-fx6-v6` (+ ugovor).
Conformance: `qnc-camera-profiles` na popisu javnih DB vlasnika. Nova kamera poznatog formata je samo nova datoteka; novi
format je nova kocka. Provjera na izoliranoj kopiji (kartice D: FX6V CFexpress i G: FX3A samo citane, stvarni projekt
nepromijenjen): Select s novim putem daje 155 zapisa jednakih onima starog koda osim oznake profila (FX6V 146, FX3A 9,
svi prepoznati), pozadinski radnik ih dovrsava (155 konacnih i potpunih, oznaka ostaje). Testovi `qnc-camera-profiles`,
`qnc-camera-profile-adapter`, `qnc-ingest-cameras`, `qnc-ingest-select`, `qnc-ingest-import-worker`,
`qnc-record-completion`, conformance. Nije izvedeno: upozorenje u Ingestu za karticu nepoznatog modela (zasad samo u
zapisu klipa), PMW-300 nije proban (disk nije bio spojen).
Otkljucano i izvedeno 2026-10-23 (korisnik: "kocke bi mogle biti nesto kao dll, tako da aplikacija bude lakse
servisirana"; plan razina: podaci, dodaci kao zasebni programi s verzioniranim ugovorom, kocke u gradnji; "da, otkljucaj i
kreni s korakom 1"): formati kamera mogu biti dodaci. Ugovor `contracts/plugins/record-format.v1.json`
(`qnc.record-format` 1: JSON redovi na stdin/stdout; dogovor o ugovoru i verziji; pitanja `read_index`, `documents`,
`thumbnail`, `metadata`; dodatak dobiva samo tekst i reference, nikad putanju ni datoteku). Nova javna kocka
`crates/qnc-plugin-process` (+ ugovor): pokrene program dodatka uz aplikaciju, provjeri ugovor i verziju, jedan proces za
posao, vrijeme odgovora i velicina izlaza, zaustavi ga nakon 30 s bez posla; dodatak koji ne odgovara ili govori drugi
ugovor je kontrolirana greska, bez zamjenskog puta. Nova javna kocka `crates/qnc-record-format-plugin` (+ ugovor): dodatak
kao format zapisa aplikacije (`PluginFormat`) i strana koju program dodatka sluzi (`serve`). Novi program
`tools/qnc-format-sony`: format Sony MediaProfile kao dodatak. Katalog formata `catalogs/record-formats/catalog.json`
(ugraden ili dodatak; Sony ostaje ugraden); `qnc-ingest-cameras::registry` slaze formate iz kataloga s profilima kamera,
format koji se ne moze koristiti ide u log (`record-format refused`). Nova kamera poznatog formata je samo profil, novi
format je dodatak i redak u katalogu, bez nove gradnje aplikacije. Dokaz: testovi `qnc-format-sony` (dodatak odgovara isto
kao ugradeni format; drugi ugovor i nepostojeci program su odbijeni); izolirana kopija (kartice D: FX6V i G: FX3A samo
citane, stvarni projekt nepromijenjen) s katalogom prebacenim na dodatak: 155 zapisa jednakih ugradenom formatu; Select
D: 24,2 s kroz dodatak prema 16,3 s ugradeno (pretvaranje zapisa u JSON). Testovi ugovora `qnc-ingest-application` dobili
su isporucene kataloge u testni korijen. Conformance. Nije izvedeno: korak 2 (ugovor `qnc-packets` za sve sto dekoder radi,
alat za prihvat dekodera), korak 3 (popis verzija dodataka i programa).
Otkljucano i izvedeno 2026-10-23 (korisnik: "nakon klika na klip u media poolu sporo se pojavljuje playhead na source
timelineu; ne bi smjelo biti naknadnih probea, niti ponovnog pokretanja Broadcast Playera"; "da, otkljucaj i kreni"):
log: probea nakon Selecta nema, ali klijent playera je pri svakom kliku gasio proces playera i pokretao novi (pokretanje
0,6-1,1 s do prve pozicije, spreman 1,5-2,2 s). Sada jedan proces playera sluzi sesiju za sesijom:
`tools/qnc-player-runner` ima nacin `--monitor-host` (svaki redak na stdin je pokretanje jedne sesije; sesija se otvara i
radi kao do sada, a kad zavrsi (Shutdown, novije pokretanje, kraj stdin) sve njezino se zatvori prije sljedece i javi se
`session_ended`; proces zavrsava sa stdin). `qnc-player-client` (`HostProcess`) drzi taj proces kroz izbore klipova:
stara sesija se zatvara u procesu prije nove (proces koji to ne javi u 2 s se gasi), svaka sesija i dalje ima svoj
identitet, tokene, upravljacku vezu i frame mapu; proces koji nije otvorio sesiju se ne koristi za sljedecu. Pravilo 8.2
(klik na drugi klip prekida staru sesiju i zvuk, nista od nje ne prelazi u novu) ostaje. Izmjereno na izoliranoj kopiji
(kartice samo citane): prva sesija kao prije (pokretanje procesa), svaka sljedeca do prve pozicije playheada 0,22-0,31 s
(prije 0,6-1,1 s), spreman 0,64-0,88 s (prije 1,5-2,2 s); playback klipova PASS (Mironik 3401 95,7 %, rupa 148 ms).
Nije izvedeno: pretvarac slike na grafickoj (`qnc-gpu-raster`, ~0,1 s po sesiji) se jos otvara za svaku sesiju (kocka
nije otkljucana). Testovi `qnc-player-client`, `qnc-player-runner`, conformance. Uzivo kroz sucelje nije provjereno.
Otkljucano i izvedeno 2026-10-23 (korisnik: "da, otkljucaj qnc-gpu-raster i napravi"): `qnc-gpu-raster` drzi jedan
graficki uredaj i jedan prevedeni raster pipeline po procesu (`shared_device`); svaki pretvarac i dalje ima svoje
spremnike, parametre i veze. Uredaj koji javi gresku ili se izgubi ne zadrzava se, sljedeci pretvarac otvara novi; jedan
pretvarac se priprema u isto vrijeme (stog gresaka uredaja je jedan). Izmjereno na izoliranoj kopiji (isti proces playera,
promjena klipa): do prve pozicije 0,20-0,24 s, spreman 0,61-0,72 s; pretvarac se vise ne ceka (priprema traje koliko i
zvucni uredaj, ~85 ms). Mironik 3401 (MXF s kartice D:) jednom je otvaranje trajalo 2,15 s i A/V na pocetku do 2,8 slike;
to je citanje kartice, ne pretvarac. Testovi `qnc-gpu-raster` (i oni koji traze GPU), conformance.
Popravak 2026-10-23 (korisnik: "izgubili smo funkciju b-roll... provjeri kako je radio juce, kako danas"): kod B-rolla
nije mijenjan od 22. 10.; log projekta test-kocka1 pokazuje 9 pokrivalica danas napravljenih, a dugme B-roll tiho ne
radi nista kad je izabran slot koji vec ima B-roll. Sync na Enter zapisuje izbor svog slota u bazu (`SelectSlot`),
klik koji samo postavlja playhead pustao je izbor samo u memoriji, pa ga je sljedece citanje price vratilo, a nakon
Synca playhead stoji u sljedecem (praznom) slotu. Sada: klik koji napusta izabrani slot brise i izbor u bazi
(`qnc-program-segments` Cue); B, P i dugme B-roll uzimaju prazni slot pod Wrap playheadom kad izabrani slot vec ima
B-roll (`qnc-program-view` `quick_cover_slot`, test); dugme B-roll s oznacenim izvorom uvijek kaze zasto nije stavilo
pokrivalicu. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-23 (korisnik: "sub-clip nije kreirao poster subclipa"; "to treba biti kocka pa se nece gubiti kad se nesto
drugo popravlja"): SubClip spremljen s IN-om i zadanim OUT-om (kraj klipa, pravilo 17. 10.) nije dobio poster, jer su
slike IN/OUT bile samo one koje je monitor pokazao na tim mjestima, a kraj klipa monitor nikad nije pokazao. Kocka
`qnc-shot-stills` (`ShotStills`) sada je jedino mjesto slika spremljenog kadra: pamti slike monitora (`follow`), a za
svaku sliku shorta (IN, OUT) i poster pokrivalice uzima sliku monitora tog framea, inace taj frame dekodira iz medija koji
projekt pusta kroz katalog dekodera (`qnc-still-locator`, isto kao zoom reza; bez probea), u pozadinskoj niti; zapis kroz
`qnc-virtual-shots` kao prije, greska se upisuje kao razlog i u player log (`shot-stills`); nakon zapisa pool ponovno cita
kadrove. `qnc-virtual-short-stills`: `captured`, `write_still_image`, `read_jpeg`. Ugovor `shot-stills.module.json`
(dekodiranje kroz katalog dopusteno). C10 Story aplikacija 593 -> 588. Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-23 (korisnik: "svaki klip je virtualan, svaki ima globalni IN i OUT (pocetak i kraj); dodavanjem novog
IN-a ili IN-a i OUT-a kreiramo subklip, neki zavrse kao segmenti, neki kao SubClip u SubClip tabu, a neki u B-roll"):
SubClip bez vlastitog OUT-a ide do kraja klipa (`qnc-timeline` `visible_source_marks`, vec tako). Taj kraj je granica iza
zadnje slike, pa kocka `qnc-shot-stills` za OUT sliku na njoj uzima zadnju sliku klipa; frame bez slike javlja
`stills.past_clip_end`. Test, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "ingest treba razbiti na kocke"; "da, otkljucaj i kreni s korakom 1"; plan u 6 koraka:
1 projekt, 2 preview, 3 Select, 4 uvoz, 5 posteri, 6 Ingest SubClip `qnc-ingest-parts`): korak 1. Nova javna kocka
`crates/qnc-ingest-project` (+ ugovor `contracts/modules/ingest-project.module.json`): projekt na kojem Ingest radi -
aktivni projekt iz baze, ucitavanje radnih postavki i kataloga izvan niti sucelja (`CatalogLoader`), ciljevi kataloga i
Selecta u bazi projekta, statistika kataloga, mapa projekta i slijed aplikacija; stanje ucitavanja (`ProjectStatus`),
`start` (provjera prikazanog projekta, odbija promjenu dok Select ili uvoz pisu), `poll` (kaze je li projekt drugi),
`fail` (nista od prethodnog projekta ne ostaje). Nista nije izvor istine: svako ucitavanje ponovno cita bazu.
`qnc-ingest-application` drzi samo `project` umjesto sedam polja i zove kocku (`settings.rs`, `state.rs`, `navigation.rs`,
`thumbnails.rs`, `playback.rs`, `dir_confirm.rs`, `import_selected.rs`); ponasanje isto. C10 Ingest aplikacija 1354 -> 1265.
Testovi `qnc-ingest-project` (2), `qnc-ingest-application` (31), conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 2"; Ingest u kocke): korak 2. Kocka `qnc-ingest-preview-source` je panel
previewa Ingesta: drzi i jedini source timeline (`qnc-source-timeline`), izbor klipa (`choose`: stara sesija se prekida
prije nove i kad novi klip nije spremljen ili projekt nije ucitan; isti klip koji se priprema ili je spreman samo ponovno
cita artefakte; `Chosen`), ono sto forma pokazuje od previewa i source timelinea (`shown`: IN/OUT ostaju, klip bez
filmstripa pokazuje poster u traci) i tipke i klik source timelinea (`route`, `route_input`; bez playera samo Esc).
Ugovor modula: nove sposobnosti, `session_local`, uklonjena zabrana `player.command` (tipke source timelinea idu
univerzalnom previewu, kao i prije kroz aplikaciju). `qnc-ingest-application` samo poziva kocku; tri preostala hrvatska
teksta u kodu idu kroz rjecnik (`ingest.msg.clip_not_found`, `ingest.msg.clip_not_saved`,
`player.broadcast_player_connected`, novi `ingest.msg.invalid_frame`). C10 Ingest aplikacija 1265 -> 1235. Testovi
`qnc-ingest-preview-source` (1, novi), `qnc-ingest-application` (31; test istog klipa sada ide kroz kocku),
conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 3"; Ingest u kocke): korak 3. Nova javna kocka
`crates/qnc-ingest-select-run` (+ ugovor `contracts/modules/ingest-select-run.module.json`): jedan Select izvora u Ingestu -
zapis odabranog izvora (`source_record`: naziv medija, inace mapa koju preglednik pokazuje, inace URI), Select sesija s
kamerama ovog racunala (`SelectRun`: `with_cameras`, `start`, `is_busy`, `cancel`) i njezini dogadaji primijenjeni na popis
klipova (`poll` daje `Polled`: poruka, broj upozorenja, uklonjeni klipovi, je li uklonjen klip na previewu, kraj Selecta).
Sam rad ostaje `qnc-ingest-select`, pravila popisa `qnc-ingest-clip-list`; zapis u bazu ide kroz javni pisac Selecta.
`qnc-ingest-application` drzi samo `select` umjesto tri polja i poziva kocku. C10 Ingest aplikacija 1235 -> 1204. Testovi
`qnc-ingest-select-run` (2), `qnc-ingest-application` (31), conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 4"; Ingest u kocke): korak 4. Nova javna kocka
`crates/qnc-ingest-import-run` (+ ugovor `contracts/modules/ingest-import-run.module.json`): Uvezi u Ingestu - oznaceni
klipovi se provjere prema onome sto projekt koristi i jednom upisu u bazu (`qnc-ingest-selection-write`, izvan niti
sucelja; klipovi na putu daju izbor), zatim se budi pozadinska aplikacija (`ImportRun`: `start`, `answer`, `wake`, `poll` daje
`Written`: predano radniku, radnik se nije dao probuditi, ili nije upisano uz izbor). Kocka ne kopira medij (to radi radnik).
`qnc-ingest-application` drzi samo `import` umjesto pisca odabira i pokretaca radnika; `worker_launch.rs` uklonjen (testovi
zadaju svoje budenje kroz `with_wake`); ovisnosti `qnc-import-fit` i `qnc-ingest-import-worker` vise nisu u aplikaciji.
C10 Ingest aplikacija 1204 -> 1162. Testovi `qnc-ingest-import-run` (1), `qnc-ingest-application` (31), conformance,
release build. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 5"; Ingest u kocke): korak 5. Posteri vec imaju svoju kocku
`qnc-clip-posters`; ono sto su Ingest i Story/MA aplikacija svaka pisala sama (popis postera koji nedostaju i stavljanje
stiglih slika na klipove) sada je u njoj jednom: `request_missing` (klipovi s adresom postera i bez slike, odabrani prvi) i
`poll_into` (stigle slike na svoje klipove; one bez klipa, npr. shortovi u Storyju, vracaju se pozivatelju). Ugovor modula:
nove sposobnosti. Ingest i Story/MA aplikacija samo pozivaju kocku. C10 Ingest aplikacija 1162 -> 1142, Story/MA aplikacija
588 -> 573. Testovi `qnc-clip-posters` (8, novi test), `qnc-ingest-application` (31), `qnc-editorial-application` (11),
conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 6"; "da, zasebna tablica, i B"; Ingest u kocke): korak 6a. Nacin B:
aplikacijski sloj forme prolazi kroz svoje kocke, nova mogucnost je nova kocka, ne novi redak aplikacije. Nova javna kocka
`crates/qnc-form-pieces` (+ ugovor): sucelje `Piece` (radnja s id-em iz kataloga i njezin klip, poll, redovi poola za tab
kocke) i `Pieces` (prva kocka koja uzme radnju odgovara); kocka vidi samo popis klipova, odabrani tab, filtar Novi, klip na
previewu, IN/OUT source timelinea i bazu projekta. Nova kocka `crates/qnc-pool-marks` (+ ugovor): kvacica, sve, nista i
odabrani tab poola (iz Ingest aplikacije premjesteni `toggle_clip.rs`, `select_all.rs`, `clear_selection.rs` i krak taba).
Nova kocka `crates/qnc-ingest-pieces` (+ ugovor): samo popis kocki Ingesta s id-evima akcija. Ingest SubClip: nova kocka
`crates/qnc-ingest-parts` (+ ugovor modula i baze `contracts/databases/ingest-parts.database.json`), zasebna tablica
`ingest_parts` (odluka korisnika: ne `virtual_shots`, jer SaveShort prima samo uvezene klipove i Story bi ih pokazao kao
SubClip) kroz posrednika baze: dio je IN..OUT klipa iz kataloga, unutar spremljene duljine (`qnc-content-read`), id
`<klip>-part-NN` se nikad ne ponavlja; `PartsPiece`: Shift+S i dugme Add SubClip spremaju IN/OUT klipa na previewu i otvaraju
tab SubClip, tab pokazuje dijelove kao kartice (poster njihova klipa), klik na dio otvara njegov klip s IN/OUT (pending marks
u `qnc-ingest-preview-source`), Delete brise odabrani dio. Katalog (QNC preset, Ingest): `save_virtual_shot` Shift+S, nova
akcija `ingest_subclip_delete` Delete; layout Ingesta: dugme Add SubClip u traci source timelinea i u redu fokusa. Conformance:
`qnc-ingest-parts` na popisu javnih DB vlasnika. C10 Ingest aplikacija 1142 -> 1137 (s novom mogucnoscu), ploca 155 ista.
Testovi `qnc-form-pieces` (1), `qnc-pool-marks` (1), `qnc-ingest-parts` (2), `qnc-ingest-application` (31; test kroz cijelu
aplikaciju: Shift+S, tab SubClip, klik otvara klip), conformance, release build. Uzivo nije provjereno. Nije izvedeno (korak
6b): Uvezi jos kopira cijeli klip; kopija samo dijelova (stream copy, 1 s rezerve, svaki dio svoj klip) je sljedeci korak.
Izvedeno 2026-10-23 (korisnik: "da, kreni s korakom 6b"; "ne mov, mora biti kao original"; "treba i poster"; "uzmi
timekodove intra slicica"; "B, kreni s qnc-mxf-cut"): korak 6b, Uvezi za klip s dijelovima. Mjereno: ffmpeg stream copy u
MXF za XAVC Long GOP pise krive vremenske oznake (obican citac gubi trecinu slika), u MOV/MP4 s edit listom broj slika ne
odgovara stvarnom. Zato: nova javna kocka `crates/qnc-mxf-cut` (+ ugovor): dio frame-wrapped MXF originala kao MXF iste vrste
iz bajtova originala - zaglavlje s metapodacima (trajanja i pocetni timecode dijela), novi indeks (temporal/key frame offset,
slice offset), paketi od intra slike na ili prije IN-a (vodece slike te grupe izostavljene; zvuk, sistemski element i ANC
svake slike iz paketa njezina mjesta u vremenu), footer i RIP; nista se ne dekodira. Nova javna kocka `crates/qnc-mp4-cut`
(+ ugovor): MP4/QuickTime stream copy ffmpegom iz kataloga dekodera od intra slike, vodece slike i paketi od iduce intra
slike izbaceni (`noise` bsf), zvuk zavrsava sa slikom, timecode prve slike upisan. `qnc-ingest-import-worker` (`parts.rs`):
klip s dijelovima se ne uvozi; svaki dio (1 s rezerve) se reze u `original/` projekta, opisuje jednim probeom nove datoteke
(demuxer po vrsti datoteke), postaje svoj uvezeni klip bez proxyja, s posterom sa svog IN-a (`qnc-poster-create`).
`qnc-content-store`: operacija `FinishWithParts` (dijelovi upisani i uvezeni, klip vracen u neuvezen i bez stare adrese
medija; sve ili nista). `qnc-ingest-parts::read_parts`; red uvoza cita dijelove (`TransportQueue::with_parts`, `RunQueue`).
`qnc-content-artifacts`: mapa projekta je izvor za filmstrip i val (kao u previewu). `tools/qnc-ingest-worker`: nakon uvoza
koji je nesto uvezao jos jedan prolaz filmstripa i vala. Provjereno (kartice D: i G: samo citane, izolirana kopija):
izrez MXF 3396 (IN 450..950): 526 slika, sve tocno 434..959 originala, bez upozorenja, zvuk uzorak po uzorak isti (kanali
1 i 4), timecode 06:14:58:00; izrez MP4 C0013: 168 slika tocno, zvuk tocno od uzorka 144x960; jedan rad radnika: 3 dijela
uvezena s posterom, filmstripom i valom za 8 s; player pusta dijelove kao original (isti kratki trzaj na pocetku ima i
original od slike 0). Testovi `qnc-mxf-cut` (3, i na pravom originalu), `qnc-mp4-cut` (2), `qnc-content-store` (31),
`qnc-ingest-parts` (2), `qnc-ingest-import-worker` (22), conformance, release build. Nije izvedeno: Sonyjev privatni XML
unutar MXF zaglavlja (NRT) zadrzava trajanje cijelog klipa; dio klipa na LAN/Intranet izvoru (rez trazi lokalni original);
uzivo kroz sucelje nije provjereno.
Promjena i izvedba 2026-10-23 (korisnik: "od izrezanih djelova kreiraj karticu sa XML datotekama kao na originalnom disku, s
originalnim nazivom + n broj, ista procedura uvoza kao s originala"; "da, sva cetiri"; "novi SubClip dir, jer kod kopiranja
originala bila bi kolizija"; "SubClipovi automatski cekirani, korisnik ih moze iskljuciti"; "ne Windows oznake"; otkljucan
Project samo za lokaciju): zamjenjuje izravni upis dijelova u katalog (`FinishWithParts`, uklonjen). Postavke projekta:
`products.subclips` = `SubClip` (`qnc-project-store` PRODUCT_DIRECTORY_FIELDS, novi projekt dobiva mapu, stari pri otvaranju
u Projectu; `qnc-work-settings` `subclips_location`). Uvezi: svaki oznaceni neizrezani dio (`qnc-ingest-parts`: stupci
`cut_name`, `selected`, novi dio oznacen; operacije MarkCut, Select; `read_uncut`, `PartsWriter`; razvojna tablica bez tih
stupaca se brise) reze se na karticu dijelova `SubClip/<oznaka medija>_<serijski broj>/` u rasporedu originalne kartice,
naziv original + broj (`Mironik 3396_01.MXF`); Sony kartica dobiva dokumente kroz novu kocku `crates/qnc-sony-card-write`
(+ ugovor): stavka MEDIAPRO.XML (datoteka, trajanje, UMID s brojem instance dijela, bez proxyja), M01.XML (UMID-ovi,
Duration, LtcChangeTable od prve slike, frameCount tablica unutar dijela) i poster sa svog IN-a (Thmbnl/T01). Zatim radnik
napravi Select te kartice istim citacem kamere kao za pravu karticu i stavi njezine klipove u red; medij koji je vec u mapi
projekta uvoz samo povezuje (`action_for`). Klip s dijelovima se ne uvozi cijeli (`qnc-content-store` FinishAsParts).
`qnc-ingest-select` `SelectionConfig::for_project` (mapa projekta kao izvor; koriste ga radnik, uvoz, dovrsetak zapisa i
izbornik diskova Ingesta: ulaz "Project <naziv> (SubClip)"); `qnc-content-read` `not_imported_named`. Tab SubClip: kvacica
dijela (PartsPiece prije PoolMarks), izrezani dio je svoj uvezeni klip (poster s IN-a); Uvezi je dostupan i kad su oznaceni
samo dijelovi (`qnc-form-pieces` `Shown` s `ready`, `ImportRun::start` budi pozadinu). Radnik reze dijelove i kad u redu
nema klipova (`parts_waiting`). Provjereno na izoliranoj kopiji (kartice D: FX6 i G: FX3A samo citane): kartice
`card_a8155186/XDROOT` i `card_30616164/PRIVATE/M4ROOT`, Select procita trajanja iz XML-a (526, 264), dijelovi uvezeni s
posterom, filmstripom i valom u jednom radu (12 s), neoznaceni dio nije izrezan. Testovi `qnc-sony-card-write` (3),
`qnc-ingest-parts`, `qnc-content-store`, `qnc-ingest-import-worker`, `qnc-ingest-application`, conformance, release build.
Uzivo kroz sucelje nije provjereno.
Popravak 2026-10-23 (korisnik testirao: "na krivom mjestu prikaz"; log: "ingest-part cut failed ... Access is denied"): rez dijela
pisao je u privremenu datoteku i preimenovao je, a zasticena mapa projekta (ACL deny delete) ne dopusta preimenovanje ni
brisanje. `qnc-mxf-cut` pise dio odmah pod konacnim imenom, a pomocnu datoteku paketa drzi u privremenoj mapi sustava;
`qnc-mp4-cut` pise ffmpegom odmah u konacnu datoteku. Ponovni pokusaj prepisuje datoteku. Provjereno na izoliranoj kopiji
s istom zastitom (icacls deny DE,DC): oba dijela izrezana i uvezena. Kartica dijela u tabu SubClip pokazivala je poster
cijelog klipa (pocetak klipa) i krivo trajanje (redak klipa u Ingestu nema broj slika): `qnc-ingest-parts` racuna trajanje
iz spremljene brzine klipa (`qnc-content-read::clip_timing_on`), a slika kartice neizrezanog dijela je slika s njegova IN-a,
dekodirana u pozadini kao slike SubClipa u Storyju (`qnc-shot-stills::decode_still`; `qnc-form-pieces` `Now.stills`).
Testovi, conformance, release build. Uzivo nije provjereno.
Napusteno 2026-10-23 (korisnik: "ne cini mi se ovo elegantnim rjesenjem... napustio bi za sada ovu mogucnost SubClipa,
iskljucio bi ga i iz tab prikaza u ingestu"): Ingest SubClip (dijelovi klipa koji se uvoze sami) je uklonjen iz aplikacije.
Cijeli rad ostaje pod lokalnom git oznakom `subclip-parts-2026-10-23` (commit ec6f86c: `qnc-ingest-parts`, `qnc-mxf-cut`,
`qnc-mp4-cut`, `qnc-sony-card-write`, kartica dijelova u `SubClip/`, `products.subclips`) i moze se vratiti. Vraceni su
commitovi 0da0e6d, 532c651 i ec6f86c (njihovi zapisi u AGENTS.md ostaju kao povijest); iz koraka 6a uklonjeni su kocka
`qnc-ingest-parts` i njezini ugovori, tab SubClip u Ingestu (u `hidden_tabs` rasporeda), dugme Add SubClip, Ingest prečaci
Shift+S, Delete i Alt+S te akcije `ingest_tab_virtual` i `ingest_subclip_delete`. Ostaje opceniti prolaz forme kroz kocke
(`qnc-form-pieces`, `qnc-pool-marks`, `qnc-ingest-pieces`) i koraci 1-5 podjele Ingesta. Projekti koji su u
`products` vec dobili `subclips` i mapu `SubClip` zadrzavaju ih (podatak se ne cita). C10 Ingest aplikacija 1137 -> 1133.
Testovi, conformance.
Audit i podjela 2026-10-23 (korisnik: "ingest moras napraviti audit i podjeliti na kocke, bez razloga drzis kod u jednoj
datoteci"): cisto premjestanje bez promjene ponasanja; svaka datoteka jedna odgovornost. `qnc-ingest-catalog` (rows,
read, loader, folder, tests), `qnc-ingest-clip-list` (events, row, tests), `qnc-ingest-selection-write` (conflicts,
write, tests), `qnc-ingest-preview-source` (content, panel, tests), `qnc-ingest-layout` (contracts: ucitavanje i
provjera ugovora; model: modeli rasporeda), `qnc-ingest-import-worker` (plan: sto i kamo; copy: kopija medija i
postera; run: preuzimanje iz reda; queue: red uvoza kroz pisca), `qnc-ingest-select` (types, session, run),
`qnc-ingest-store` (records, store, validate, ids, tests), `qnc-ingest-application` (testovi iz lib.rs u
`app_tests.rs`, testovi postavki po temama u `work_settings_tests/`: catalog, settings, source). Uklonjeni ostaci
SubClipa (`Answer.open`, IN/OUT oznake u preview-sourceu). Broj testova svake kocke isti kao prije (application 31,
import-worker 22, select 18, store 3, catalog 3, selection-write 3, clip-list 2, preview-source 1). C10 Ingest
aplikacija 1133 -> 1129. `selection_config.rs` (306 redaka, model postavki izvora) ostaje jedna datoteka. Conformance.
Audit i podjela 2026-10-23 (korisnik: "napravi isti audit i podjelu za Story"): cisto premjestanje bez promjene
ponasanja; svaka datoteka jedna odgovornost. `qnc-program-db`: `store.rs` (2896 redaka) u `store/` po radnjama
(segments, markers, reads, sound, covers, cover_split, schema, rules, spans, slots, order, checks), model zahtjeva i
zapisa u `model.rs`, testovi po temama u `tests/` (segment, marker, slot, cover, sound, store). `qnc-program-segments`:
`lib.rs` 1379 -> 418 (story_read, commands, select, create, writes, refresh). `qnc-panel-focus`: panel, route, walk,
tests. `qnc-program-playlist`: build, overlay, window. `qnc-program-input`: load, build, media. `qnc-trim-stills`:
worker, tests. `qnc-segment-panel`: header, program, tests. `qnc-program-view`: program, testovi. `qnc-virtual-shots`:
`store/` schema, rows. `qnc-sync-cover`: testovi u svoje datoteke. `qnc-editorial-application`: samo testovi izvan
`lib.rs`; podjela radnog koda bila bi digla C10 (573 -> 584, zaglavlja novih datoteka), pa je vracena. Broj testova
svake kocke isti kao prije (program-db 40, program-segments 53, playlist 17, editorial-application 11, sync-cover 10,
virtual-shots 8, panel-focus 7 i zlatna tablica tipki, program-input 6, trim-stills 4, program-view 4,
segment-panel 2). Conformance. Debug gradnja cijelog workspacea nije prosla zbog punog diska (povezivac).
Otkljucano i izvedeno 2026-10-23 (korisnik: "da, obrisi debug cache i kreni s korakom 2"; korak 2 plana dodataka: dekoder
kao dodatak): nalaz: dekoder kao zaseban program (`qnc_packets_v1`, docs/70) pokrivao je samo reprodukciju; svih sest
ostalih poslova dekodera (slike filmstripa, niz slika reza, kljucne slike, zvuk za val, vec procitan zvuk i slike) zvalo je
ffmpeg naredbu izravno, pa bi drugi dekoder pokvario filmstrip, val i zoom reza. Izvedeno: ugovor
`contracts/plugins/decoder-jobs.v1.json` (`qnc.decoder-jobs` 1), nova javna kocka `crates/qnc-decoder-jobs` (+ ugovor
modula; strana aplikacije s vise procesa za istodobne poslove i strana programa), `qnc-plugin-process` dobio odgovor u
dijelovima i prekid posla (`call_parts`, `serve_parts`, `CANCEL`; postojeci `call`/`serve` isti, dodatak formata kamera
nepromijenjen), referentni program `tools/qnc-decoder-ffmpeg` (isti ffmpeg kod kao dosad), `qnc-decoder-catalog`: unos
`"jobs": { "program": ... }` salje sve te poslove tom programu (bez njega kao prije; program koji nije uz aplikaciju je
kontrolirana greska, bez zamjene), zadani `qnc.ffmpeg` u `catalogs/decoders/catalog.json` ga ima. Alat
`tools/qnc-decoder-acceptance`: svaki posao ugradeno i kroz program na mediju samo za citanje (slike bajt po bajt, kljucne
slike, zvuk bit po bit), prekid, drugi ugovor odbijen. Brojevi u ugovoru tocni (`serde_json` `float_roundtrip`; bez toga se
vremena kljucnih slika razlikuju u zadnjem bitu). Conformance: granica playera dopusta `qnc-decoder-jobs` i
`qnc-plugin-process` katalogu i programu playera (nova granica koju korak trazi). Provjereno: prihvat FX6 MXF (D:) i FX3A
MP4 (G:) ACCEPTED, release: zvuk 27 s / 4 kanala 205 ms ugradeno, 245 ms kroz program, slike isto; zivi testovi
`qnc-trim-stills` i `qnc-keyframe-nav` kroz instalirani katalog; testovi kocki koje koriste te poslove. Nije izvedeno:
korak 3 (popis verzija dodataka i programa); reprodukcija i dalje ide kroz ugradeni put (`qnc_packets_v1` postoji, program
za nju nije napisan). Debug cache obrisan uz dozvolu korisnika (50,9 GB).
Izvedeno 2026-10-24 (korisnik: "nastavi s tockom 3"; plan dodataka, korak 3: popis verzija): nova javna kocka
`crates/qnc-installed-versions` i alat `tools/qnc-versions` (`--json`): svaki dodatak iz kataloga (format kamera, program
poslova dekodera) upitan tko je (id, verzija, ugovor) i prihvaca li ga aplikacija; svaki dekoder kataloga dekodera s izvrsnom
datotekom i njezinom verzijom (`-version` za ffmpeg obitelj) i ulogom (selected/preferred) te programom poslova; programi
`qnc-*` uz aplikaciju (velicina, datum). Nista ne mijenja i nista ne bira. Citac kataloga formata premjesten (ne kopiran) iz
`qnc-ingest-cameras` u `qnc-record-format-plugin` (`FormatCatalog`, `FORMAT_CATALOG`); `qnc-decoder-catalog`
`installed_catalog` i `Registration::executable_path` su javni. Nije izvedeno: obicni programi aplikacije ne govore svoju
verziju (prikazani su velicinom i datumom); verzija vrijedi za dodatke i dekodere.
Otkljucano i izvedeno 2026-10-24 (korisnik: "trebali bi u projekt postavke uvesti Lead-out / Tail vrijeme (default) koje korisnik
moze promjeniti..., takoder i nivo normalizacije tona na A1 i A2 prema EBU standardu (default) s mogucnosti promjene"; odluke:
tail 2 s; A1 i A2 zasebno; u previewu ukljucivanje i iskljucivanje precacem; export slijedi razine koje je korisnik namjestio
ako se uklapaju u pravilo): Project otkljucan samo za te dvije postavke. `seed/system_seed.json` (svi sistemski predlosci):
`export.tail_seconds` = 2, `audio.loudness` = { a1_lufs -23, a2_lufs -23, true_peak_dbtp -1, tolerance_lu 0.5 } (EBU R 128);
`qnc-project-store` dopunjuje ih starijem projektu pri otvaranju (test); `contracts/ui/project.layout.json` Advanced: blok
"Loudness (EBU R 128)" (cilj A1, cilj A2, vrh, tolerancija) i "Lead-out / tail (s)" u bloku Export, izbori u ugovoru,
natpisi u rjecniku; `qnc-work-settings` `audio_loudness()` (`Loudness`) i `export_tail_seconds()` citaju strogo (test).
Project je ponovno zamrznut. Nije izvedeno (ceka): mjerenje glasnoce, prekidac normalizacije u previewu, primjena u exportu
(export jos ne postoji).
Otkljucano i izvedeno 2026-10-24 (korisnik: "lead/out treba se automatski dodati na pocetku prvog segmenta i na kraju
zadnjeg segmenta prilikom montaze"; "ako je prvi segment off, dodaje se crno i tisina, ako je ton, dodaje se samo slika";
"pokrivalica pocinje od pocetka lead in i pokriva lead in i pocetak offa"; lead-in je zasebna postavka; "tretiraj ih kao
veoma kratke segmente"; v5 to nema): Project otkljucan samo za postavku `export.lead_in_seconds` (seed svih sistemskih
predlozaka 2 s, `qnc-project-store` je dopunjuje starijem projektu pri otvaranju, Advanced polje "Lead-in (s)" uz
"Lead-out / tail (s)", rjecnik; `qnc-work-settings` `export_lead_in_seconds` strogo). `qnc-program-db`: lead-in i
lead-out su redovi `story_parts` s oznakom `lead` (`in`/`out`, stupac dodan postojecim pricama), koje vlasnik baze nakon
svake izmjene price slaze iz prvog i zadnjeg segmenta i postavki projekta (`store/leads.rs` `sync_leads`, prvi korak
`finalize_story`; postavke cita iz `public_project_settings`, nedostaju li - kontrolirana greska i nista se ne upisuje):
uz Off crno i tisina (Off s A1 iskljucenim), uz Ton slika njegova klipa prije IN-a odnosno iza OUT-a bez tona (Ton s A1
iskljucenim); koliko klip nema slika, ostatak je crn. Redovi se prepisuju samo kad se razlikuju; promjena duljine
lead-ina pomice korisnicke markere za razliku. Pocetni marker je na pocetku lead-ina, zavrsni na kraju lead-outa, pa prvi
i zadnji slot (i pokrivalica u njima, uz pravilo "bez rupe") pokrivaju lead. Brisanje, ukljucivanje, zamjena,
skracivanje, premjestanje, zvuk i J slika leada se odbijaju (`story.lead_is_automatic`); redoslijed segmenata, novi
segment i premjestanje ne vide leadove. `public_story_parts` i `ProgramSegment` nose `lead`, `sort_index` je mjesto u
prici. `qnc-program-view`: lead se ne prikazuje u tabu Segment, Wrap redovi ga imaju. Player, playlista i prerender
nisu mijenjani (lead je obican segment). Testovi `qnc-program-db` (4 nova), `qnc-program-view`, `qnc-program-segments`,
`qnc-program-input`, `qnc-work-settings`, `qnc-project-store`, `qnc-editorial-application`, conformance, release build.
Uzivo nije provjereno. Aktivni projekt dobiva postavku lead-ina tek kad se otvori u Projectu; do tada se prica ne mijenja.
Project je ponovno zamrznut.
Izmjena prikaza 2026-10-24 (korisnik: "lead i out segmenti trebaju se ljepiti na prvi korisnicki segment i na zadnji
korisnicki segment, a ne biti samostalni segmenti"): Wrap redovi panela Segmenti su redovi korisnickih segmenata; prvi
red pocinje s lead-inom, zadnji zavrsava s lead-outom (`qnc-program-view` `SegmentsView::wrap_rows`, `SegmentRow.lead`;
`qnc-segment-panel` crta red po prozoru iz `wrap_rows`, lead ima oznaku "Lead"). Slika ili zvuk na lead-inu pripadaju
prvom segmentu, na lead-outu zadnjem (`user_segment_at`: Ctrl+Shift+I/O, Ctrl+1, Ctrl+Shift+J); gore/dolje preskacu
leadove. Baza (`qnc-program-db`) nije mijenjana. Testovi `qnc-program-view` (novi), `qnc-program-segments`,
`qnc-segment-panel`, `qnc-panel-focus`, conformance, release build. Uzivo nije provjereno.
Popravak 2026-10-24 (korisnik: "ne pleja segmente"; log: "Program source 'part:lead_in_0:base_audio' has no active video
or audio output"): Off s iskljucenim A1 (lead-in/lead-out, a i Off koji je korisnik iskljucio, od 16. 10.) dobivao je u
playlisti izvor zvuka bez izlaza, pa je provjera odbijala cijelu pricu (player i prerender). `qnc-program-playlist`:
segment s iskljucenim A1 nema izvor zvuka; stavka bez izvora je crno i tisina (provjera vise ne trazi izvor); engine ju
je vec tako svirao. `qnc-program-input` `sound_only` vise ne vraca prvi izvor praznoj stavci. Test
`an_off_with_a1_off_is_black_and_silence_also_under_a_cover`, testovi playliste, program-input, enginea, rendera,
conformance, release build. Uzivo nije provjereno.
Dopuna 2026-10-24 (korisnik: "kad se dublklikom radi zum segmenta, onda se segment prikazuje u timeline; ako se radi zoom
program timeline, onda se on prikazuje; ako pokrivalica onda pokrivalica i dio predhodnog i narednog klipa"): zoom na
mjestu source timelinea vise nije uvijek slot (prica bez korisnickih markera ima jedan slot preko cijelog programa, pa
je dvoklik uvijek pokazivao sve). `qnc-program-segments` (`zoom.rs` `ZoomOn`): dvoklik (ili Ctrl+E na Wrap playheadu)
na slot s pokrivalicom povecava slot sa susjedima po 15 %, drugdje segment na tom mjestu (sa zalijepljenim leadom);
dvoklik na redak programa povecava cijeli program (nova akcija kataloga `program_zoom` bez tipke, `qnc-segment-panel`
je daje s retka programa; `qnc-program-commands` `Zoom { slot, frame }`, `ProgramZoom`). Isto ponovno zatvara, Esc
zatvara. Test, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-24 (korisnik: "nastavi s mjerenjem glasnoce"; plan normalizacije A1/A2 po EBU R 128, korak 1 i 2):
nova javna kocka `crates/qnc-loudness` (+ `contracts/modules/loudness.module.json`): mjeri jedan mono kanal po
ITU-R BS.1770-4 (K-filtar za svaku frekvenciju uzorkovanja; na 48 kHz koeficijenti standarda), za svakih 100 ms cuva
srednji kvadrat K-filtriranog signala i true peak (4x interpolator standarda); iz tih odsjecaka bilo koji dio programa
uz bilo koje pojacanje daje integriranu glasnocu (blokovi 400 ms svakih 100 ms, apsolutni prag -70 LUFS, relativni
-10 LU) bez ponovnog citanja zvuka. Otkljucani `qnc-wave` i `qnc-wave-worker` (zamrznuti, odjeljak 17; korisnik je
odobrio plan u kojem je to navedeno): mjeri se u istom prolazu kao val svake slike (kartica se ne cita jos jednom),
`WaveArtifactRecord.loudness` (po traci `LaneLoudness`), `WAVE_RENDER_VERSION` 3; val verzije 2 i dalje se prikazuje
(`shown_render_version`, i u `qnc-timeline-assets`), a radnik ga jednom napravi ponovno da dobije glasnocu. Grubi val i
zvuk s rupom nemaju mjerenje. Provjereno: testovi `qnc-loudness` (sinus -20 dBFS = -23 LUFS na 48 i 44,1 kHz, pragovi,
pojacanje, true peak, koeficijenti), `qnc-wave` (mjerenje u prolazu, rupa), ostali testovi vala; primjer
`qnc-loudness/examples/measure_raw.rs` na pravim klipovima s kartica (samo citanje) daje isto sto ffmpeg `ebur128`
(C0013: -22,96 / -22,9 LUFS, -4,87 / -4,9 dBTP; Mironik 3396 kanali 3 i 4: -40,70 / -40,7 i -38,97 / -39,0 LUFS).
Conformance, release build. Nije izvedeno: glasnoca programa (A1/A2 price iz odsjecaka klipova), prekidac normalizacije
u previewu, primjena u exportu.
Izvedeno 2026-10-24 (korisnik: "nastavi s glasnocom price i precacem u previewu"): nova javna kocka
`crates/qnc-program-loudness` (+ `contracts/modules/program-loudness.module.json`): integrirana glasnoca (LUFS) i true
peak (dBTP) A1 i A2 price iz odsjecaka od 100 ms izmjerenih s valom svakog klipa; segmenti na A1, pokrivalice na A2, po
izvornom IN/OUT-u, sa svojom razinom (iskljuceno je tisina, pa ni leadovi ne spustaju glasnocu); crossfade i J/L se ne
racunaju. `qnc-program-waveform` uz val cita i glasnocu klipa (val bez nje cita ponovno dok ga radnik ne napravi s
njom), `qnc-program-segments` je racuna kad se program, valovi ili stopa promijene (i s nacrtom zvuka), a
`SegmentsView.loudness`/`normalize` ih nose. Zaglavlje panela Segmenti pokazuje A1 i A2 u LUFS (`~` dok nije sve
izmjereno) i NORM dok je normalizacija ukljucena. Tipka N (nova akcija kataloga `toggle_loudness_normalize`, QNC preset,
scope storyboard, s bilo kojeg panela) ukljucuje i iskljucuje normalizaciju; `qnc-source-preview::normalize_program`
cilj uzima iz projekta (`audio_loudness`), pojacanje (cilj - izmjereno, najvise +-12 dB) daje izlaznoj razini A1/A2
playera dok svira prica (uz nacrt razine, preko njega), i svakom novom otvaranju price. Izvorni klip na source timelineu se
ne normalizira; izbor vrijedi dok aplikacija radi. Testovi `qnc-program-loudness`, `qnc-program-segments` (N),
`qnc-source-preview` (razina), conformance, release build. Uzivo nije provjereno. Nije izvedeno: primjena u exportu.
Izvedeno 2026-10-24 (korisnik: "nastavi s exportom; klik na dugme exporta otvara overlay info box s popisom svih postavki
exporta koje postoje u projektu; trenutno postavljene oznacene kao default, ali korisnik moze odabrati bilo koju iz
postojecih predefiniranih; izdvojen na samom pocetku prikaz Export to XML"): dugme Export HI-res u traci panela Segmenti
je ukljuceno kad prica ima segmente i salje novu akciju kataloga `export_open`. Nova javna kocka `crates/qnc-export-choice`
(+ ugovor): Export to XML prvi, zatim sve postavke exporta projekta (ugradeni preseti iz `contracts/export_profiles.json`
i projektovi vlastiti kroz `qnc-export-preset`), projektova (`export.preset`) oznacena kao default i odabrana; vrijednosti
koje ne odgovaraju nijednom presetu nude se kao "Project export settings" (default). Nova pasivna kocka
`crates/qnc-export-box` (+ ugovor): prozor u sredini ekrana, XML odvojen crtom, default oznacen, odabrani osvijetljen,
Export i Cancel; klik bira, dvoklik izvozi. `qnc-source-preview::export_settings` daje `export` iz postavki projekta,
`qnc-panel-focus` otvara prozor i dok je otvoren gore/dolje biraju, Enter izvozi, Esc zatvara; `qnc-program-segments`
drzi izbor (`open_export`, `ExportChoose`, `ExportConfirm`, `ExportCancel`). Akcije kataloga `export_open`,
`export_choose`, `export_confirm`, `export_cancel` (bez tipki). Sam export jos nije izveden: potvrda samo kaze sto je
odabrano. Testovi `qnc-export-choice`, `qnc-program-segments`, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-24 (korisnik: "FCP7 XML u mapu exporta; za video odmah ispod XML Express (navedi postavke prerender
videa, npr. HD 1080 PAL p50 4:2:0 ili onaj koji je stvarno koristen; ako GPU moze dekodirati 4:2:2, prilagodis); ispod
Postavke s naglaskom Original, a ispod default i ostale"): prozor exporta (`qnc-export-choice`, `qnc-export-box`) ima
redom Export to XML, Express s formatom prerendera iz postavki predaje projekta (format i kroma po kodeku: H.264 4:2:0,
XDCAM HD422 4:2:2; prerender radi u tom formatu), zatim odvojeno "Settings (rendered from the originals)": Original
(istaknut), projektova postavka kao default i odabrana, pa ostale. Export to XML: nova javna kocka `crates/qnc-fcp7-xml`
(+ ugovor): ravna playlista price kao FCP7 XML (xmeml 5): segmenti V1/A1, pokrivalice V2/A2, izvorni kanal i razina
(Audio Levels) po komadu, uzastopni komadi istog klipa spojeni, svaki klip jedna datoteka (original na ovom racunalu,
`pathurl`, pocetni timecode). Novi alat `tools/qnc-export --xml` (prica kao je cita player, bez upisa u bazu; XML
`<projekt>_<revizija price>.xml` u mapu exporta projekta `export.directory`) i nova kocka `crates/qnc-export-launch`
(+ ugovor; pokrece alat u svojoj niti, ishod se preuzima). `qnc-program-segments`: Export s XML-om pokrece izvoz i
javlja "Exporting..." pa put ili gresku; Express, Original i postavke jos samo javljaju da su sljedeci korak. Nova kocka
`crates/qnc-player-clip-content` (+ ugovor): jedini citac zapisa klipova za player ulaz; dvije dosadasnje kopije (preview,
prerender) uklonjene. Provjereno na izoliranoj kopiji aktivnog projekta (`iso3`, baze kopirane, kartica samo citana):
XML ispravan (xml parser), 4 clip itema, putanje na originale na kartici, timecode 06:35:53:24. Testovi, conformance,
release build. Kroz sucelje nije provjereno.
Izvedeno 2026-10-24 (korisnik: "da, Express tako, Original tako, nastavi"): Express izvoz. Nova javna kocka
`crates/qnc-unit-join` (+ ugovor `contracts/modules/unit-join.module.json`): gotovi dijelovi prerendera price redom spojeni u
jednu datoteku ffmpegom kataloga dekodera (slika i zvuk kopirani, bez ponovnog renderiranja); prije spajanja mjeri spojeni
zvuk (`qnc-loudness`, EBU R 128 / BS.1770-4); A1 (izlaz 1) i A2 (izlaz 2) zadrzavaju korisnikove razine kad su unutar cilja
+- tolerancije i ispod granice vrha, inace svaki dobiva pojacanje do svog cilja, smanjeno da vrh ostane na granici
(`qnc-loudness::export_gain`, test), i samo se zvuk ponovno pise (PCM 24-bit); ostali izlazi ostaju. Datoteka se pise
jednom pod konacnim imenom (mapa projekta ne dopusta preimenovanje). `tools/qnc-export --express`: dijelovi price kao u
prerenderu (isti otisci); dio koji prerender jos nema renderira se samo za izvoz u privremenu mapu sustava i brise poslije;
izlaz `<projekt>_<revizija>_express.mov` u mapi izvoza projekta. Gumb Export: Express se pokrece kao XML
(`qnc-program-segments`), poruka `export.written`. Citanje medija za render (`local_access`) premjesteno iz
`tools/qnc-prerender` u `qnc-program-render` (jedna kopija za prerender i izvoz). Provjereno na izoliranoj kopiji `iso3`
(kartica samo citana): 4 dijela renderirana iz originala za 40 s, 2124 slike i 2039040 uzoraka (tocno 960 po slici), A1
-27,4 LUFS doveden na -23,0 (ffmpeg ebur128 potvrdjuje -23,0), A2 tisina bez pojacanja. Original nije izveden: originali su
XAVC H.264 4:2:2 10-bit, a ffmpeg ovog racunala (LGPL, Intel QSV) nema koder za H.264 4:2:2 10-bit (ima ProRes, DNxHR,
MPEG-2); zamjenski kodek se ne bira bez odluke korisnika. Testovi, conformance. Uzivo kroz sucelje nije provjereno.
Izmjena 2026-10-24 (korisnik: "export prerendera bi bio bolji u MXF jer TV kuce preferiraju njega"): Express izlaz je
MXF OP1a umjesto MOV (`qnc-unit-join`, `tools/qnc-export`: `<projekt>_<revizija>_express.mxf`). Slika (H.264 prerendera,
bez B-slika) kopira se bez ponovnog kodiranja; svaki izlaz zvuka je svoja mono traka PCM 24-bit kao kod kamera (A1, A2,
...); izlaz bez pojacanja izdvaja se uzorak po uzorak, A1/A2 izvan pravila glasnoce dobivaju pojacanje. Provjereno
(izolirana kopija `iso3`): ffmpeg MXF muxer s H.264 daje 2124 slike bajt za bajt iste kao prerender, ton bit po bit,
OP1a; Express 4 dijela 36 s, A1 -23,0 LUFS (ffmpeg ebur128). Probano i XDCAM HD422 50 Mbit 1080p50 u MXF (2124 slike,
4:2:2) za kasniji export iz originala. Otvoreno: pocetni timecode datoteke je 00:00:00:00 (mnoge TV kuce traze
10:00:00:00; ceka odluku i postavku projekta). Testovi, conformance.
Popravak 2026-10-24 (korisnik: "pokvario si montazu sync/b-roll"; "bilo koji klip da odaberem na media poolu on postavlja
source timeline ovako"; "provjeri gdje si zeznuo"): log i baza stvarnog projekta (samo citanje) pokazali su tri greske Sync
puta; danasnje izmjene (leadovi, zoom, glasnoca, export) nisu dirale Sync kocke, a sva tri uzroka su u kodu od 20. 10.:
(1) I u letu dok Sync vec svira stizao je Syncu kao Space, pa je zaustavio izvor nakon jedne slike i ostavio pokrivalicu od
jedne slike (505-506 u projektu); sada I u letu dok Sync svira pocinje Sync ispocetka od tog IN-a, od istog markera
(`qnc-program-segments` `arm_sync_on_new_in`, test `an_in_on_the_fly_while_a_sync_plays_starts_it_again_from_that_in`).
(2) Sync koji ceka Enter crtao je svoj IN/OUT na source timelineu svakog klipa izabranog u poolu; sada samo na svom klipu
(`qnc-sync-cover` `source_clip`, `qnc-program-segments` `sync_source`). (3) Ukljucivanje Synca stavlja Wrap playhead na
najblizi marker, pa je Space nakon IN-a pustao pricu umjesto Synca; IN koji naoruza Sync sada izlazi iz Wrapa
(`qnc-sync-session` `armed_by_in`, `qnc-program-segments` `sync_armed`). Testovi, zlatna tablica tipki, conformance.
Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-24 (korisnik: "u ingest source timeline ima, u story source timeline nema"; "da, otkljucaj i
napravi"): u Storyju se od 18:55 nije otvorio nijedan player. Pri otvaranju projekta kartica D: je na tren nestala iz sustava
(radnik vala: "A device which does not exist was specified", os error 433), a Story je popis spojenih diskova procitao bas
tada i drzao ga do ponovnog otvaranja projekta; Ingest ga cita iznova kad se otvori popis diskova pa je radio. Sada
`qnc-source-bindings::with_mounted` popisu izvora procitanom jednom dodaje diskove spojene sada (postojeci ostaju kakvi
jesu); `qnc-source-preview` ga koristi pri svakom pokretanju playera (klip, program, Sync, izvan niti sucelja), pa vrijedi za
sve forme, a `qnc-still-locator` (slike reza) samo kad izvor klipa nije na procitanom popisu. Test
`a_disk_attached_after_the_sources_were_read_is_added_and_a_known_one_kept`, conformance. Uzivo nije provjereno.
Popravak 2026-10-24 (korisnik: "ako idem u povecavanje prikaza Story izlazi iz Story panela, pomaknut je obrub oko panela;
vrati ga na pravo mjesto i smanji debljinu linije, neka bude diskretna"; otkljucani `qnc-segment-panel`, `qnc-ui-kit`):
zaglavlje panela Segmenti (A1/A2 LUFS, Segment, Playhead, Duration) slagalo se zdesna u istom redu kao naslov, pa je
vrijednost koja ne stane rastezala cijeli panel ulijevo preko monitora (i bez povecanja: naslov i LUFS su se preklapali),
a okvir fokusa ostao je na mjestu panela. Sada je zaglavlje red tocno sirine panela: naslov lijevo, vrijednosti iza njega
odrezane na panelu, nikad sire; tijelo panela nikad nije sire od njegova mjesta. Okvir fokusa (`qnc-ui-kit::paint_focus`,
sve forme) je 1 px, boja fokusa prigusena, na rubu mjesta panela. Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-24 (korisnik: "nastavi s exportom originala"; odabrao "XDCAM HD422 MXF"; "dodaj link u statusnoj liniji
gdje prikazujes da je export zavrsio"): export Original i sve postavke exporta iz prozora Export. Originali su XAVC H.264
4:2:2 10-bit, a ffmpeg ovog racunala (LGPL) nema koder za njega, pa je Original XDCAM HD422 50 Mbit (MPEG-2 4:2:2, 8-bit)
u MXF OP1a pri slici i zvuku same price (velicina prve slike, stopa price, izlazi zvuka projekta).
`qnc-export-choice` `delivery_values` (`StoryFormat`): vrijednosti predaje izabranog retka (original, postavke projekta ili
preset preko postavki projekta). `tools/qnc-export --render <id>`: isti put kao Express (dijelovi iz prerendera kad su u
istom formatu, inace renderirani iz originala u privremenu mapu), spajanje i pravilo glasnoce `qnc-unit-join`, izlaz
`<projekt>_<revizija>_<id>.mxf` u mapi exporta. `qnc-program-segments`: Original i postavke pokrecu `--render`; tekst
`export.not_yet` uklonjen. Popravak u `qnc-program-render`: koder XDCAM nikad nije radio (ffmpeg odbija zatvoreni GOP uz
prepoznavanje scena; dodan `-sc_threshold 1000000000`), a greska kodera sada se javlja umjesto "pipe has been ended".
Link: nova kocka `crates/qnc-status-line` (+ ugovor; tekst statusa s datotekom o kojoj govori, otvaranje datoteke u
pregledniku datoteka OS-a: Explorer, Finder, mapa na Linuxu); `qnc-shell-desktop-api` `footer_link`, `qnc-shell-footer`
(status s linkom je podcrtan i klikom salje `OpenLink`), `qnc-desktop` (otvara ga), `qnc-program-view`
`SegmentsView.exported_file` (dok poruka kaze da je export gotov), `qnc-editorial-application`/`-desktop` `footer_status`
daje `Status`, Story i MA adapteri `footer_link`. C10 editorial aplikacija 573 -> 571. Provjereno na izoliranoj kopiji
(kartica samo citana): Original 4 dijela za 45 s, MXF s MPEG-2 4:2:2 1920x1080p50, 2124 slike (kao Express), A1 -27,4 doveden
na -23,0 LUFS (ffmpeg ebur128), dvije mono trake PCM 24-bit. Testovi, conformance. Link kroz sucelje nije provjeren.
Interlaced preset (npr. `xdcam_hd422_50i`) i dalje daje kontroliranu gresku (render jos ne radi isprepletenu predaju).
Izvedeno 2026-10-24 (korisnik: "export XDCAM treba ici iz originala, ne iz prerendera"; "tekst Exporting + naziv formata
exporta treba biti crvene boje, kad zavrsi zelene"; log: izbor XDCAM HD422 50i javio je "Prerender does not make interlaced
delivery yet"): (1) `tools/qnc-export --render` (Original i sve postavke) renderira svaki dio iz originala, nikad ne uzima
prerender; prerender koristi samo Express. (2) Isprepletena predaja: dijelovi se renderiraju bez gubitka kao progresivni
dijelovi u stopi price (FFV1, `qnc-program-render` `Delivery::intermediate`), a pri spajanju (`qnc-unit-join` `join`,
argument `picture`) slika se kodira jednom: prica dvostruke stope daje svakoj slici jedno polje (`interlace`, redoslijed
polja iz `field_order`), prica iste stope oba polja jedne slike; XDCAM HD422 50 Mbit s `+ildct+ilme`, GOP 12
(`Delivery::interlaced_encode`); interlaced H.264 jasna greska (`render.interlaced_codec`). (3) Status: "Exporting
<naziv formata>..." crveno dok radi i kad export ne uspije, "Exported: ..." zeleno s linkom. `qnc-status-line` `Tone`
(Plain, Alert, Done) i `Status::plain`, `qnc-program-view` `SegmentsView.export_alert` i `status()`,
`qnc-program-segments` (`export_alert`), `qnc-shell-desktop-api` `footer_tone`, `qnc-shell-footer` (boja po tonu),
`qnc-desktop`, Story i MA adapteri. Provjereno na izoliranoj kopiji (kartica samo citana): 50i 4 dijela za 2 min,
MXF MPEG-2 4:2:2 1920x1080, 25/1, field_order tt, 1062 slike (2124 slike price / 2), idet 301 od 301 TFF, A1 -23,0 LUFS
(ffmpeg ebur128); Original iz originala i dalje radi. Testovi, conformance. Kroz sucelje nije provjereno.
Popravak 2026-10-24 (korisnik: "link ne vodi na exportiranu datoteku"): put exporta bio je mjesovit
(`...\exports/projekti\<datoteka>`, `export.directory` je zapisan s `/`), a Explorer s `/select,` takav put ne nade i
otvori drugu mapu. `tools/qnc-export` (`export_folder`) i `qnc-status-line` (`open`) slazu put sa separatorom ovog OS-a.
Provjereno: izolirana kopija daje `...\exports\projekti\test_ukupni_11.xml`; testovi. Klik kroz sucelje nije provjeren.
Otkljucano i izvedeno 2026-10-24 (korisnik: "timecode 10:00:00:00 kao postavka projekta"; Project otkljucan samo za tu
postavku): `export.start_timecode` (HH:MM:SS:FF) je timecode prve slike svakog exporta. Seed svih sistemskih predlozaka
10:00:00:00, `qnc-project-store` ga dopunjuje starijem projektu pri otvaranju (test), Advanced blok Export izbor "Start
timecode" (10:00:00:00, 01:00:00:00, 00:00:00:00; `start_timecodes` u ugovoru, rjecnik `project.start_timecode`);
`qnc-work-settings` `export_start_timecode` cita strogo (nedostaje ili neispravan: kontrolirana greska, test). MXF
exporti (Express, Original, postavke, i isprepletena predaja) ga upisuju (`qnc-unit-join` `join(.., start_timecode)`,
ffmpeg `-timecode`), XML (FCP7) kao timecode sekvence (`qnc-fcp7-xml` `sequence(.., start_timecode)`, frame po stopi
price). Aktivni projekt bez postavke ne exportira dok se ne otvori u Projectu. Provjereno na izoliranoj kopiji: Original
i XDCAM HD422 50i MXF `timecode=10:00:00:00`, XML `10:00:00:00` / frame 1800000. Testovi, conformance. Project je
ponovno zamrznut.
Izvedeno 2026-10-24 (korisnik: "ako je u storyju za neki klip odredio da na A2 kanalu bude 3 audio kanal kamere, onda za
taj klip u pokrivalici treba koristiti 3 kanal; defaultna postavka je 2 kanal kamere, ali korisnik mora moci za svaku
pokrivalicu izabrati bilo koji kanal; isto tako i za A1"; "promjena kanala moze se napraviti i u zoom opciji storyja"):
zvuk A1/A2 (kanal kamere, ukljucen/iskljucen, razina) koji se zadrzi na source timelineu (Enter, klik na kanal) pamti se
po klipu u bazi projekta. Nova javna kocka `crates/qnc-clip-sound` (+ ugovor modula `clip-sound.module.json` i baze
`clip-sound.database.json`): modul tablica posrednika, tablica `clip_sound` i pogled `public_clip_sound`; `ClipSounds`
cita u pozadini jednom po klipu i pise bez cekanja kroz serijskog pisca. `qnc-source-timeline` (`sync` prima preview):
klip koji ima zapis pocinje na svom zvuku (samo kanali koje klip ima), inace A1 kanal 1, A2 kanal 2; zadani zvuk klipa
bez zapisa se ne pise. Novi segment i pokrivalica tog klipa uzimaju taj zvuk (kao do sada uzimaju zadrzani zvuk source
timelinea); napravljeni segment ili pokrivalica zadrzavaju svoj zvuk, koji se i dalje mijenja zasebno na Wrap retku i u
zoomu. Vrijedi u svakoj formi sa source timelineom (Story, MA, Ingest). `qnc-source-preview` `project_target` (iz
konteksta, izvan niti sucelja). Conformance: `qnc-clip-sound` na popisu javnih DB vlasnika. Testovi, conformance.
Uzivo nije provjereno.
Izvedeno 2026-10-24 (korisnik: "A2 nije normaliziran i neke pokrivalice iskacu iz pravila"; odluke: najvise +12 dB po
pokrivalici; vrijedi za export i preview N): mjereno u `test_ukupni_42`: A2 cijeli -23,0 LUFS, a pojedine pokrivalice od -20
do -38 LUFS, jer je export dizao samo A2 kao cjelinu. Sada svaka pokrivalica sama ulazi u pravilo A2 projekta (cilj,
tolerancija, granica vrha): `qnc-program-loudness` `cover_gain` i `CoverRule` (pokrivalica izmjerena sama, od svoje prve
slike, iz odsjecaka glasnoce spremljenih s valom klipa, na razini koju je korisnik dao; unutar tolerancije 0, inace cijeli dB
do cilja, dizanje najvise `COVER_GAIN_MAX_DB` 12 dB i nikad preko granice vrha; iskljucena, tiha ili neizmjerena
pokrivalica ostaje kako je montirana; test), `qnc-program-input` (`normalize.rs`: pojacanje ide na razinu pokrivalice dok se
gradi program, pa ga cuju player, render i export jednako; `loader`/`draft_loader`/`load_program` primaju
`Option<CoverRule>`; nista se ne pise u bazu). `tools/qnc-export` gradi program s pravilom (XML, Express, Original,
postavke); `qnc-source-preview` uz ukljucen N otvara pricu s pravilom (A1 i dalje cijeli, kao prije), a ukljucivanje ili
iskljucivanje N ponovno otvara pricu na istom mjestu. Prerender ostaje bez normalizacije (dijelovi s promijenjenom razinom
imaju drugi otisak i renderiraju se za export). Provjereno na izoliranoj kopiji projekta (baza kopirana citanjem, kartica samo
citana): 8 od 12 pokrivalica -22,9 do -23,5 LUFS, preostale 4 dignute za punih 12 dB (izabrani kanal tih klipova je tih) na
-26 do -32. Testovi, conformance. Uzivo kroz sucelje nije provjereno.
Otkljucano i izvedeno 2026-10-24 (korisnik: "napravi CBR Expres export"; "da, otkljucaj Project i kreni s 35 Mbit/s CBR"):
H.264 prerender (pa i Express, koji ga spaja bez ponovnog kodiranja) kodira se na stalnom bitrateu iz postavke projekta
`export.video_bitrate_mbps` (seed svih sistemskih predlozaka 35, razina XAVC Long 35 / XDCAM HD 35 za 4:2:0;
`qnc-project-store` je dopunjuje starijem projektu pri otvaranju, test; Advanced Export "H.264 bitrate CBR (Mbit/s)" 25/35/50,
`video_bitrates` u ugovoru, rjecnik). `qnc-program-render`: `Delivery.bitrate_mbps` (H.264 bez nje je kontrolirana greska),
`encoder.rs` svaki koder u CBR (`-b:v`/`-minrate`/`-maxrate` isti; QSV, NVENC `-rc cbr`, AMF `-rc cbr`, VideoToolbox
`-constant_bit_rate`, libx264 `nal-hrd=cbr`), spremnik cetvrt sekunde, poceti napola pun (s jednom sekundom spojeni dijelovi
skakali su 32-43 Mbit/s po sekundi); `UNIT_FORMAT` 4 (postojeci dijelovi jednom se renderiraju ponovno). Prozor exporta pise
"Express: HD 1080p50 4:2:0 35 Mbit/s CBR" (`qnc-export-choice`). XDCAM HD422 ostaje 50 Mbit/s CBR. Izmjereno na izoliranoj
kopiji (kartica samo citana): jedan klip QSV 34,7-36,4 Mbit/s po sekundi; Express price od 29 dijelova slika prosjek 35,6,
po sekundi 34,0-40,5 (vrhovi na pocetku dijelova), datoteka ukupno 38,5 Mbit/s sa zvukom. Aktivni projekt bez postavke ne
radi prerender ni Express dok se ne otvori u Projectu. Testovi, conformance. Project je ponovno zamrznut.
Popravak 2026-10-25 (korisnik: "export prerendera je extremno dug"; log: Express 87 s za pricu od 41 s): prerender je
dijelove radio bez normalizacije pokrivalica, a export s njom, pa su dijelovi pokrivalica s pojacanjem imali drugi otisak i
Express ih je ponovno renderirao iz originala; uz to je radio usporedno s prerenderom koji je bio u tijeku. Sada
`tools/qnc-prerender` gradi pricu s istim pravilom pokrivalica kao export (`CoverRule` iz postavki projekta), N u previewu je
ukljucen pri pokretanju (`qnc-program-segments`, preview cuje sto daje export i pusta prerender), Express dok je najam
prerendera ziv ceka njegove dijelove (najvise 10 min, `prerendered_units`) umjesto da ih renderira drugi put, a svaki export
pise u player log broj dijelova, koliko je iz prerendera, trajanje i glasnocu (`export ...`). Izmjereno na izoliranoj kopiji
(kartica samo citana): prerender cijele price 2 min 4 s (29 dijelova, iz originala s kartice), Express zatim 3,3 s (29 od 29
iz prerendera; prije 60-87 s). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-25 (korisnik: "gdje vidim da li je normalizacija ukljucena, gdje je ukljucujem ili iskljucujem"; "da, dodaj
dugme NORM i zapis u log"): u traci panela Segmenti desno od pokazivaca razine dugme NORM (`qnc-segment-panel` `bar.rs`),
osvijetljeno dok je normalizacija ukljucena; klik je isto sto tipka N (`toggle_loudness_normalize`), savjet pod misem u
rjecniku (`story.ui.normalize_hint`). Player log: `story-normalize on|off` s izmjerenim A1/A2 pri svakoj promjeni i
`story-normalize on|off (project opened)` pri otvaranju projekta (`qnc-program-segments`). Popravak: otvaranje projekta
vracalo je panel na zadano stanje i iskljucivalo normalizaciju (N je zato bio iskljucen iako je od 25. 10. ukljucen pri
pokretanju); sada izbor korisnika ostaje (test). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-25 (korisnik: "pogledaj log... dugme normalizacije treba imati izgled kao i drugi dugmi (npr. B-roll)"):
(1) dugme NORM crta se kao prekidac B-roll | Sync/B-roll (`qnc-ui-kit::chrome_button`, ukljuceno u boji fokusa;
`qnc-segment-panel` ovisi o `qnc-ui-kit`). (2) Log je pokazao `story-normalize ... a1_lufs=None`: puni val segmenta
Mironik 3452 (4,8 min) s glasnocom bio je veci od granice zapisa vala (256 KB), pa ga je radnik racunao iznova i baza ga je
stalno odbijala ("The wave record is too large"); klip je imao samo grubi val bez glasnoce, pa A1 nije bio izmjeren.
`qnc-artifact-db`: zapis vala smije imati do pola najvece poruke transporta baze (8 MB, oko sat vremena 4 kanala); zapis
filmstripa ostaje 256 KB. Provjereno na izoliranoj kopiji: radnik je spremio puni val 3452 s glasnocom sva 4 kanala.
Testovi, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-25 (korisnik: "neke TV kuce traze A1 na oko -20, a A2 treba biti tisi"; "da, otkljucaj Project i
prosiri izbornike"; Project otkljucan samo za izbornike cilja glasnoce): Project Advanced Loudness ima zasebne izbornike
cilja: A1 `lufs_a1` -16, -18, -20, -23 (EBU R 128), -24 (ATSC A/85); A2 `lufs_a2` -20, -23 (EBU R 128), -26, -28, -30
(`contracts/ui/project.layout.json`; zajednicki `lufs` uklonjen). Zadano ostaje -23/-23 iz seeda; spremljena vrijednost
izvan liste i dalje se prikazuje i vrijedi. Normalizacija u previewu (NORM), prerenderu i exportu vec cita cilj A1 i A2
zasebno. Testovi, conformance. Project je ponovno zamrznut.
Izvedeno 2026-10-25 (korisnik: "da, ubrzaj prerender, crossfade u susjedni dio"; "tekuci projekt je za A2 postavljen na -30,
znaci li to da odabir ne radi"): (1) Odabir radi: aktivni projekt je `novi--33333` i u njegovoj bazi je A2 -30 LUFS (ranije
sam citao `test ukupni`). (2) Prerender: jedinice price rezu se samo gdje se slika mijenja (drugi klip, skok u izvoru, crno
i slika), ne gdje se mijenja samo zvuk; crossfade od nekoliko slika vise nije zaseban dio (svaki je imao fiksni trosak
1,1-1,7 s). `qnc-broadcast-engine` (`unit_cuts`, `sound_pieces`: zvuk unutar jedinice na svom mjestu, rezan gdje pocinje,
zavrsava ili mu pocinje/zavrsava fade; `with_prerendered` za jedinicu bez gotovog dijela uzima sve zvucne dijelove unutar nje),
`qnc-render-unit` (`AudioPart.at_sample`, `length`, `span`; izostavljeni kad dio traje cijelu jedinicu, pa jedinice bez
crossfadea zadrzavaju otisak), `qnc-program-render` (dio zvuka puni samo svoje mjesto u jedinici). `tools/qnc-player-acceptance`
otvara pricu s ukljucenim N kao Story (inace prica ne odgovara normaliziranom prerenderu i svira iz izvora). Izmjereno na
izoliranoj kopiji (kartica samo citana): 12 dijelova umjesto 29, render oko 41 s umjesto oko 64 s; Express 1,3 s; zvuk A1 i
A2 Expressa bit po bit isti kao s 29 dijelova; prica iz prerendera 2x PASS 100 %, bez trzaja (iz izvora s kartice, bez
prerendera, 95 % i 18-25 trzaja). Testovi (`units_are_cut_where_the_picture_changes_and_a_crossfade_lies_inside_one`),
conformance. Uzivo kroz sucelje nije provjereno.
Izvedeno 2026-10-25 (korisnik: "s alatima koje imamo veoma je tesko precizno rezati ton, posebno kad se zeli izbaciti slovo
ili kratka rijec"; "postojeci alati, ali prosireni: zoom koji imamo na segmentima (desni klik skoci na najblizi marker, zoom dvije
slike s markerom izmedju njih) primijeniti i na IN/OUT source timelinea; tom zoomu dodati zoom audija s tankim playholderom koji
prolazi izmedju dvije slike i dijeli audio prikaz"): spremljeni val ima 1200 tocaka po klipu (cetvrt sekunde na klipu od pet
minuta). (1) Trim zoom i za IN/OUT source timelinea: dok je IN ili OUT uzet (Ctrl+I/O, desni klik, Ctrl+kotacic), prozor u
sredini ekrana pokazuje za IN zadnju sliku prije i prvu sliku IN-a, za OUT zadnju sliku do OUT-a i prvu iza njega; rez je
izmedju njih (`qnc-timeline` `taken_source_mark`, `qnc-program-segments` `SourcePick.taken`, `source_stills`; rjecnik
`story.still.before_in`, `in_first`, `out_last`, `after_out`). Rub segmenta (Ctrl+Shift+I/O) i dalje pokazuje svoje slike.
(2) Ispod slika svakog trim zooma (slot, marker, rub, IN/OUT) su A1 i A2 sekundu prije i poslije reza, s tankom crvenom linijom
reza koja prolazi izmedju dviju slika i kroz oba vala (`qnc-trim-overlay`; visina vala u dB od -54 dBFS, pa se tihi govor i
pauza poslije rijeci razlikuju). Za source IN/OUT valovi su kanali koje A1/A2 source timelinea cuju, za pricu segmenti na A1 i
pokrivalice na A2 sa svojom razinom (`qnc-program-segments` `zoom_sound`, `qnc-program-view` `ZoomSound`). Nova javna kocka
`crates/qnc-fine-wave` (+ ugovor `contracts/modules/fine-wave.module.json`): fini val citan iz originala kamere na ovom racunalu
oko reza (1,5 s rezerve) kroz katalog dekodera u pozadini, uzorci u memoriji, vrh po stupcu prozora (900); bez probea, bez
baze. `qnc-still-locator` `locate_sound` (spremljeni zvucni tokovi originala), `qnc-trim-stills` ga puni. `tools/qnc-player-acceptance
--fine-wave NAZIV` mjeri citanje. Izmjereno na izoliranoj kopiji (kartica samo citana): 3396 oko slike 400, prvi kanal 2,6 s
(budenje kartice), ostali 0,17-0,24 s. Ingest nema trim zoom (nije u ovom koraku). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-25 (korisnik: "pogledaj log, nije bas robusno"): fini val reza nije bio pouzdan na rubovima klipa.
(1) Citanje koje ide preko kraja klipa dekoder odbija ("timestamp outside saved duration"); greska je oznacavala cijeli kanal
klipa kao neuspio do kraja rada, pa taj kanal vise nikad nije imao fini val. Sada se pamti samo neuspjeli dio, a zvuk procitan
do kraja klipa se zadrzava. (2) Komad koji mediji ne mogu dati do kraja trazenog dijela (pocetak ili kraj klipa) trazio se
ponovno u svakom crtanju, pa se kartica citala u krug i sucelje je stalno crtalo. Sada komad pamti slike za koje je procitan
i vise se ne trazi (`qnc-fine-wave` `Chunk.frames`, `answers`, `Ask::within`, test). Svako citanje pise u player log
(`fine-wave read ... ms= ok=`). Izmjereno na izoliranoj kopiji (kartica samo citana): 3396 kod pocetka i kod kraja klipa
(1376 slika) po jedno citanje kanala, 0,14-0,44 s (prvo 2,3 s, budenje kartice), zvuk do kraja klipa. Conformance.
Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-25 (audit cijele aplikacije; korisnik: "da, kreni redom od 1, otkljucavam Project"): korak 1,
greske playera. (1) Zvuk koji se isprazni usred takta (ne na pocetku) gasio je sesiju: greska upisa u red bila je Contract,
engine je pauzirao i Play vise nije radio. Sada je to zasebna vrsta `BroadcastEngineErrorKind::AudioUnderrun`
(`qnc-broadcast-engine` `audio_error`), transport ostaje u Playing i zvuk se priprema ponovno kao kod praznog zvuka na pocetku
takta (`qnc-broadcast-player` `tick`). (2) Prazan zvuk manje od 16 slika prije OUT-a cekao je zauvijek (sat stoji u Playing):
tada play zavrsava na granici raspona kao malo kasnije (`too_near_the_end`, `end_at_boundary`, dogadaji iduceg takta; test).
(3) Zvuk je bio samo 0,64 s ispred prikazane slike, pa je svaki zastoj slike dulji od toga praznio uredaj; sada 1,5 s
(`AUDIO_LEAD_SECONDS`, `audio_lead_frames`; spremnik uredaja 2 s). Razina, rute i dio razine i dalje se primjenjuju u pozivu
uredaja, odmah. Nije mijenjano: nakon praznog zvuka zvuk krece od prikazane slike (slika i zvuk ostaju zajedno; ako je slika
stajala dulje od zalihe, dio zvuka se cuje ponovno). Dva testa koja su zakljucavala staro pravilo (zvuk koliko i slika)
prilagodena. Provjereno: testovi `qnc-broadcast-player` (80), `qnc-broadcast-engine`, `qnc-audio-output`, conformance;
`qnc-player-acceptance` na izoliranoj kopiji: prica od 0 i od 600 100 % slika (jednom trzaj 85 ms), A/V -0,2 slike; klip 3396
1-3 trzaja oko slike 470 jednako kao stari kod (izmjereno istim alatom prije izmjene: 1-4 trzaja). Uzivo nije provjereno.
Izvedeno 2026-10-25 (audit, korak 2, greske montaze price): (1) Skracivanje segmenta (`TrimSegment`) racunalo je markere
kroz slike izvora, pa je izvor upola manje stope (25p u 50p prici) gubio neparnu sliku i mogao obrisati markere koji ostaju u
segmentu; sada u slikama price (`qnc-program-db` `trim_segment`, test). (2) Zamjena i skracivanje segmenta mogli su ostaviti
dva markera na istoj slici; nakon izmjene granicni markeri dobiju novo mjesto, markeri na istoj slici postaju jedan
(`merge_coincident_markers`), slotovi se slazu iznova. (3) Sync ili rez markera koji se nikad nisu upisali (upis nije krenuo
ili je pao) drzali su Enter i odbijali svako drugo uzimanje: bez upisa u tijeku nista ne ceka (`qnc-program-segments` `poll`),
novi IN i Delete pustaju Sync, Esc odbacuje i zatvoreni Sync slot koji nije upisan (`qnc-sync-cover` `waits`,
`qnc-panel-focus`). (4) Undo/Redo puste sve uzeto (slot, rub, J/L, zvuk, pokrivalica, marker; test), brisanje i premjestanje
segmenta zatvaraju otvoreni edit. (5) Oznake markera, segmenata i pokrivalica ne mogu se ponoviti u istom procesu
(`unique_stamp`). Testovi `qnc-program-db` (45), `qnc-program-segments` (60), zlatna tablica tipki, conformance. Nije
izvedeno iz audita (niska vaznost): razlika jedne slike izvora izmedu nacrta i upisa rolla pri 2:1, rez pokrivalice na
neparnoj slici pri 2:1, provjera susjeda u bazi pri pomaku markera. Uzivo nije provjereno.
Izvedeno 2026-10-25 (audit, koraci 3-5): (3) Kopirani kod u jednu kocku: nova javna kocka `crates/qnc-gain` (+ ugovor;
dB kao pojacanje, prije 13 kopija; granica playera je dopusta, cista aritmetika); korijen QNC-a jedno pravilo
`qnc-dev-diagnostics::resolve_qnc_root` (ljuska, Project, Ingest, Story i MA; postuje QNC_ROOT, nevaljani QNC_ROOT nije
korijen, nikad radni direktorij; Ingest je prije padao na radni direktorij, samostalne aplikacije nisu postovale QNC_ROOT);
pokretanje pomocnih programa kroz `qnc-plugin-process` (`beside_application`, `background_command`) umjesto tri kopije;
XML export formatira i cita timecode kroz `qnc-source-timecode` (`parse_timecode`), bez izmisljenih 25 fps. Nije spojeno
(promijenilo bi izgled ili dira vlasnike baza): tema ljuske i formi (ljuska crta drugim bojama i razmacima), otvaranje
SQLite baze u sedam vlasnika, zrcalni radnici filmstripa i vala. (4) Nova javna kocka `crates/qnc-short-save` (+ ugovor):
spremanje SubClipa s provjerama i upisom u pozadini (prije je upis cekao bazu na niti sucelja); kartice SubClipova slaze
`qnc-virtual-short-cards::cards`; C10 Story aplikacija 571 -> 528. Nije izvedeno: dijeljenje `qnc-panel-focus` i
usklajivanje previewa iz Story aplikacije. (5) Project otkljucan (korisnik "otkljucavam Project"): tekstovi Projecta
(`DB:`, `Template:`, "RADNI TOK", greske spremista), Select poruke Ingesta, ishod uvoza i poruke radnika i ljuske idu
kroz rjecnik. `qnc-project-close` nije mijenjan: AGENTS.md odjeljak 3 izricito mu dopusta isprazniti `active_project_id`.
Nije izvedeno: rad Projecta s bazom izvan niti sucelja (log: 100-300 ms zastoja pri otvaranju i kreiranju), boje tema
Soft/HighContrast u ugovor ljuske (ljuska nije otkljucana). Testovi dotaknutih kocki, conformance, release build. Project
je ponovno zamrznut.
Otkljucano i izvedeno 2026-10-25 (korisnik: "napravi rad Projecta s bazom izvan niti sucelja"; Project otkljucan za to):
Project forma je otvaranje spremista, popis projekata i templatea, kreiranje, otvaranje i brisanje projekta, izbor, kreiranje i
brisanje templatea te mapu projekata radila na niti sucelja (log: 100-300 ms zastoja). Nova javna kocka
`crates/qnc-store-jobs` (+ ugovor; jedna pozadinska nit vlasnik spremista, poslovi redom kojim su poslani, odgovori se
preuzimaju bez cekanja; ne zna ni spremiste ni aplikaciju) i nova kocka `crates/qnc-project-jobs` (+ ugovor; zahtjevi i
odgovori Project forme, izvode se u toj niti nad `ProjectStore` koji se tamo i otvara; otvoren ili kreiran projekt tamo
procita i slijed aplikacija za prelazak desktopa). `qnc-project-application` `ProjectComponent` salje zahtjeve (`request`,
`poll`, `busy`), `qnc-project-session` primjenjuje odgovore kad stignu (`poll`, `apply_reply`), forma (`qnc-project-desktop`)
ih preuzima svaki frame i trazi crtanje dok posao traje. C10 `qnc-project-application` 237 -> 147. Nije mijenjano: preglednik
lokacija (`qnc-dir-browser`) i dalje lista mape na niti sucelja. Provjereno: testovi `qnc-store-jobs`, `qnc-project-jobs`
(odgovori redom iz pozadinske niti na privremenom korijenu), `qnc-project-store`, `qnc-desktop`, conformance, release build;
pokretanje aplikacije bez ijednog sporog framea u logu. Uzivo kroz sucelje (kreiranje, otvaranje, brisanje) nije provjereno.
Project je ponovno zamrznut.
Dopuna 2026-10-25 (korisnik: "kada segment editiram u source timeline i kada zavrsim, kako se vracam na normalni rad"): log i
baza aktivnog projekta (samo citanje) pokazali su da je Enter spremio izmjenu (IN 851 -> 848), ali je poruka i dalje glasila
"Segment edit: ... Enter saves, Esc cancels", pa je izgledalo da je izmjena jos otvorena. Nakon spremanja status kaze
"Segment saved; back to normal work." (`qnc-program-segments` `commit_edit`, rjecnik `story.msg.segment_edit_saved`).
Postupak je isti: Enter sprema, Esc odustaje; oba zavrsavaju uredivanje segmenta.
Popravak 2026-10-25 (korisnik: "pogledaj log"; log: nakon Edit segmenta Story na mirovanju crta 61 put u sekundi,
0,45-0,9 s procesora u sekundi, 235 `ui-frame-slow` 25-112 ms): player u stanju Ready (ne Paused) na svaki upit stanja
odgovara istim stanjem s novim rednim brojem odgovora, a `qnc-source-preview` je cijeli pogled playera usporedjivao
zajedno s rednim brojem, pa je svaki poll bio "promjena", ona je trazila novo crtanje, a novo crtanje novi poll. Sada je
isto stanje ono s istom sesijom, generacijom, dogadajima, pripremom, slikom i greskom (`same_player_state`, kao
`same_state` klijenta). Test `a_reply_with_only_a_new_sequence_is_the_same_player_state`, conformance, release build;
uzivo: Story na mirovanju 3-4 crtanja u sekundi.
Izvedeno 2026-10-25 (korisnik: "pogledaj log, i nastavi sredivati aplikaciju prema rezultatima audita"): log nakon
popravka crtanja u mirovanju je uredan (Story u mirovanju 2 crtanja u sekundi, bez gresaka zvuka i playera; jedini zastoj
sucelja od 451 ms poklopio se s istodobnim zastojem niti playera od 310 ms, sto upucuje na zastoj sustava ili kartice).
Stavka audita "preglednik lokacija lista mape na niti sucelja": Project je popis diskova i mapa (mapa projekata, mapa
exporta) citao na niti sucelja, pa je disk koji spava zamrzavao formu. Nova javna kocka `crates/qnc-dir-browse-jobs`
(+ ugovor `contracts/modules/dir-browse-jobs.module.json`): sesija `qnc-dir-browser` zivi u pozadinskoj niti
`qnc-store-jobs`, koraci (korijeni, mapa, roditelj, pocetna putanja) idu redom, popis se preuzima bez cekanja, uz njega
privatna putanja prikazane mape za potvrdu. `qnc-project-application` (`browse`, `poll_browsers`), `qnc-project-session`
(popis primjenjuje `poll`, koji forma vec zove svaki frame). Ingest vec lista kroz `qnc-source-browse` u pozadini.
`tools/qnc-conformance` pravilo Project granice prihvaca zajednicku sesiju i kroz tu kocku. C10 `qnc-project-application`
147 -> 123. Testovi (`qnc-dir-browse-jobs`, Project kocke, conformance 26), release build. Uzivo kroz sucelje nije
provjereno. Project je ponovno zamrznut.
Popravak 2026-10-25 (korisnik: "exportirani xml resolve 21 ne ucitava, a u ediusu prva pokrivalica ne pleja ton iako se vidi
da postoji"): u XML-u (`novi--33333_59.xml`, samo citano) pokrivalice na A2 preklapale su se za 3-4 slike (crossfade, J), a
FCP7 XML na jednoj traci smije imati samo jedan klip u isto vrijeme; datoteka je zvuk opisivala kao jednu traku s 4 kanala,
a klip na A2 trazio je `trackindex` 2 (traku koja po tom opisu ne postoji); sekvenca nije imala format zvuka. Baza i XML
biraju dobar kanal (kanal 2 klipa 3449 ima ton, -24,5 dB; 3427 je na kanalu 2 doista tih, -90 dB). `qnc-fcp7-xml`: klip
koji pocne pod prethodnim rezu se na pocetku sljedeceg, a preklapanje je Cross Fade (+3dB) prijelaz od tog reza
(`without_overlaps`, `<transitionitem>`); svaki kanal datoteke je svoja mono traka (`<audio>` s `channelcount` 1,
`sourcechannel` i frekvencijom iz spremljenog zapisa, `ClipFile.audio_sample_rate`); sekvenca ima `samplecharacteristics`
(24-bit, frekvencija projekta) i dva izlaza. `tools/qnc-export` predaje frekvencije. Provjereno na izoliranoj kopiji: XML
ispravan (parser), A2 12 klipova i 10 prijelaza, bez preklapanja. Test. U Resolveu i Ediusu nije provjereno.
Otkljucano i izvedeno 2026-10-25 (korisnik: "zablokirao se source player"; "da, otkljucaj i popravi, obrisi i debug cache";
otkljucani `tools/qnc-player-runner` i `crates/qnc-player-client`): log: nakon Entera Synca preview je klip otvarao s A1
sljedeceg Sync prozora, a to je bio lead-out (A1 iskljucen), koji ne cita nijednu datoteku i nema vezu izvora; player je
takvo pokretanje odbijao ("player bootstrap needs one clip with its binding") i nista nije javio, pa je klijent svaki put
cekao 10 s i source player nije radio. Sada player prima A1 Synca bez veza izvora (tisina; `config.rs`
`clip_bindings_fit`, test), a odbijeno pokretanje odmah javlja (`session_refused`; i sesija koja zavrsi prije pozdrava),
pa klijent javlja gresku umjesto cekanja (`qnc-player-client` `open_session`). Uz to disk C: je bio pun (454 MB slobodno,
prerender nije mogao pisati): obrisan debug cache (`cargo clean --profile dev`, 40,7 GiB) uz dozvolu korisnika. Testovi,
conformance, release build. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-25 (korisnik: "da, dodaj ponovno otvaranje kad kartica nestane"; log 15:41: kartica D: je na
oko 2 s nestala, player price javio "Source transport unavailable" i stao do sljedeceg klika): `tools/qnc-player-runner`
(`open_card_media`, `card_away`): lokalni opener playera (klip, zvuk, A1 Synca, prica) otvara medij kartice ponovno svakih
200 ms dok izvor javlja da je nedostupan (`qnc_source_reader::ReadError::Unavailable`), najvise 4 s; medij koji ne postoji,
nije dopusten ili nije datoteka ne ceka se. Pokusaji idu u player log (`card-retry`). Ovisnost `qnc-source-reader` u
programu playera. Otvaranje price ide u pozadinskoj niti dekodera, pa cekanje ne zaustavlja sat; priprema klipa ceka
najvise toliko. Test `a_card_gone_for_a_moment_is_opened_again_a_missing_medium_is_not`, conformance, release build.
Uzivo nije provjereno (kartica nije namjerno vadena).
Popravak 2026-10-25 (korisnik: u Resolveu otvoren XML; "u ediusu neki A2 ne daju ton iako se vidi da ima krivulje"; "izbaci
prijelaze i ostavi ciste rezove"): kanal 2 svih pet pokrivalica ima ton na kartici (izmjereno, -24,5 do -33 dB) i XML
pokazuje na njega, a tihe su bile pokrivalice iza Cross Fade prijelaza. `qnc-fcp7-xml` (`without_overlaps`): preklapanje na
traci je cisti rez na pocetku sljedeceg klipa, bez `<transitionitem>`. Crossfade ostaje u playeru, prerenderu i MXF exportu.
Test, conformance, release build. U Resolveu i Ediusu nije provjereno.
Izvedeno 2026-10-25 (korisnik: u Ediusu se cuje samo treca pokrivalica na A2; probne datoteke bez razine i s razinom u dB
rade u Ediusu; "pokusaj export po pravilu FCP X"): nalaz: Edius FCP7 `Audio Levels` (omjer, 1 = 0 dB) ne cita kao omjer,
pa je cula samo pokrivalica s razinom iznad 1 (+4 dB). Export to XML sada uz `.xml` (FCP7) pise i `.fcpxml` (Final Cut Pro
X, verzija 1.10) iste revizije u mapu exporta: `qnc-fcp7-xml` modul `fcpx.rs` (`fcpxml`): vrijeme u racionalnim sekundama,
razina kao `adjust-volume` u dB, kanal kamere kao `audio-channel-source` (ostali kanali iskljuceni), original kao
`media-rep`; glavna linija je A1 (samo zvuk) s prazninama, V1, V2 i A2 su spojeni klipovi na trakama 1, 2 i -1; klip
koji se koristi samo kao zvuk dobiva velicinu slike sekvence. `tools/qnc-export --xml`, ugovor modula (`export.xml.fcpx`).
Provjereno na izoliranoj kopiji: datoteka je ispravan XML. U Resolveu, Ediusu i Final Cutu nije provjereno.
Izmjena 2026-10-25 (korisnik: FCP X "edius ne podrzava format, resolve razbaca audio kanale na 8 traka i cuje se samo
prvi"; "ok"; u Ediusu se cuju obje probne verzije, bez razine i s razinom u dB, ali "razine ne utjecu na audio"): Export
to XML vise ne pise `.fcpxml` (`qnc-fcp7-xml` modul `fcpx.rs`, `tools/qnc-export`, sposobnost `export.xml.fcpx`
uklonjeni). FCP7 XML vise nema filtar Audio Levels: Edius omjer (1 = 0 dB) cita drukcije i pokrivalice ostaju tihe, a dB
ne primjenjuje; razina ostaje u prici, playeru i MXF exportima, u XML-u se ne prenosi. Svaki kanal datoteke je mono
(`<layout>mono</layout>`, `<channellabel>discrete</channellabel>`), pokusaj da Resolve uzme izabrani kanal kamere
umjesto prvog na oba izlaza. Provjereno na izoliranoj kopiji: XML ispravan, bez filtra razine. U Resolveu i Ediusu nije
provjereno.
Zapis 2026-10-25 (Resolve i kanal kamere; korisnik: "nije sada bitno, glavno da radi u ediusu za sada"): Resolve 21 iz
FCP7 XML-a ne preuzima kanal kamere po klipu ni kad uveze XML koji je sam izvezao (oznaka `trackindex` po klipu i opis
datoteke s 4 ili 16 kanala, s vezama ili bez njih: A2 bez kanala, cuje se kanal 1). Resolveov AAF kanal zapisuje po klipu
(SourceClip na slot kanala izvora), ali uvoz u Resolve nije dovrsen. Export to XML ostaje FCP7 kakav jest (Edius svira
sve pokrivalice); AAF za Resolve je odgoden.
Izmjena 2026-10-25 (korisnik: "preuredi export prozor, dodaj mu tabove PAL i NTSC, pa podijeli popis exporta po grupama;
exporti koji nisu napravljeni, zasivi ih; export XML i Prerender izdvoji iznad tabova PAL i NTSC"): prozor exporta ima
gore Export to XML i Express (prerender), ispod tabove PAL (25 i 50) i NTSC (ostale stope), a u tabu postavke po grupama
Original, XDCAM HD422 (MXF), H.264, DNxHD (Avid), Other; default je prvi u svojoj grupi. Postavka u kojoj se ova prica jos
ne moze izvesti zasivljena je s oznakom "not made yet" i ne moze se izabrati (ni klikom ni strelicama): kodek koji render
ne radi (DNxHD, isprepleteni H.264) ili stopa koja nije stopa price (isprepletena XDCAM i pola stope price). Strelice
gore/dolje biraju, lijevo/desno tab. `qnc-export-choice` (`Standard`, `shown`, `set_tab`, `available`, `group`;
`from_settings` prima stopu price), `qnc-export-box` (tabovi i grupe, `Answer::Tab`), akcija kataloga `export_tab`,
`qnc-program-commands` (`ExportTab`), `qnc-program-segments` (`export_tab`, stopa price iz baze), `qnc-panel-focus`,
`qnc-segment-panel`, rjecnik. Probne XML/AAF datoteke u mapi exporta projekta nisu obrisane: mapa projekta je zasticena
od brisanja (zastita nije dirana). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-25 (korisnik: "drzi se kolor palete koju koristimo i stila koji koristimo, ovo odskace"): prozor exporta
crta se bojama i dijelovima teme aplikacije (`qnc-desktop-theme`): okvir kao info prozor (`qnc-choice-box`), tabovi PAL i
NTSC kao tabovi poola (`qnc-media-pool-head::tab`), Export kao jaki gumb (`strong_button`), Cancel kao chrome gumb,
font teme; vlastite boje (`ExportBoxStyle`) i zaobljeni okviri redaka uklonjeni. `qnc-segment-panel::show` prima temu
aplikacije umjesto teme timelinea.
Zatvoreno ograniceno odobrenje 2026-10-25 (korisnik: "otkljucaj Project da se mapa exporta smije cistiti"): otkljucan
samo `crates/qnc-project-store`. Mapa `exports` projekta dobiva istu dozvolu kao mapa prerendera
(`(OI)(CI)(IO)(DE)` za trenutnog korisnika): sve u njoj (exporti, podmapa `exports/projekti` i datoteke u njoj) smije se
brisati, sama mapa `exports` i ostatak projekta ostaju zasticeni. Postojeci projekti dobivaju dozvolu kad Project otvori
spremiste (provjera bez masovnog ACL posla). Export direktorij izvan `exports` (apsolutni ili drugi relativni) nije
pokriven. Test `the_files_of_the_prerender_cache_may_be_deleted_the_rest_stays_protected` (i export), 42 testa. Project je
ponovno zamrznut.
Otkljucano i izvedeno 2026-10-25 (audit, korisnik "otkljucaj i kreni s 1 do 5"): (1) Glasan miks vise ne gasi player:
zvucni red je uzorak izvan [-1, 1] odbijao kao neispravan blok (`Contract`), sesija je stala do ponovnog otvaranja
(pokrivalica +6 dB na glasnom ambijentu, crossfade blizu punog nivoa). `qnc-audio-output`: uzorci se pri upisu u red
rezu na puni nivo, a izlaz uredaja nakon razina, razine dijela i podloge Synca jos jednom (`clip`); nekonacan uzorak i
dalje je greska. Test `samples_over_full_scale_are_clipped_not_refused`. (2) Probe ciji ishod nikad nije upisan
(radnik ubijen, laptop zaspao) drzao je klip zauvijek bez konacnog zapisa: `qnc-media-record-db` pokusaj bez ishoda
stariji od 2 min (`ABANDONED_AFTER_MS`, probe traje najvise 40 s) ustupa novom; pokusaj koji jos moze trajati i dalje
se ne preuzima. Test. (3) Uvoz "Optimizirani" pisao je `.partial` pa preimenovao, sto zastita projekta ne dopusta:
`qnc-ingest-import-worker` pise odmah pod konacnim imenom. (4) Test `qnc-export-launch` pokretao je pravi export nad
aktivnim projektom: `qnc-export` i `qnc-prerender` rade samo uz `--root` ili `QNC_ROOT` (bez trazenja korijena uz
program); `qnc-export-launch` i `qnc-prerender-launch` predaju korijen aplikacije (`resolve_qnc_root`); test daje
prazan korijen i trazi gresku i da nista nije zapisano. (5) Original export i XML uzimali su velicinu slike iz
reprezentacije koju projekt pusta (proxy): `tools/qnc-export` (`original_picture`) cita je iz spremljenog originala.
Testovi `qnc-audio-output` (23), `qnc-media-record-db` (30), `qnc-ingest-import-worker` (22), `qnc-export-launch`,
`qnc-prerender-launch`; XML na izoliranoj kopiji. Sve ostalo ostaje zamrznuto.
Izvedeno 2026-10-25 (audit, korisnik "nastavi" na sljedece tocke 6-9): (6) Express je mogao pasti usred spajanja kad
prerender za vrijeme exporta obrise dio koji prica vise nema: export dok spaja dijelove prerendera drzi najam
`export_lease` (`qnc-prerender-launch` `EXPORT_LEASE`, `tools/qnc-export`), a `tools/qnc-prerender` dok je on svjez ne
brise stare dijelove (proba kasnije). (7) Neuspjeli export ostavljao je nepotpun MXF pod konacnim imenom, preko dobrog
exporta iste revizije: `qnc-unit-join` pise u radnu datoteku uz izlaz (`<ime>.joining-<proces>-<vrijeme>.mxf`) koja tek
cijela dobiva konacno ime, a pri gresci se brise. (8) Prozor exporta nudio je sto render odbija: Express je dostupan samo za
progresivnu predaju koju render radi u stopi price, a svaka postavka trazi barem onoliko izlaza zvuka koliko ih prica ima
(projektni `audio.channels`; `qnc-export-choice` `renders`, `express_made`, `from_settings(.., outputs)`;
`qnc-source-preview` `story_outputs`, `qnc-program-segments` `open_export`, `qnc-panel-focus`); Original bez stope price ide u
tab stope predaje projekta (prije izmisljenih 50). (9) Pojacanje pokrivalice zaokruzuje se nanize (`qnc-program-loudness`
`cover_gain`), pa vrh nikad ne prelazi granicu. Testovi `qnc-export-choice` (5), `qnc-program-loudness`, `qnc-unit-join`,
`qnc-program-segments` (60), `qnc-panel-focus`, `qnc-source-preview`, conformance; Express na izoliranoj kopiji (12
dijelova, bez zaostale radne datoteke). Uzivo kroz sucelje nije provjereno.
Izvedeno 2026-10-25 (audit, korisnik "nastavi" na tocke 10-13): (10) Klip ciji probe trajno ne uspije drzao je
`final=0`, pa je `tools/qnc-ingest-worker` svake sekunde pokretao novi krug dovrsetka (s posterima i zapisom u log):
radnik pamti zapise koje je krug ostavio nedovrsene i novi krug pokrece tek kad ceka neki drugi klip. (11) Duljine iz
datoteke s kartice vise ne mogu srusiti radnika ni pojesti memoriju: `qnc-mxf-index` citanje vece od 256 MB je greska,
velicina tablice indeksa racuna se bez preljeva; `qnc-mp4-sound` broj uzoraka (najvise 32 M) i velicina uzorka (najvise
64 MB) iz tablice, slika izvan tablice i kratki `esds` su greska ili "nema trake", ne pad. Testovi
`a_length_past_any_file_is_an_error_not_an_allocation`, `a_broken_sample_table_is_no_track_not_an_allocation`.
(12) `qnc-media-readahead`: datoteka koja nije otvorena otvara se izvan zajednicke brave (budenje kartice nije
zaustavljalo sve dekodere), a citanje koje ne uspije pusta vezu na tu datoteku, pa ponovno otvaranje nakon kratkog
nestanka kartice dobiva novu. (13) Prica: nakon svake izmjene korisnicki markeri prije pocetka ili na/iza kraja price
brisu se sa svojim slotovima, a markeri na istoj slici spajaju (kraci lead-out vise ne ostavlja marker i slot iza kraja;
`qnc-program-db` `drop_markers_past_the_story`, `finalize_story`); brisanje segmenta nikad ne izabire lead. Test
`a_shorter_lead_out_takes_the_markers_past_the_new_end_with_it`. Testovi `qnc-program-db` (46), `qnc-program-segments`
(60), `qnc-mp4-sound`, `qnc-mxf-index`, `qnc-mxf-sound`, `qnc-media-readahead`, `qnc-ingest-worker`, conformance. Uzivo
nije provjereno.
Izvedeno 2026-10-25 (audit, korisnik "nastavi dalje"): (1) "Slot pod playheadom" bio je napisan tri puta i na kraju
price se nije slagao (Ctrl+S je nalazio zadnji slot, B i Overwrite ne): svi koriste `qnc-program-view::slot_at`
(zadnji slot vrijedi i na svom kraju). (2) Baza price sama provjerava da pomak M markera ostaje strogo izmedu njegovih
susjeda (`qnc-program-db` `move_marker`; prije je to cuvao samo upravljac, pa je drugi pisac mogao preskociti susjeda i
premjestiti pokrivalicu u drugi raspon); poruka `story.marker_between_neighbours`. (3) `tools/qnc-prerender` postavke
exporta i zvuka citao je jednom pri pokretanju; sada zavrsava kad se promijene (sljedece budenje krece s novima).
(4) Hrvatski tekstovi izvan rjecnika: status ljuske (`shell.app_active`, `shell.app_no_entry`), greska programskog
izlaza (`output.cannot_start`), poruke baze price (`story.purge_only_excluded`, `story.slot_moves_to_neighbour`,
`story.last_slot_ends_with_story`) i nazivi slika prozora reza (`story.still.slot_first`, `story.still.slot_last`).
Testovi `qnc-program-db` (46), `qnc-program-view`, `qnc-program-segments` (60), `qnc-desktop`, `qnc-program-output`,
`qnc-ui-text`, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-25 (audit, korisnik "nastavi sa sitnima"): (1) `tools/qnc-prerender`: dio koji nije uspio (npr. kartica
na trenutak nestala) ponovno se pokusava nakon minute (`RETRY_FAILED`), a ne tek pri novom pokretanju prerendera.
(2) `qnc-player-client`: frame mapa sesije koja se ne otvori brise se odmah (`MapFile`), a mape starije od dana
(aplikacija ugasena silom) brisu se jednom pri prvoj sesiji procesa (`sweep_old_frame_maps`); prije je svako odbijeno
otvaranje ostavljalo ~64 MB u TEMP-u. (3) `tools/qnc-export`: Express ne ceka prerender koji minutu ne napravi nijedan
dio (stoji iza playera ili radi dijelove koje export ne trazi); preostale dijelove renderira sam. Nije izvedeno (trazi
odluku): pomak pokrivalice kod omjera 2:1 i dalje ovisi o broju Entera (pola slike izvora pri neparnom pomaku).
Testovi `qnc-player-client` (18), gradnja `qnc-prerender`, `qnc-export`, conformance. Uzivo nije provjereno.
Popravak 2026-10-25 (korisnik: "na ingestu Cancel i Select su pomaknuti na dolje, podigni ih na sredinu"): traka potvrde
preglednika lokacija bila je visoka tocno kao gumb, a unutra se jos oduzimao rub, pa su Cancel i Select izlazili prema dolje
i bili odrezani. `qnc-dir-browser-panel` `show_with_confirm`: traka je visoka kao gumb plus rub gore i dolje, gumbi su
okomito u sredini. Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-26 (korisnik: "kreni s korakom 1" plana sredivanja arhitekture prije AI modula; korak 1a): radnici
filmstripa i vala imali su zrcalne kopije istog rasporedivaca (red, oznaka gotovih, niti, otkazivanje, prednost otvorenog
klipa, prednost playera, granica broja radnika). Nova javna kocka `crates/qnc-background-jobs` (+ ugovor
`contracts/modules/background-jobs.module.json`): red i radnici jedne vrste posla nad klipovima projekta; otvoreni klip
ide prvi uz ostale (`Preferred::Beside`, filmstrip, posao procesora) ili sam (`Preferred::Alone`, val, posao medija);
dok player svira radi samo otvoreni klip, ostali se otkazuju i vracaju u red; drugi projekt gasi radnike i zaboravlja
gotove. Sama nista ne cita ni ne pise. `qnc-filmstrip-worker` i `qnc-wave-worker` (otkljucani uz korak 1) zadrzavaju
samo svoj posao, citanje baze i predaju piscu; ponasanje isto, osim sto i filmstrip dok player svira premjesta
otvoreni klip na pocetak reda i val i dok player svira pokrene otvoreni klip iz `poll` (kao filmstrip). Testovi
rasporedivaca presli su u kocku (10, ukljucujuci prednost playera, otvoreni klip sam i uz ostale, granicu), testovi
`qnc-filmstrip-worker`, `qnc-wave-worker`, `qnc-timeline-artifacts`, `qnc-content-artifacts`, conformance. Uzivo nije
provjereno. Prepis, scene i vektori slika (AI) bit ce novi poslovi iste kocke.
Izvedeno 2026-10-26 (isti korak 1, dio 1b): pet modula tablica posrednika (`qnc-program-db`, `qnc-virtual-shots`,
`qnc-artifact-db`, `qnc-prerender-db`, `qnc-clip-sound`) imalo je istu kopiju spajanja na posrednika (tvornica,
omotac, pretvorba zahtjeva i odgovora) i istog autorizatora (pise samo svoje tablice, cita sve). Sada je to jednom u
`qnc-db-broker` (`owned.rs`: svojstvo `OwnedTables`, `owned_tables::<M>()`, `write_only_own_tables`); modul daje samo svoj
id, vrste zahtjeva, spajanje (shema) i izvodenje. `qnc-virtual-shots` dobio je popis svojih tablica `OWNED`. Ponasanje
isto. `qnc-media-record-db` i `qnc-source-index-db` imaju drukciji oblik (spajanje kroz `project.rs`) i nisu dirani.
Testovi: dva nova u posredniku (pisanje samo u svoje tablice, modul samo za citanje ne pise ni u svoje), testovi pet
modula, `qnc-program-segments`, `qnc-content-artifacts`, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-26 (korak 2a plana AI; korisnik: "kreni s korakom 2", "ne postuj postojece rjesenje,
predlozi najbolje", "da" na whisper.cpp s Vulkanom kao zaseban program, "da, preuzmi VAD, jezik postavka projekta",
"obvezno da podrzava hrvatski"; Project otkljucan samo za postavku jezika): prepis govora kao dodatak. Ugovor
`contracts/plugins/transcribe.v1.json` (`qnc.transcribe` 1: aplikacija pise 16 kHz mono WAV u praznu privremenu mapu,
program vraca segmente s rijecima i njihovim vremenima u dijelovima, pa jezik; program nista ne pise; backend koji ne
moze pokrenuti je greska). Nova javna kocka `crates/qnc-transcribe-jobs` (+ ugovor modula): strana aplikacije
(`Transcriber`, jedan proces, poslovi redom jer model drzi graficku), strana programa (`serve`, `Engine`) i zvuk koji
ugovor trazi (`to_whisper_rate`: windowed-sinc s ostrim rubom do 7,8 kHz; kratki filtar koji je gusio od ~6 kHz davao
je "Hvala." umjesto govora, izmjereno; `write_wav`, `read_wav`). Nova javna kocka `crates/qnc-ai-catalog` (+ ugovor):
katalog AI motora racunala `catalogs/ai/catalog.json` (program, backend vulkan|cpu, model i VAD model relativno mapi
kataloga, niti; jedan odabran, model koji nedostaje je greska, nema zamjene). Novi program `tools/qnc-transcribe-whisper`
(whisper-rs 0.16 / whisper.cpp; znacajka `vulkan`: Intel, AMD i NVIDIA graficka i procesor; `whisper`: samo procesor;
bez nje javlja da nema motora): model se ucitava jednom, vremena rijeci iz DTW poravnanja za large-v3-turbo, Silero VAD,
beam 5, whisperovi zadani pragovi i ponovni pokusaj na visoj temperaturi (bez njega govor je izlazio kao "Hvala."),
zvuk u komadima do 240 s rezanima na najtisem mjestu, uzastopni isti segmenti se brisu (petlja na ambijentu; kao v5).
`whisper-rs` 0.16 `set_abort_callback_safe` cita svoj closure kao krivi tip i prekida svaki posao, pa se koristi izravni
callback. Gradnja: cmake, Vulkan SDK (`C:\VulkanSDK\1.4.363.0`), LLVM, `--features vulkan`, i kratki `CARGO_TARGET_DIR`
(MSBuild ne prima putanju od 265 znakova u `QNC\target`); exe se kopira uz aplikaciju. Modeli (nisu u gitu):
`data/models/ggml-large-v3-turbo.bin` (1,6 GB), `data/models/ggml-silero-v5.1.2.bin`. Project: postavka `ai.language`
(seed svih sistemskih predlozaka `hr`, i `ai.transcription` false; `qnc-project-store` ih dopunjuje starijem projektu,
test; Advanced sekcija "Transcription", izbor "Language of speech": hr, sr, bs, sl, mk, en, de, it, auto); Project je
ponovno zamrznut. `qnc-work-settings`: `transcription_enabled`, `speech_language`, `transcribe_channel`
(`audio.transcribe_channel` CH1 -> 0), strogo, test. `qnc-installed-versions`/`qnc-versions` pokazuje motor prepisa.
Izmjereno (Mironik 2002 s kartice G:, samo citanje, kanal 1, hrvatski): Intel Iris Xe Vulkan 90 s zvuka za 58 s
(1,55x stvarnog vremena, s ucitavanjem modela), cijeli klip 204 s za oko 136 s; procesor (6 niti) 90 s za 782 s.
Tekst novinara dobar ("Vidite, upravo ovdje u Medugorju otvara svoj teniski centar..."), greske u imenima (Pajevic za
Pavic, Bekera); na tihom kanalu preko ambijenta whisper izmislja kratke recenice ("Oprosti Gula.", "Hvala sto
pratitev."). Testovi `qnc-transcribe-jobs`, `qnc-ai-catalog`, `qnc-work-settings`, `qnc-project-store`,
`qnc-transcribe-whisper`, conformance. Nije izvedeno (2b): pozadinski posao (raspored iz koraka 1, zvuk kanala iz
originala kroz katalog dekodera), tablica prepisa u bazi projekta s pretragom rijeci (FTS5), prikaz; prijevod na
hrvatski (whisper prevodi samo na engleski, treba zaseban model).
Izvedeno 2026-10-26 (korisnik: "transkript bi se pozivao iz forme Media Assist Audio AI... korisnik bi odabrao klip
i odredio IN/OUT, ili virtualni IN/OUT (pocetak i kraj klipa)"; "kreni"; korak 2b, dio 1): grupa e ima desni panel
prepisa i dugme Transcribe u zaglavlju source timelinea (akcija kataloga `transcribe_marks`, bez tipke). Prepisuje se
samo IN-OUT prikazanog klipa (bez oznaka cijeli klip), kanal govora i jezik iz postavki projekta (`ai.transcription`
mora biti ukljucen u Project Advanced), program iz AI kataloga ovog racunala. Retci se pokazuju dok nastaju; gotov
prepis zapisuje se u bazu projekta i odande se cita. Rijec izgovorena na playheadu je osvijetljena, klik na rijec
stavlja playhead na nju (`transcript_cue_word`). Nove javne kocke (s ugovorima): `qnc-sound-range` (zvuk jednog
kanala originala za raspon slika kroz katalog dekodera; premjesteno iz `qnc-fine-wave`, koji ga sada koristi),
`qnc-transcript-db` (modul tablica posrednika: `clip_transcripts`, `public_clip_transcripts`; ugovor baze
`transcripts.database.json`; isti raspon prepisan ponovno zamjenjuje stari), `qnc-transcribe-run` (jedan prepis
raspona klipa: privremeni 16 kHz WAV u praznoj privremenoj mapi, program ostaje ucitan u procesu), `qnc-transcript`
(stanje prepisa prikazanog klipa, citanje i rad izvan niti sucelja), `qnc-transcript-panel` (crtanje), `qnc-right-panel`
(desni panel ploce po vrsti iz ugovora rasporeda: segment_panel, transcript_panel, none). Drzi ih `qnc-panel-focus`
(kao source timeline); ploca Storyja/MA poziva `qnc-right-panel` umjesto segment panela, bez rasta (C10 101).
`StillLocator::settings`. Conformance: grupa e mora imati `transcript_panel`, g i l prazni; `qnc-transcript-db` na
popisu javnih DB vlasnika. Testovi, conformance, release build. Uzivo kroz sucelje nije provjereno. Nije izvedeno
(dio 2): rijeci u trim zoomu oko reza.
Izvedeno 2026-10-26 (korisnik: "kreni s drugim dijelom, zoom"; korak 2b, dio 2): trim zoom (slot, marker, rub, source
IN/OUT) ispod vala A1 pokazuje rijeci izgovorene u prozoru, svaku od mjesta gdje je izgovorena, s crticom pocetka, pa se
vidi i stanka izmedu rijeci; crvena linija reza prolazi i kroz njih. Nova javna kocka `crates/qnc-zoom-words` (+ ugovor
`contracts/modules/zoom-words.module.json`): za svaki dio zvuka koji A1 pusta u prozoru rijeci njegova klipa iz prijepisa
u bazi projekta (`qnc-transcript-db`); dio koji nijedan prijepis ne pokriva prepisuje se jednom, 5 s sa svake strane, u
pozadini (`qnc-transcribe-run`, program ostaje ucitan), zapisuje u bazu i ponovno cita odande; klip ciji prijepis nije
uspio ne pita se ponovno u istom radu; bez ukljucenog prijepisa u projektu nema rijeci. Rijeci su iz kanala govora
projekta. Puni je `qnc-trim-stills` uz fini val (aplikacija nije dirana), crta `qnc-trim-overlay` (`paint_words`);
`qnc-program-view` `ZoomSound.words`, `ZoomWord`; `qnc-still-locator` `reader`. Testovi, conformance, release build.
Uzivo nije provjereno.
Izvedeno 2026-10-26 (korisnik: "minuta traje vjecno, usporio se pojavu playholdera, neupotrebljivo"; "ljudi traze
multitasking, podijeliti na vise jezgri i niti"; "zoom ne prekida pozadinske procese transkripcije, on u zumu radi
transkripciju na CPU; pozadinsku treba usporiti ili pauzirati ako su potrebni resursi za potrebu koju korisnik forsira";
"da, kreni s podjelom 1-3"): log je pokazao da whisper na istoj grafickoj (Iris Xe) kao player ne ustupa playeru (svaki
Ctrl+I u zoomu pokretao ga je sam; dekodiranje unatrag istek 10 s, player se prestao javljati), uz istodobni uvoz novog
projekta i prerender. Izmjereno (Mironik 2002, 16 kHz): whisper q5 na procesoru neupotrebljiv (12 s zvuka za 363 s na 6
niti), na grafickoj 60 s za 36 s; NVIDIA Parakeet TDT 0.6B v3 int8 kroz sherpa-onnx na procesoru 12 s za 1,5 s i 60 s za
8,8 s na 2 niti, bez izmisljanja na tisini; tekst slabiji (imena, nema velikog Đ). Podjela: (1) zoom (`qnc-zoom-words`)
prepisuje na procesoru Parakeetom i ne dira pozadinu; (2) pozadinski prijepis (MA Transcribe) ostaje whisper na
grafickoj, u komadima od 30 s: prije svakog ceka dok player svira ili korisnik montira (`qnc-playback-activity` razina iz
baze projekta), a komad koji korisnik prekine radom playera zaustavlja se i ponavlja kad player miruje
(`qnc-transcribe-run` `Purpose`, svaki program drzi svoj model, rade istodobno); (3) novi program
`tools/qnc-transcribe-parakeet` (ugovor `qnc.transcribe` 1; sherpa-onnx C API v2 ucitan iz `sherpa-onnx-c-api.dll` uz
program, bez nje kontrolirana greska; zvuk u prozorima do 30 s rezanima na najtisem mjestu, rijeci iz tokena, recenice po
interpunkciji ili stanci). AI katalog (`qnc-ai-catalog`): `transcribe.zoom` imenuje motor zooma (bez njega zoom nema motor,
nista ne pada na drugi), model smije biti mapa. Datoteke izvan gita: `data/models/parakeet-tdt-0.6b-v3-int8`,
`data/models/ggml-large-v3-turbo-q5_0.bin` (izmjeren, nije u katalogu), DLL-ovi sherpa-onnx 2.0.0 i onnxruntime uz program
(beta paket ih mora ukljuciti). Probni program `crates/qnc-transcribe-jobs/examples/transcribe_wav.rs`. Testovi
(`qnc-ai-catalog`, `qnc-transcribe-parakeet`, `qnc-transcribe-run`, `qnc-zoom-words`, `qnc-transcript`), conformance.
Uzivo nije provjereno.
Dopuna 2026-10-26 (korisnik: "popis imena" kroz sherpa-onnx: snaga 1,5-5 ne mijenja tekst, 6-10 ga raspada, Parakeet TDT
nije za to; "selektiram klipove koje treba transcribe, dok se to radi u pozadini ili ceka, u zoomu mogu imati pomoc za
rezanje izjava"; motor "postavka uz gumb"): red prijepisa. Klipovi oznaceni kvacicom u poolu MA Audio AI (grupa e,
`ticks_several` u `editorial.layout.json`; kvacice su samo izbor na ekranu do Transcribe) ili bez kvacica IN/OUT
prikazanog klipa stavljaju se u red u bazi projekta (`qnc-transcript-db`: tablica `transcript_queue`, pogled
`public_transcript_queue`; klip, raspon, `fast`|`best`, stanje waiting|running|done|failed, greska), s izborom Fast
(Parakeet, procesor) ili Quality (whisper) uz gumb Transcribe (akcije kataloga `transcribe_fast`, `transcribe_best`,
`transcribe_mark`). Nova javna kocka `crates/qnc-transcribe-queue` (+ ugovor): jedan izvrsitelj reda po projektu (najam
`transcribe_lease` u bazi), klip po klip, svaki u komadima koji cekaju dok player svira ili korisnik montira; klip koji
je ostao `running` od prekinutog rada ceka ponovno. `qnc-transcribe-run` (`Quality` Best|Fast, `Pace` Now|YieldToPlayer),
`qnc-ai-catalog` (`Quality`, kljuc `fast` umjesto `zoom`), `qnc-transcript` (kvacice, red, prekidac, citanje reda svake
sekunde, pokretanje izvrsitelja kad nesto ceka), `qnc-transcript-panel` (red s nazivom, motorom i stanjem iznad teksta),
`qnc-panel-focus` (poll prima klipove poola), Story/MA ploca i aplikacija (C10 spusteno: aplikacija 525, ploca 100).
Zoom (Parakeet, odmah) radi neovisno o redu. Testovi (`qnc-transcript-db` red, `qnc-ai-catalog`, `qnc-transcript`,
zlatna tablica tipki), conformance. Uzivo nije provjereno.
Popravak 2026-10-26 (korisnik nakon probe: "whisper je po ovom sto vidim neupotrebljiv"; "da, kreni s 1-3, Quality makni
za sada"): log i red u bazi: Fast 185 s zvuka za 66 s, Quality jednom pao ("the plug-in ended", greska nije zapisana pa je
klip oboren), Quality na buci i djecjem govoru izmislja i ponavlja, a rez na fiksnih 30 s sijece recenice; svaki dodir
playera bacao je cijeli komad. Izvedeno: (1) Fast (Parakeet) je jedini motor reda; prekidac Fast | Quality uklonjen s
rasporeda grupe e (kod i akcije kataloga ostaju). (2) Red cita zvuk klipa jednom od pocetka medija
(`qnc-sound-range::read_from_start`): frame-wrapped MXF s nekomprimiranim mono kanalom samo kroz zvucne elemente
(`qnc-mxf-sound::read_element_frames`; pravilo "k-ti audio stream je element k" premjesteno iz `qnc-wave-worker` u
`qnc-mxf-sound::sound_elements`), ostalo kroz katalog dekodera; izmjereno s kartice G: cijeli zvuk Mironik 2679 (224 s,
1,6 GB) za 25,7 s. (3) Zvuk se reze na komade do 30 s na najtisem mjestu (`qnc-transcribe-jobs::quiet_pieces`, koristi ga
i program Parakeet); komad koji korisnik prekine radom playera ponavlja se sam, gotovi komadi ostaju; program koji se
ugasi u komadu pokrece se ponovno jednom, greska se zapisuje u log. Zoom i dalje cita nekoliko sekundi kroz `read`.
Probni programi `qnc-mxf-sound/examples/element_read.rs`, `qnc-transcript-db/examples/queue.rs` (samo citanje).
Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-26 (korisnik: "praznine ne prikazuj u tekstu, umjesto njih napisi crvenim slovima audio bez teksta od
timecode - timecode"; "za transcribe po fajlu duljina IN/OUT, progres bar, koliko vec traje, i na kraju koliko puta brze od
realnog vremena"): panel prepisa izmedu redaka crveno pise "Audio without text <TC> – <TC>" za svaki dio prepisanog zvuka
bez rijeci od 2 s ili dulji (timecode izvora prikazanog klipa; `qnc-transcript` `silences`, `TranscriptView.silences`;
dok prepis radi, samo do mjesta do kojeg je stigao). Red prepisa pokazuje sve klipove: naziv, duljinu IN/OUT, Fast,
stanje; klip koji se prepisuje traku napretka (citanje zvuka, pa udio prepisanih sekundi) i koliko vec radi; gotov koliko
je trajao i koliko puta brze od stvarnog vremena. `qnc-transcribe-run` javlja napredak (`Progress`), `qnc-transcribe-queue`
ga prosljeduje (`Heard::Progress`) i upisuje trajanje gotovog klipa (`qnc-transcript-db` `took_ms`, stupac dodan starijoj
tablici reda), `qnc-right-panel` prosljeduje timecode izvora panelu. Testovi, conformance, release build. Uzivo nije provjereno.
Popravak 2026-10-26 (korisnik: "stvarna brzina u prosjeku manja od 2x, razmak izmedu tisina ogroman, recenice izlaze iz
panela, trebaju biti u wrap modu"; "puno toga nije transcribirano"; "da cita samo A1 kanal, da ton ispod neke razine
preskace"): (1) Retci prijepisa su jedan tekst prelomljen na sirinu panela (`qnc-transcript-panel`, LayoutJob; klik na
rijec i osvijetljena rijec kao prije). (2) Log: citanje zvuka klipa s kartice pocinjalo je ispocetka pri svakom dodiru
playera (do 7 puta po klipu); sada se nastavlja od zadnje procitane slike (`qnc-mxf-sound` `read_element_frames_into`,
`qnc-sound-range` `read_from_start_kept`). (3) Motoru ide samo zvuk kanala govora projekta (vec tako, CH1) glasniji od
-42 dBFS (RMS desetinke sekunde, prosiren 1/3 s, spojen preko stanki krocih od 1 s; `qnc-transcribe-jobs` `loud_spans`,
`qnc-transcribe-run` `SILENCE_FLOOR_DBFS`), log `transcribe background ... Ns of Ms over -42 dBFS`. Nalazi: motor sam
(Parakeet, 2-6 niti) 8-10x stvarnog vremena na 120 s zvuka; Mironik 2002 i 2679 imaju ispod -42 dBFS samo 3 s tisine (ambijent
je glasniji), pa granica tamo malo skida. Rupe u tekstu nisu tisina: na 2679 118-148 s Parakeet daje "Okay, I think it's an
evil.", whisper na istom zvuku hrvatski govor (djeca, dijalekt, buka); Parakeet ne moze dobiti zadani jezik. Testovi,
conformance, release build. Uzivo nije provjereno.
Dopuna 2026-10-26 (korisnik: "neki tekst se ponavlja; radis li transkript globalno ili po projektu, kako se baza ponasa ako se
vise puta napravi transcribe"; "dvostruki klik za IN/OUT markere; IN je manji timecode"): (1) Prijepis je po projektu
(`clip_transcripts` u `project.db`). Isti raspon klipa ponovno prepisan zamjenjuje stari; drugi raspon koji se preklapa
zadrzavao je stare retke, pa se tekst ponavljao (klip s prijepisom cijelog klipa i kasnijim 644..4394). Sada je najnoviji
prijepis ono sto je receno u svom rasponu: stariji prijepis klipa gubi retke ciji je srednji frame u novom rasponu, a
prijepis ciji je raspon unutar novog se brise, u istoj transakciji (`qnc-transcript-db` `save`, test). Postojeci dupli redovi
nestaju kad se taj raspon ponovno prepise. (2) Dvoklik na source timelineu (sve forme) je rez na kliknutoj slici, kao
srednja tipka: prvi je IN, drugi oznacava razmak (manji frame IN, veci OUT), treci pocinje novi par. `qnc-timeline`
(`TimelineIntent::CutAt`, `with_first_cut_at`, `with_second_cut`), `qnc-mouse-map` (`double_click_frame`),
`contracts/input/mouse.json` (source_timeline `double_click_frame: source_cut`), `qnc-source-timeline` (`cut` prima sliku),
`qnc-panel-focus`. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-26 (korisnik: "nisam zadovoljan odzivom playheada na source timelineu u MA AI ni nacinom transcribe; treba
naci nacin da se ubrza"): log: dok prijepis radi u pozadini, sucelje MA AI crta 61 put u sekundi (450-570 ms procesora u
sekundi; slaganje redaka prijepisa i kopija prikaza pri svakom crtanju), a prijepis je stajao pri svakom dodiru playera
(citanje zvuka 5-7 puta prekinuto po klipu). Izvedeno: (1) dok radi samo pozadinski prijepis forma crta 4 puta u sekundi
(`qnc-transcript` `working`, `qnc-panel-focus` `working`, `qnc-editorial-application` `next_repaint_delay`); retci i tisine
prijepisa slazu se samo kad se promijene (`rev`, `Arc`). (2) Program Parakeet radi ispod normalnog prioriteta procesa
(`tools/qnc-transcribe-parakeet`), pa player (iznad normalnog) i sucelje uvijek dobivaju procesor; zato pozadinski prijepis
i citanje zvuka staju samo dok player svira, ne i dok korisnik montira (`qnc-transcribe-run` `while_idle`, `wait_idle`).
Testovi, conformance, release build. Uzivo nije provjereno.
Dopuna 2026-10-26 (korisnik: "dvoklik na tekst rijeci ne kreira marker; ako ne moze dvoklik, misem ili touchpadom, sto
moze"; "u zoomu rade samo strelice, ne radi kotacic ni touchpad"): (1) Dvoklik na rijec u panelu prijepisa (mis ili dvostruki
dodir touchpada) je rez na toj rijeci (akcija kataloga `transcript_cut_word`, bez tipke): prvi je IN na pocetku rijeci,
rijec iza prvog reza OUT na svom kraju, ranija IN na svom pocetku; par drzi source timeline kao za skare
(`qnc-source-timeline` `cut_word`, test; `qnc-transcript-panel`, `qnc-panel-focus`). Prvi klik dvoklika stavlja playhead na
rijec kao i prije. (2) Kotacic i touchpad nad prozorom trim zooma rade isto sto nad panelom na kojem je uzeto (source
timeline ili Wrap redovi): `qnc-trim-overlay` vraca radnju kotacica tog panela, ploca je predaje kao i druge radnje (C10
isti). Testovi, conformance, release build. Uzivo nije provjereno.
Ispravak 2026-10-26 (korisnik: "sto je IN, a sto OUT odreduje se na osnovi timecodea, manji broj je IN"): dvoklik na dvije
rijeci uvijek oznacava obje cijele, bez obzira na redoslijed: IN na pocetku ranije rijeci (manji timecode), OUT na kraju kasnije
(`qnc-source-timeline` `cut_word` pamti prvu rijec cijelu, test u oba redoslijeda). Skare (srednja tipka, dvoklik na source
timelineu) vec su tako radile. Conformance nije pokretan za ovu izmjenu.
Dopuna 2026-10-26 (korisnik: "tekst koji se pojavljuje u zoomu trebao bi imati mogucnost dvoklika za marker, kao i tekst u MA
AI"): u trim zoomu source timelinea dvoklik na rijec ispod A1 (`qnc-trim-overlay` `paint_words`; prozor zooma sada prima
klik) salje `transcript_cut_word`. Dok je u zoomu uzet IN, rijec ga pomice na svoj pocetak, uzet OUT na svoj kraj, oznaka
ostaje uzeta (`qnc-panel-focus`, `SourceTimeline::drag`); bez uzete oznake je rez kao na tekstu prijepisa. U zoomu Wrap redova
(slot, marker) dvoklik na rijec ne radi nista (os je program, ne klip). Testovi, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-26 (log: Express i H.264 export dali su A1 -31,6 LUFS s pojacanjem samo +5,5 dB, jer je vrh
-6,5 dBTP zaustavio pojacanje na granici -1 dBTP, pa je A1 izvezen na oko -26 LUFS; korisnik: "da, otkljucaj i napravi
limiter"; otkljucani `qnc-loudness` i `qnc-unit-join`): export dovodi A1 i A2 do cilja, a vrhove koji bi presli granicu
drzi limiter. `qnc-loudness` `limited_gain` (`Lift`: pojacanje do cilja, nista unutar tolerancije, najvise
`EXPORT_GAIN_MAX_DB` 20 dB; `limit` kad bi vrh uz to pojacanje presao granicu); `export_gain` ostaje za pokrivalice.
`qnc-unit-join`: traka s limiterom ide kroz ffmpeg `aresample=192000,alimiter=...:level=false:latency=true,aresample=<rate>`
(cetiri puta brzina, kao true peak BS.1770; strop 0,5 dB ispod granice), zapisana datoteka se mjeri ponovno i, ako je vrh
jos iznad granice, pise jos jednom sa stropom nizim za razliku; `LaneOutcome` nosi `limited`, `written_lufs`,
`written_dbtp`, a `tools/qnc-export` ih pise u player log. Probni program `crates/qnc-unit-join/examples/join_units.rs`
(cita samo dijelove). Provjereno na kopiji dijelova prerendera projekta `novi-ukupni_1` u scratchpadu: A1 -31,6 -> -23,0
LUFS, -1,5 dBTP (ffmpeg ebur128 isto), A2 -23,6 -> -23,0; 833280 uzoraka = 868 slika x 960; razlika izlaza i ulaza
najmanja bez pomaka (-55,8 dB, pomak od 1 uzorka -44,8 dB), dakle zvuk ostaje uz sliku. Testovi, conformance. Kroz
sucelje nije provjereno.
Otkljucano i izvedeno 2026-10-26 (korisnik: "preciznost nije najbolja.. na ovom primjeru cijela rijec"; "da, otkljucaj i
kreni"): nalaz: rijec nije izgubljena u zoomu nego u prijepisu (Parakeet na istom zvuku tri puta: "Pa dosta njih(rača)",
bez rijeci na rezu; vremena rijeci na korak od 80 ms, kraj rijeci cesto prekratak). Nova javna kocka
`crates/qnc-word-align` (+ ugovor `contracts/modules/word-align.module.json`; cista, nista ne cita ni pise): rubovi rijeci
pomaknuti na zvuk ispod njih (pocetak natrag do mjesta gdje glas naraste, najvise 6 slika i nikad prije prethodne rijeci;
kraj dalje dok glas ne padne, nikad u sljedecu rijec; tisina na rubu se odreze do 6 slika); glas je 6 dB iznad tihe petine
prozora, nikad ispod -42 dBFS; glas koji nijedna rijec ne pokriva (najmanje 4 slike) vraca se zasebno. `qnc-zoom-words`
(`fill(.., fine)`) ga primjenjuje samo kad je fini val A1 cijeli procitan (`qnc-trim-stills`), `qnc-program-view`
`ZoomSound.unheard`, `qnc-trim-overlay` crta takav glas crveno s "?" u retku rijeci. Prijepis u bazi se ne mijenja.
Testovi `qnc-word-align` (4), `qnc-zoom-words`, `qnc-trim-overlay`, `qnc-trim-stills`, `qnc-program-view`, conformance.
Uzivo nije provjereno.
Ispravak isti dan (korisnik: "rijec dosad pocinje zvucno tek na mjestu crvenog reza"; "potrebno je da rijeci budu na mjestu
tona kako bi se tekst mogao koristiti za rezanje izjava"): izmjereno na zvuku kartice (samo citanje, 2680 kanal 1, RMS po
slici): "Pa" pocinje na 865, "dosad" na 873, "njih" na 881, a Parakeet ih daje na 856, 868, 884 (5-9 slika ranije ili kasnije);
prvo pravilo je pocetak jos pomicalo unatrag u dah. `qnc-word-align` sada: pocetak svake rijeci ide na najblizi pocetak glasa
(porast iz tisine ili iz udubine za 9 dB, najvise 10 slika daleko, redom, bez preskakanja), kraj gdje njezin glas padne (kroz
udubine do 5 slika, nikad u sljedecu rijec); glas je i najvise 18 dB ispod glasnog dijela prozora, pa dah i prostorija
izmedu rijeci nisu glas. Test na izmjerenim vrijednostima tog mjesta. Uzivo nije provjereno.
Ispravak isti dan (korisnik: "i dalje krivo... gdje pise dosad tu je Pa, dosad je desno na mjestu crvenog playheada"; "ili
ti je pomaknut wave prikaz"): wave nije pomaknut (vrhovi slika procitani s kartice ffmpegom: glas "Pa" od 864, "dosad" od
873, isto mjesto kao val zooma); najblize mjesto po rijeci (pohlepno) dalo je "dosad" pocetak "Pa" (865), a "Pa" dahu.
`qnc-word-align`: pocetak glasa mora trajati (3 od 4 slike glasne), porasti 2 slike blizu su jedan pocetak; rijeci se na
pocetke slazu redom s najmanje ukupnog pomicanja (`assign`, rijec bez pocetka zadrzava vrijeme motora uz kaznu vecu od
dosega), pa jedna rijec ne uzima pocetak koji pripada drugoj. Test na izmjerenim vrhovima 2680 (slike 820-930): 864, 873,
881. Uzivo nije provjereno.
Popravak 2026-10-26 (korisnik: "odabrao sam tri klipa za transkript, pogledaj log"): sva tri su odmah pala (ms=0) jer je u
projektu prijepis iskljucen (`ai.transcription` false), a log nije rekao zasto. `qnc-transcript` (`start`): Transcribe s
iskljucenim prijepisom odmah pise `transcribe.off_in_project` i nista ne stavlja u red; `qnc-transcribe-queue` u logu uz
ishod pise i gresku. Testovi `qnc-transcript`, `qnc-transcribe-queue`.
Otkljucano i izvedeno 2026-10-27 (korisnik: "pojava playheada je u source timelineu spora"; "trebamo urediti MA AI transcribe
panel ... popisu transkribiranih klipova dodati link ... oznaceni IN/OUT teksta kopira se u listu odabranog teksta s pocetnim
i zavrsnim timecodeom i duljinom ... baza koju bi novinar mogao koristiti za kopiranje u skript"; "da, otkljucaj i kreni s
oba"): (1) Player (otkljucan `qnc-broadcast-engine`): log je pokazao da sesija pada oko 3 s nakon "spreman": u pauzi player
ponovno otvara dekoder slike (ffmpeg na kartici koja se budi) na svojoj niti, ne odgovara dulje od 2 s, klijent javlja
"control read header timed out" i sljedeci klik trazi novu sesiju (0,9-1,4 s do spremnosti; 7 puta 26. 10., i 25. 10.). Sada
klip kojem otvoreni dekoder ne moze skociti otvara novi u pozadinskoj niti (`Opening`, kao kod price); dok se otvara player
odgovara, slika ceka kao kod dekodiranja. Testovi `qnc-broadcast-engine`. Uzivo nije provjereno. (2) Panel prijepisa:
transkribirani klip u popisu je link (`transcript_open_clip`): otvara klip na previewu i njegov tekst; rijeci izmedu IN i OUT
podcrtane; "Statements": "Add IN-OUT" (`transcript_add_quote`) sprema rijeci izmedu IN i OUT prikazanog klipa s pocetnim i
zavrsnim timecodeom i duljinom (kako se prikazuju) u novu tablicu `transcript_quotes` baze projekta (`qnc-transcript-db`:
`AddQuote`, `DeleteQuote`, `ReadQuotes`, pogled `public_transcript_quotes`; ugovor baze `transcripts.database.json`); tablica
Start | End | Duration | Clip | Text, klik na red otvara klip s IN/OUT izjave (`transcript_open_quote`, kroz isti put kao Edit
segmenta), X brise (`transcript_delete_quote`), "Copy" stavlja tablicu u medjuspremnik (red po izjavi, stupci odvojeni
tabom). `qnc-transcript`, `qnc-transcript-panel` (`quotes.rs`), `qnc-panel-focus`, `qnc-editorial-application` (jedan redak
izmijenjen, C10 isti), rjecnik, katalog akcija (bez tipki). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-27 (korisnik: "copy prozor treba biti sirok kao i transcribe"): svaka izjava u Statements je red preko cijele
sirine panela: gore timecodeovi, duljina, klip i X, ispod tekst prelomljen kao u prijepisu (`qnc-transcript-panel` `quotes.rs`).
Otkljucano i izvedeno 2026-10-27 (korisnik: "na vrhu desnog panela dva taba Audio AI i Video AI"; "slazem se za naziv, moze i
AI Media Assist"; analiza kadrova kao zaseban pozadinski posao nakon filmstripa, ne u istom citanju): grupa e zove se
"AI Media Assist" (`apps/qnc-media-assist-audio-ai/qnc-app.json`, ugovor aplikacije, naslov prozora; id-evi isti; katalog
aplikacija osvjezen alatom `qnc-app-catalog refresh`). Desni panel grupe smije imati tabove na vrhu (`right_panel_tabs` u
`contracts/ui/editorial.layout.json`: naziv, akcija kataloga bez tipke, vrsta panela; `qnc-editorial-layout` `RightPanelTab`);
crta ih `qnc-right-panel` `show_tabbed` istim izgledom kao tabove poola (`qnc-pool-tabs-panel`), a otvoreni tab je izbor
prikaza ovog procesa (memorija sucelja, nista se ne pise). Grupa e: Audio AI (prijepis) i Video AI (`video_ai_panel`, zasad
poruka `ai.video_none`). Akcije `ai_tab_audio`, `ai_tab_video` u katalogu bez tipke; tipke za tabove i krug fokusa nisu
izvedeni. Forma nije narasla (C10 ploce isti). Testovi, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-27 (korisnik: "da, otkljucaj Project i kreni s analizom kadrova"): Project nije trebalo otkljucati: postavka
postoji, Advanced kvacica "AI shot analysis and SubClips" (`ai.enabled`, seed false, `qnc-work-settings` `ai_enabled`), pa je
ona prekidac analize. Analiza kadrova je zaseban pozadinski posao poslije filmstripa i vala: `tools/qnc-ingest-worker`
(`make_shots`) je pokrece na kraju svog rada (otvaranje projekta, Select, Uvezi), ceka dok player svira, klip otvoren u
previewu ide prvi. Nove kocke (s ugovorima modula): `qnc-shot-detect` (cisto: male sive slike -> kadrovi; promjena scene je
skok histograma svjetline daleko iznad okolnih, kadar kraci od 2 slike je bljesak i spaja se; izgled black, soft, moving,
still i najostrija slika kadra), `qnc-shot-db` (modul tablica posrednika, `clip_shots` / `public_clip_shots`, ugovor baze
`shots.database.json`; rupa izmedju kadrova ili greska uz kadrove se odbija), `qnc-shot-analysis` (uvezeni klipovi bez
analize: jedna intra slika originala oko svake sekunde kroz indeks MXF-a, kartica samo citana, dekodirane 160x90 kroz katalog
dekodera po 16, upis pod najmom radnika; original koji nije na racunalu ceka sljedeci rad, original bez indeksa ili drugog
kodeka zapisuje se s razlogom), `qnc-clip-shots` (analiza klipa na previewu citana izvan niti sucelja, ponovno svake 3 s dok je
nema), `qnc-shot-panel` (Video AI tab: kadrovi s TC-om i izgledom, kadar pod playheadom osvijetljen, klik = `shot_cue`,
playhead na pocetak kadra). Drzi ih `qnc-panel-focus`, crta `qnc-right-panel`; ploca isti broj redaka. Izmjereno na kartici G:
(samo citanje, probni program `qnc-shot-analysis/examples/analyse_file.rs`): Mironik 2002 (3:24) 204 slike za 21,9 s, 1 kadar;
2679 (3:44) 224 slike za 20,8 s, 3 kadra (rez na 6601 provjeren slikom: razgovor -> tribina); 1506 158 slika za 12,7 s,
1 kadar. Granice (black, soft, moving, still) su prva procjena i trebaju zivu provjeru. Nije izvedeno: MP4 i ostali formati
bez indeksa MXF-a, kadar u Wrapu/Storyju, trazenje po slici. Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-10-27 (korisnik nakon probe: "to nije analiza i ne vidi neke subklipove"; log: svi uvezeni klipovi analizirani
bez greske, ali skoro svaki 1 kadar, jer klip kamere je jedno snimanje bez rezova; "da, kreni s 1, pa nastavi do 5"):
analiza daje upotrebljive dijelove snimanja. (1) `qnc-shot-analysis` cita svaku intra sliku originala (najvise 4 u sekundi;
long GOP kamere oko 2 u sekundi, prije 1 u sekundi). (2) `qnc-shot-detect` `parts.rs`: pokret kamere izmedu susjednih slika
je pomak cijele slike (`camera_move`: trazenje na pola velicine, pa piksel na punoj); mirno do 0,04 sirine slike u sekundi,
jednoliki svenk do 0,5 i susjedni pomaci slicni, brze je trzaj ili brzi svenk. (3) Dio je niz mirnih ili jednolikih parova
unutar jednog kadra, ostar i nije crn, najmanje 3 s (`PART_LEAST_SECONDS`), s najostrijom slikom. (4) `qnc-shot-db`:
`PartRecord` (from, to, camera steady|pan, key_frame), stupci `parts_json` i `version` (`ANALYSIS_VERSION` 2); tablica
ranijeg oblika bez dijelova se brise pa se klipovi analiziraju ponovno, red druge verzije se ne cita. (5) Video AI panel
(`qnc-shot-panel`) prvo pokazuje dijelove (TC, mirna kamera / jednoliki svenk), ispod kadrove; klik na dio
(`shot_open_part`, akcija kataloga bez tipke) otvara klip s IN/OUT dijela na source timelineu (`qnc-clip-shots`
`open_part`, `take_source_request`, isti put kao izjava prijepisa), pa ga Add SubClip / Shift+S sprema kao SubClip s
posterom. SubClipovi se ne prave sami. Izmjereno na kartici G: (samo citanje): 2679 466 slika 48 s, 11 dijelova; 2002 425
slika 62 s, 8 dijelova (stand-up 0-1681 miran, odlazak novinara izbacen, svenkovi po tribini 2065-2257 i 2617-2881,
provjereno slikama); 1506 328 slika 54 s, 3 dijela. Analiza je oko 2 puta sporija nego s jednom slikom u sekundi. Testovi
`qnc-shot-detect` (pomak, dio miran, jednoliki svenk, trzaj), `qnc-shot-db` (dijelovi, stara tablica), conformance. Uzivo nije
provjereno.
Dopuna 2026-10-27 (korisnik: "analiziranim dijelovima dodamo boju okvira kao sto imamo za duzinu trajanja pokrivalica u
Storyju"): svaki upotrebljivi dio u Video AI panelu ima okvir u boji svoje duljine i duljinu `sekunde:slike`, istim
pravilom i granicama kao pokrivalica u Storyju (`qnc-segment-panel` `cover_fill`, `cover_lengths` iz
`contracts/ui/editorial.layout.json`); stopa iz timecodea klipa. `qnc-shot-panel` (ovisi o `qnc-segment-panel` i
`qnc-program-view`, bez kopije pravila), `qnc-right-panel` predaje granice. Conformance. Uzivo nije provjereno.
Popravak 2026-10-27 (korisnik: "zasto se ceka pocetak transcripta na klipu od 3,29 skoro 40 sekundi"; "da, otkljucaj i
napravi"; otkljucani `qnc-transcribe-run`, `qnc-transcribe-queue`, uz njih nuzno `qnc-sound-range` i `qnc-mxf-sound`):
log (Mironik 2002, 203,9 s): citanje cijelog zvuka s kartice 35 s, motor 23,6 s; prvi tekst je cekao kraj citanja, a karticu
je istodobno citao i radnik vala za isti klip. Sada red prepisa cita zvuk u jednoj niti i daje ga motoru dok se cita:
komad cim je procitano 35 s zvuka iza njega (rez na najtisem mjestu zadnjih 5 s, najvise 30 s), ostatak kad je citanje gotovo;
citanje i prepis i dalje stoje dok player svira, prekinuto citanje nastavlja od procitanih slika (`qnc-transcribe-run`
`read_sound`, `hear_while_read`, `hear`). `qnc-sound-range` `stream_from_start` (MXF zvuk okvir po okvir, ostali mediji kroz
katalog dekodera odjednom) zamjenjuje `read_from_start` i `read_from_start_kept`; `qnc-mxf-sound` `read_element_frames_each`.
`qnc-transcribe-queue` nije trebalo mijenjati. Testovi, conformance. Uzivo nije provjereno. Nije izvedeno: radnik vala i
prepis i dalje mogu istodobno citati isti klip s kartice.
Popravak 2026-10-27 (korisnik: "kadrovi koje napravi, kad se na njih klikne, trebali bi playholder prebaciti na IN tog kadra"):
klik na upotrebljivi dio u Video AI panelu (i na izjavu prijepisa, Edit segmenta) otvara klip s IN/OUT; kad je taj klip vec na
previewu, preview ga ne otvara ponovno pa je playhead ostajao gdje je bio. Sada playhead tada ide na IN (`qnc-editorial-application`
`open_marked`: `open_at` ili `cue`). C10 Story aplikacija 525 -> 524. Testovi, conformance. Uzivo nije provjereno.
Popravak i izmjena 2026-10-27 (korisnik: "pogledaj log... zasto je kompletan tekst u copy prozoru, treba dodati brisanje odabranog i da prikazuje samo odabrano s timecodeom: timecode, ispod tekst, ispod end timecode"): (1) Log: sedam klipova
reda prijepisa palo je s `db.transcript_invalid`: u njima nema zvuka iznad -42 dBFS, motor nije nista cuo pa jezik nije bio
zadan. Prijepis bez cutog zvuka dobiva jezik iz projekta (`qnc-transcribe-run`). (2) Izjava u panelu prijepisa: klip i trajanje
s dugmetom Delete (prije znak koji font nema), ispod pocetni timecode, izgovoreni tekst, zavrsni timecode
(`qnc-transcript-panel`, rjecnik `transcript.quote_remove`); Copy daje isto: pocetni TC, tekst, zavrsni TC, prazan red
izmedu izjava (`qnc-transcript` `quotes_text`, test). Izjava bez OUT-a ide do kraja klipa (pravilo 17. 10.). Testovi, conformance.
Popravak 2026-10-27 (korisnik: "brzina transkode je pokvarena"): uzrok nije izmjena prijepisa nego kartica: pozadinski radnik je radio
analizu kadrova (Video AI, svaka intra slika svakog klipa, pet ffmpega) dok je red prijepisa citao zvuk s iste kartice;
izmjereno: zvuk MXF-a od 9 s s kartice 4,5-7,3 s (iz memorije 0,01 s). Analiza kadrova sada ceka i dok radi prijepis koji je
korisnik pokrenuo (`tools/qnc-ingest-worker`: najam `qnc-transcribe-queue::LEASE` u bazi, kao za player). Log prijepisa
pise koliko je trajalo citanje, motor i cekanje na citanje (`qnc-transcribe-run`). Testovi, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-27 (korisnik: plan pisanja priloga od prijepisa do grubo slozene price; "da, otkljucavam
Project, snimanje offa da"; "project forma ima tehnicki dio i kreativni dio"; "sve na jednom panelu: gore podaci za prilog,
na kraju izbor tehnickog templatea; sve drugo tek kad se otvori Advanced"; "smanji projekt formu, prebaci u kocke"):
(1) Podaci priloga: nova tablica baze projekta `project_story_info` i pogled `public_project_story_info` (slug, emisija,
format vtr|vo|sot, emitiranje, zadano trajanje u sekundama, novinar, snimatelj, montazer; vlasnik Project,
`qnc-project-store` `story_info.rs`, `create_project_with_story`, `story_info`, `save_story_info`; novinar bez upisa je
korisnik racunala iz zapisa porijekla), ugovor `project-workspace.database.json`. Pozadinski poslovi `LoadStory`,
`SaveStory`, `Create` nosi podatke (`qnc-project-jobs`). (2) Project panel: gore sekcija Story (naziv projekta i polja iz
ugovora `story_info`, crta ih `qnc-settings-form`: nova vrsta polja `text` i `duration` m:ss, `FieldSection`), zatim
template, zatim Create (novi projekt) ili Save i New project (otvoreni projekt; sesija drzi podatke otvorenog projekta,
`story_draft`, `story_project`); AI, lokacije, template akcije, Advanced sekcije i vlastiti template samo pod otvorenim
Advanced (`pts_slots`, conformance Project pravilo prilagodeno). Akcije kataloga `project_story_save`, `project_new_draft`.
(3) Forma u kocke: nova kocka `crates/qnc-location-field` (+ ugovor; polje lokacije s preglednikom, oba polja Projecta su
njezina dva primjerka; 14 radnji preglednika postale su `PickLocation`/`AnswerLocation` s istim id-evima kataloga), nova
kocka `crates/qnc-settings-rows` (+ ugovor; redovi i gumbi panela postavki, prije `widgets.rs`), izgled forme
`qnc-desktop-theme::apply_settings_look` (prije kopija u `theme.rs`), Advanced radi s jednom temom. C10 `qnc-project-blocks`
1714 -> 1178, `qnc-project-session` 788 -> 568 redaka. Testovi (`qnc-project-store` 45, `qnc-location-field`,
`qnc-settings-rows`, `qnc-settings-form`), conformance, release build. Uzivo nije provjereno. Project je ponovno zamrznut.
Izmjena 2026-10-27 (korisnik: "Template nema skrol"; "Stori obrazac vertikalno, Enterom u novi red"; "polja baze pod sifrom kako bi naziv
polja mogao biti proizvoljan"; "vise pripremljenih templatea"; "owerlaj ... samostalni obrazac kojega moze kreirati administrator sustava,
ako mu ni jedan od ponudenih ne odgovara; u njemu se pise i naziv projekta, forma preuzima naziv"; "ne ogranicavati opcije, dati opciju
zakljucavanja" - "samo prekidac"): zamjenjuje sekciju Story iz prethodnog zapisa. Obrazac priloga je list polja po sifri. Nova kocka
`crates/qnc-newsroom-sheet` (+ ugovor; naziv neutralan jer `qnc-story-*` pripada obitelji aplikacije Story): pripremljeni listovi
`catalogs/story-sheets` (dnevne vijesti, javni servis, komercijalna TV, lokalna TV, agencija/online, minimalni; nazivi su sifre rjecnika
`sheet.*`), listovi ovog racunala `data/story-sheets` i izabrani `data/story-sheet.json`; overlay u sredini ekrana: izbor lista, naziv
projekta, polja jedno ispod drugog (Enter `form_next_field` ide dalje), Create/Save i Cancel; uredivac lista (naziv, prekidac Locked,
polja: naziv, vrsta tekst/izbor/trajanje/broj, izbori odvojeni zarezom, redoslijed, uklanjanje, novo polje sa sifrom iz naziva). Pripremljeni
list se nikad ne prepisuje, njegova kopija dobiva svoju sifru. Nova kocka `crates/qnc-newsroom-sheet-db` (+ ugovori modula i baze
`newsroom-sheet.database.json`; modul tablica posrednika): `project_story_sheet` (opis lista kakav je bio kad je popunjen) i
`project_story_values` (vrijednost po sifri), javni pogledi; conformance: na popisu javnih DB vlasnika. Project: panel ima template i
New project (otvara overlay, naziv se pise u njemu) te Story sheet za otvoreni projekt; `qnc-project-store` vise nema `StoryInfo` (tablica
`project_story_info` uklonjena iz ugovora), daje `project_db_binding`; `qnc-project-jobs` poslije kreiranja upisuje list u bazu novog
projekta kroz posrednika (projekt bez upisanog lista ostaje i kaze to), `LoadStory`/`SaveStory` citaju i pisu kroz kocku. Akcije kataloga
`project_story_open`, `project_story_confirm`, `project_story_cancel` (umjesto `project_story_save`, `project_new_draft`). Samostalni
program `tools/qnc-newsroom-sheet` (exe `qnc-newsroom-sheet`): administrator bira, mijenja ili pravi list racunala; uz aktivni projekt
popunjava njegov list (naziv iz projekta) i sprema ga u njegovu bazu. C10 `qnc-project-blocks` 1170 -> 1146. Testovi
(`qnc-newsroom-sheet` 4, `qnc-newsroom-sheet-db` 2, `qnc-project-jobs` 2: novi projekt cuva list u svojoj bazi, `qnc-project-store` 43),
conformance, release build (s `libav`). Uzivo nije provjereno. Project je ponovno zamrznut.
Dopuna 2026-10-28 (korisnik: "prezbijeno je.. visina svakog redka kao visina template izbornika, vece margine, prosiri obrazac";
"project name s ponudenim datumom y/m/d"; "projekti bi se trebali moci pretrazivati po novinaru, snimatelju, montazeru,
producentu i drugim"): obrazac priloga (`qnc-newsroom-sheet`) sirok 760, margine 24, redovi 32 (kao red izbora templatea),
razmak 12; polja uz oznaku visoka kao red (`qnc-settings-form` `text_box`); naslov u traci, gumbi desno dolje; novi projekt
pocinje danasnjim datumom `yyyy/mm/dd ` (`today`, crate `chrono` samo u toj kocki). Pretraga projekata: Project registry
dobio je tablicu `project_search` (ugovor `project-registry.database.json`; `qnc-project-store` `set_project_search`,
`ProjectRow.search_text`, `matches`: svaka rijec u nazivu, datumu ili vrijednostima obrasca), `qnc-project-jobs` je pise
kad god sprema obrazac (`qnc-newsroom-sheet-db` `StorySheet::search_text`: vrijednosti i nazivi izabranih izbora), polje
pretrage desno od naslova popisa projekata (`qnc-settings-rows` `title_with_search`), popis filtrira sesija
(`shown_projects`). C10 `qnc-project-blocks` 1146 -> 1142. Testovi (`qnc-project-jobs`: projekt se nalazi po obrascu),
conformance. Uzivo nije provjereno.
Popravak 2026-10-28 (korisnik: "ton se nastavlja u 2:30, a prepis tek od 2:33"; "da, otkljucaj i popravi"; otkljucan
`qnc-transcribe-jobs`): Mironik 2002: govor pocinje na 2:31,0, a red prijepisa je presjekao komad na 2:32,8, na kratkoj
stanci izmedu dvije rijeci (najtise mjesto zadnjih 5 s, a tih 5 s bio je sav govor); Parakeet za govor odrezan na kraju
komada ne napise nista (provjereno: 2:29-2:32,8 i 2:20-2:32,8 daju 0 redaka), pa su rijeci prije "poprilicno
ambiciozno" izgubljene. `qnc-transcribe-jobs` `piece_end` (koriste ga red prijepisa i program Parakeeta): rez je
najtise mjesto zadnjih 5 s samo kad je to stanka (ispod -45 dBFS RMS desetinke sekunde), inace zadnja stanka zadnjih
15 s, a tek bez nje najtise mjesto kao prije. Primjer `qnc-transcribe-jobs/examples/pieces.rs` (samo citanje) ispisuje
komade reda za WAV. Na zvuku kartice (samo citanje): komad sada pocinje na 2:30,90 i Parakeet daje "...teniski centar
osmisljen je poprilicno ambiciozno...". Prazno 0:00-0:18 i 1:26-2:33 nije greska: tamo nema govora (korisnik: live
javljanje, novinar je sutio dok je iz studija isla izjava). Test
`a_piece_whose_last_seconds_are_all_speech_ends_at_the_pause_before_them`, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-28 (korisnik: "da, dodaj zapis panica u log"; aplikacija se ugasila u 10:56:16 bez traga u
logu; uzrok je bilo azuriranje aplikacije Claude, koja je ugasila QNC pokrenut iz njezine sesije, ne pad): otkljucani
`crates/qnc-dev-diagnostics` i ulazi programa. `qnc-dev-diagnostics` `log_panics(program)`: svaki panic procesa upisuje
se u `player.log` (program, pid, nit, mjesto u kodu, poruka, backtrace) prije uobicajenog ispisa na stderr, i kad je
dijagnostika iskljucena; program pokrenut iz prozora nema stderr, pa panic prije nije ostavljao trag. Poziva se na
pocetku `main` svih sedam aplikacija (`apps/*`) i programa `qnc-ingest-worker`, `qnc-prerender`, `qnc-export`,
`qnc-player-runner`, `qnc-program-output-host`, `qnc-transcribe-parakeet`, `qnc-transcribe-whisper`,
`qnc-decoder-ffmpeg`, `qnc-format-sony`. Test `a_panic_line_names_the_program_thread_place_and_message`, conformance,
release build (player i prerender s `libav`). Gradnja whispera s Vulkanom nije uspjela (build skripta `whisper-rs-sys`),
pa uz aplikaciju ostaje njegova verzija od 26. 10. bez zapisa panica.
Izvedeno 2026-10-28 (korisnik: "u AI MA u media poolu treba dodati tab Transcript u kojemu ce biti klipovi koji su
transkriptirani... kad se zavrsi transkripcija svih automatski se otvara taj tab... briše se popis fajlova u desnom panelu";
"korisnik manualno ako zeli moze to napraviti kad god pozeli"): grupa e (AI Media Assist) ima u poolu tabove All, SubClip i
Transcript (`contracts/ui/editorial.layout.json` `tabs_left` grupe e). Transcript pokazuje uvezene klipove koji imaju
spremljeni prijepis u bazi projekta (`qnc-transcript-db` `ReadTranscribed`, cita se uz red svake sekunde;
`qnc-transcript` `TranscriptView.transcribed`, `pool_clips`). Kad red prijepisa koji je cekao ili radio vise nema nicega
sto ceka ili radi, a nesto je gotovo, aplikacija sama otvara tab Transcript (`TranscriptView.all_done`, jednom) i gotovi
klipovi izlaze iz reda (`qnc-transcript-db` `ClearDone`; neuspjeli ostaju s razlogom). Korisnik tab otvara i sam: klik
ili Alt+T (nova akcija kataloga `editorial_tab_transcript`, QNC preset). Klik na klip u tabu otvara ga na previewu i
njegov prijepis u desnom panelu; rez (dvoklik na rijec, I/O, srednja tipka) i Add SubClip / Shift+S rade kao i do sada.
Nova kocka `crates/qnc-pool-tabs` (+ ugovor `contracts/modules/pool-tabs.module.json`): model tabova poola premjesten iz
`qnc-editorial-application` (`LibraryTab`, s novim `Transcript`, `lists_shots`); prazna poruka po tabu u
`qnc-editorial-layout` (`EditorialMediaCard::empty_message`, `transcript_empty_message`). C10 spusteno: Story/MA
aplikacija 524 -> 504, ploca 100 -> 94. Testovi `qnc-pool-tabs`, `qnc-transcript-db` (popis prepisanih, brisanje gotovih),
`qnc-transcript` (tab, kraj reda), `qnc-editorial-application`, `qnc-panel-focus`, conformance. Uzivo nije provjereno.
Dopuna i popravak 2026-10-28 (korisnik: "Transkript tab treba biti prije SubClip"; "svi tabovi u kojima nema nista trebaju
biti zasivljeni i nedostupni.. to se odnosi na sve forme"; "odabrani dio teksta i klipa s ADD SubClip dodaje se u SubClip
tab"): (1) Grupa e: tabovi All, Transcript, SubClip. (2) Tab poola bez sadrzaja je siv i ne moze se izabrati (klik, tipka,
strelice pa Enter): `qnc-pool-tabs` `Contents::has` (klipovi, SubClipovi, B-roll, segmenti, prepisani klipovi);
Story/MA `EditorialView::action_enabled`, a aplikacija tab bez sadrzaja ne bira; Ingest tab All siv kad pool nema
klipova (`IngestViewModel::action_enabled`). Tabovi desnog panela (Audio AI / Video AI) nisu dirani. (3) Log: Add SubClip
nakon dvoklika na rijec nije nista spremio i nista nije rekao (u bazi aktivnog projekta nema nijednog SubClipa): zastita
"Enter je zauzet" (`sync_holds_enter`) iz vremena kad je Enter spremao SubClip odbijala je spremanje dok source timeline
drzi uzetu oznaku. Sada SubClip ceka samo Sync/B-roll koji radi ili ceka Enter (`qnc-program-segments` `subclip_waits`);
`qnc-short-save` u player log pise zasto SubClip nije spremljen i sto je spremljeno. Primjer
`qnc-virtual-shots/examples/shorts.rs` (samo citanje) ispisuje SubClipove aktivnog projekta. Testovi (`qnc-pool-tabs`,
test Virtual taba prilagoden novom pravilu), conformance. Uzivo nije provjereno.
Popravak 2026-10-28 (korisnik: "neka Video AI krene cim prijepis zavrsi"): log: zadnji prijepis gotov u 12:27:32, analiza
kadrova krenula tek u 12:34:10, jer pozadinskog radnika nitko nije probudio (analiza je krenula tek kad se radnik pokrenuo iz
drugog razloga). `qnc-transcribe-queue`: kad u redu nema vise posla, izvrsitelj pusti najam `transcribe_lease` i probudi
radnika projekta (`qnc-worker-wake`; radnik koji vec radi i ceka najam krene odmah), ishod ide u player log. Izmjereno
(Parakeet, 120 s govora Mironik 2002 iz WAV-a, samo citanje): 1 nit 0,86 jezgre, 4,2x stvarnog vremena; 2 niti (katalog
racunala) 1,9 jezgre, vrh 2,6, 8,1x; 4 niti 3,6 jezgre, 8,6x. Conformance. Uzivo nije provjereno.
Dopuna 2026-10-28 (korisnik: "kada se kreira SubClip, njegov play krece od IN do OUT i tu staje, korisnik ga moze editirati
ili novim Space nastaviti play; trebamo akciju koja ce ga vratiti na pocetak SubClipa"): nova akcija kataloga `go_to_mark_in`
(Alt+I, QNC preset, opsezi `source_timeline`, `storyboard`, `ingest`; grupa pomoci "Reprodukcija i playhead"): playhead na IN
prikazanog klipa (korisnikov IN, bez njega pocetak klipa), pa Space ponovno pusta SubClip. Uzeta oznaka ostaje uzeta.
`qnc-source-panel` (`go_to_mark_in`, `in_frame`, test), `qnc-panel-focus` (s bilo kojeg panela: pogled izvora, tipkovnica na
source timeline). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-28 (korisnik: "gdje pise ono je a, jer covjek kaze aono"): u zoomu Mironik 3452 rijec "ono" stajala je na
2418, gdje glas "a" pocinje (izmjereno ffmpegom s kartice: tisina 2395-2416, glas od 2418), a "A" je ostalo u tisini na 2413.
Motor je "A ono" cuo kao jedno, pa su dvije rijeci imale jedan pocetak glasa i poravnanje ga je dalo kasnijoj. `qnc-word-align`:
rijec ne smije poceti u tisini bez ikakvog glasa do sljedece rijeci; tada uzima pocetak glasa te rijeci, a sljedeca pocinje gdje
ju je motor stavio, iza nje. Test na stvarnim vrhovima te kartice (prozor zooma IN 2427): A 2418, ono 2421, sto 2431, nas 2444.
Granica izmedu "a" i "ono" je vrijeme motora (2421), jer u zvuku nema stanke. Testovi, conformance. Uzivo nije provjereno.
Ispravak isti dan (korisnik: "ne, a je jednako glasno kao i ono"): prvi val od 2418 je "a", drugi od 2431 "ono"; motor je sve
rijeci dao 3-10 slika ranije. Pravilo iznad (sljedeca rijec ostaje gdje ju je motor stavio) dalo je "ono" na 2421, usred "a".
Sada `qnc-word-align` `assign` rijeci ne dopusta da ostane na mjestu motora koje je tisina (`keepable`): takva rijec uzima
pocetak glasa, a ostale se slazu redom. Mironik 3452 (pravi vrhovi s kartice): A 2418, ono 2431, sto 2444, nas 2449, sve 2462,
zanima 2469; test Mironik 2680 i dalje prolazi. Testovi, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-10-28 (korisnik: "sada bi se posvetio MA Audio: pisanje i snimanje offova i tekstualno slaganje
price"; v5 i v4 to nemaju, u katalogu su samo nazivi `record_toggle`/`save_off`; plan u tri koraka: scenarij, snimanje
OFF-a, slaganje price u Story; "da, otkljucaj Project i kreni s korakom 1"; "za pisanje offova na dnu mali editor u
kojem korisnik pise i ispravlja, dugmetom Dodaj off tekst ide u tablicu iznad"; "taj editor moze biti TextSourceEditor"):
korak 1. Project: postavka `audio.reading_words_per_second` (seed svih sistemskih predlozaka 2,5; `qnc-project-store` je
dopunjuje starijem projektu pri otvaranju, test; Advanced "Audio editing" izbor 2/2,5/3/3,5; rjecnik;
`qnc-work-settings` `reading_words_per_second` strogo). Project je ponovno zamrznut. Nove kocke (s ugovorima modula):
`qnc-script-db` (modul tablica posrednika, `script_blocks` / `public_script_blocks`, ugovor baze `script.database.json`:
blokovi redom citanja, OFF tekst ili izjava s klipom, IN/OUT, timecodeovima, duljinom i rijecima; dodaj iza izabranog,
gore/dolje, ukloni, prazan OFF i izjava bez raspona se odbijaju; conformance: na popisu javnih DB vlasnika),
`qnc-script` (stanje scenarija: citanje i pisanje izvan niti sucelja, ponovno citanje svake 2 s; duljina OFF-a iz rijeci i
brzine citanja projekta, izjave iz klipa; ukupno prema zadanom trajanju iz obrasca priloga, prvo polje vrste duration),
`qnc-text-source-editor` (TextSourceEditor: mali editor teksta s retkom informacija i gumbima pozivatelja; ne drzi tekst),
`qnc-script-panel` (panel Scenarij: duljine OFF/SOT/ukupno i cilj (crveno kad je preko), blokovi s duljinom, Open, gore,
dolje, X; izjave iz AI Media Assista s "+"; na dnu TextSourceEditor: Add OFF stavlja tekst iza izabranog bloka, klik na OFF
vraca njegov tekst u editor, Replace ga sprema natrag, Clear prazni; Add SOT (IN-OUT) uzima IN/OUT prikazanog klipa s
rijecima izgovorenim tamo; Copy daje scenarij kao tekst). Grupa g (Media Assist Audio) ima desni panel `script_panel`
(`contracts/ui/editorial.layout.json`; conformance: g mora imati script_panel). `qnc-right-panel` ga crta, `qnc-panel-focus`
drzi scenarij i salje mu radnje (`script_*`, katalog bez tipki), `qnc-transcript` `TranscriptView::said`. Tipke kataloga
ne rade dok se pise u editoru (polje teksta drzi tipkovnicu, postojece pravilo). Testovi `qnc-script-db` (3), `qnc-script`
(2), `qnc-project-store`, `qnc-panel-focus`, conformance, release build. Uzivo nije provjereno. Nije izvedeno: korak 2
(snimanje OFF-a, mapa `products/voice`), korak 3 (slaganje price u Story iz scenarija).
Otkljucano i izvedeno 2026-10-28 (korisnik: "da, otkljucaj Project i kreni s korakom 2"; u scenariju je napisao da je
prvo najava koju cita spiker, pa prvi off; odabrao dijelove "Najava i odjava" i preslusavanje kroz Broadcast Player;
"ton koji snimamo neka u QNC dobije timecode i moguce video title teksta; treba omoguciti i ucitavanje tona snimljenog na
drugi nacin (kamera na terenu), kasnije preko mreze"; "koristi kocke"): korak 2, prvi dio. Scenarij: vrste blokova
Najava (`intro`, spiker prije priloga) i Odjava (`outro`, spiker poslije), ne snimaju se i ne broje u trajanje priloga
(zasebno "Anchor"); editor ima gumbe Lead-in i Tag; najava bez izabranog bloka ide na pocetak. `qnc-script-db`: vrste i
polja snimke (`voice_uri`, `voice_seconds`, `voice_tc`), operacija `SetVoice` (samo OFF), `Place::First`; tablica prvog
dana (bez tih stupaca) prepisuje se u novu pri prvom upisu, blokovi ostaju, a do tada se cita bez snimki (test). Project:
mapa proizvoda `products/voice` (popis mapa proizvoda je jedan korijen `products` i nazivi polja, iste vrijednosti kao
prije; C13 Projecta 6 -> 0, baseline spusten); `qnc-work-settings` `ProductLocations.voice` (neobavezno, projekt bez nje
dobiva kontroliranu gresku samo za snimanje) i `voice_location` (mapa i URI pod mapom projekta kao izvorom medija).
Project je ponovno zamrznut. Nova kocka `qnc-voice-record` (+ ugovor): mikrofon racunala (zadani ulaz ili naziv iz
postavke racunala `voice-input.json` uz ostale postavke) snima prvi kanal na frekvenciji projekta u mono 24-bit WAV u mapi
projekta (provjera da je put u projektu; datoteka odmah pod konacnim imenom), mikrofon koji ne radi na toj frekvenciji je
jasna greska; timecode je doba dana pocetka snimke u stopi projekta. `qnc-script`: radnja `record_toggle` na OFF bloku
(Rec/Stop, otvaranje mikrofona izvan niti sucelja), snimka se upisuje uz blok (`off_<id>_<vrijeme>.wav`), novi take
zamjenjuje zapis; duljina snimljenog OFF-a je duljina snimke. Panel: Rec/Stop na OFF bloku, vrijeme i razina mikrofona dok
snima, "recorded m:ss TC ..." uz snimljeni OFF. Proba kocke na scratchpadu: 2,0 s, 48 kHz, 24-bit mono. Testovi
`qnc-script-db` (5), `qnc-voice-record` (3), `qnc-script` (2), `qnc-project-store` (43), zlatna tablica tipki,
conformance, release build. Uzivo kroz sucelje nije provjereno. Nije izvedeno: preslusavanje kroz Broadcast Player (snimka
kao klip projekta sa slikom naslova teksta i timecodeom), ucitavanje tona snimljenog drugdje, slaganje price u Story.
Izvedeno 2026-10-28 (korisnik: "da, kreni s klipom snimke za Broadcast Player"): korak 2, drugi dio. Nove kocke (s
ugovorima modula): `qnc-title-card` (naslovna slika: bijeli tekst na crnom u velicini slike projekta, font sucelja QNC-a
Ubuntu-Light iz `epaint_default_fonts` preko `ab_glyph`, rijeci prelomljene i smanjene dok ne stanu, mala linija iznad;
ista na svakom racunalu) i `qnc-voice-clip` (snimka kao klip: slika je naslovna slika teksta, zvuk snimka kakva jest
(24-bit PCM, njezina frekvencija), timecode doba dana snimanja; MXF OP1a s XDCAM HD422 slikom (MPEG-2 4:2:2, 50 Mbit CBR,
GOP 12) u stopi i velicini projekta, ffmpegom kataloga dekodera, odmah pod konacnim imenom u mapi projekta; slika ide
kroz privremenu mapu sustava). `qnc-script`: Stop snimanja odmah pravi klip `products/voice/off_<id>_<vrijeme>.mxf` uz WAV
i upisuje njegov URI uz OFF blok (naslov "OFF n"). Proba na scratchpadu (samo citanje izvora): klip 1920x1080p50
mpeg2video 4:2:2, pcm_s24le 48 kHz mono, 2,0 s, timecode 19:14:48:00; naslovna slika provjerena (i dijakritici).
Testovi `qnc-title-card` (2), `qnc-voice-clip` (2), `qnc-script`, conformance, release build. Uzivo kroz sucelje nije
provjereno. Nije izvedeno: klip jos nije u katalogu projekta (player pusta samo klipove iz kataloga; ulazak ceka odluku
korisnika jer Ingest Select i probe smije pokretati samo Ingest), ucitavanje tona snimljenog drugdje.
Izvedeno 2026-10-28 (korisnik: "aplikacija je zamisljena za opcu namjenu, taj dio treba biti programibilan, ne fiksiran";
pregled: iNEWS nema propisan obrazac, prilog je story form (polja po redakciji) + tijelo + numerirani production cueovi i
`*CG` upute, razmjena MOS protokolom `roStorySend`; plan u 5 tocaka: profil, vrste blokova iz profila, polja uz vrstu,
izvoz kroz predloske, uredivanje profila; "da, kreni s tockom 1"): tocka 1. Nova kocka `crates/qnc-script-profile`
(+ ugovor `contracts/modules/script-profile.module.json`): profil scenarija je podatak, ne kod. Profil ima vrste blokova
(sifra koju baza cuva, naziv, kratka oznaka, uloga spoken/anchor/clip/note, trajanje story/anchor/none, snima li se,
incue/outcue, oznaka numeriranog cuea s `{n}` kao iNEWS production cue, polja uz vrstu kao u obrascu priloga) i izbore
polja. Pripremljeni profili u `catalogs/script-profiles` (nazivi su sifre rjecnika): `general_tv` (najava, OFF, SOT s
potpisom ime/funkcija, stand-up, prirodni ton, odjava), `inews_mos` (On Camera, Take VO n, Take SOT n, `*CG` s predloskom i
retcima, TAG), `radio`, `agency_online`; profili racunala u `data/script-profiles`, izbor u `data/script-profile.json`;
pripremljeni se nikad ne prepisuje (kopija dobiva svoju sifru), profil koji se ne moze koristiti (krive sifre, sifra
dvaput, snima se a nije spoken, note s trajanjem, polje s nepostojecim izborima) izostavlja se uz razlog. Sifre `off`,
`sot`, `intro`, `outro` postojecih blokova su vrste profila koji ih imaju. Testovi `qnc-script-profile` (3). Conformance
NE prolazi dok tocka 2 ne spoji profil u scenarij: C5 "javna kocka bez korisnika" (`qnc-script-profile` jos nitko ne koristi).
Nije izvedeno (tocke 2-5): baza i panel na vrstama iz profila, polja uz vrstu, izvoz kroz predloske, uredivanje profila.
Izvedeno 2026-10-28 (korisnik: "da, kreni s tockom 2"): baza i panel scenarija rade s vrstama iz profila, a projekt pamti
profil s kojim je napisan. `qnc-script-db`: vrsta bloka je sifra profila (`Block.kind: String`, tablica vise ne nabraja
vrste; stare tablice s `kind IN (...)` prepisuju se uz zadrzane blokove i snimke); nova tablica `script_profile` (jedan red:
sifra i cijeli profil kakav je bio kad je izabran) i pogled `public_script_profile`; operacije `ReadProfile`,
`SetProfile` (odbija profil bez vrste koju blokovi imaju); upis bloka vrste koje profil nema, prazan tekst tekstualne
vrste, klip blok bez raspona i snimka vrste koja se ne snima se odbijaju; bez profila se ne pise. `qnc-script`: projekt bez
profila pri prvom citanju dobiva profil izabran na racunalu (`qnc-script-profile` `DEFAULT_PROFILE` = general_tv kad nista
nije izabrano), oznaka bloka je numerirani cue vrste (`OFF 2`, `Take SOT 1`) ili kratka oznaka, trajanje po ulozi (klip
svoj raspon, snimka svoje, note nista, tekst po rijecima), zbroj priloga i spikera po `counts`, Rec samo na vrsti koja se
snima, naslov kartice snimke je oznaka bloka; akcije `script_add_text` i `script_add_clip` (target: sifra vrste),
`script_replace_text` (umjesto `script_add_off`/`_intro`/`_outro`, `script_add_sot`, `script_replace_off`).
`qnc-script-panel`: gumb `+ <oznaka> (IN-OUT)` za svaku klip vrstu, gumb `+ <oznaka>` u editoru za svaku tekstualnu
vrstu, naziv profila uz naslov. `qnc-text-source-editor` `EditorButton.target`. Ugovori `script.database.json`,
`script-db.module.json`, rjecnik, katalog akcija. Testovi `qnc-script-db` (7), `qnc-script` (3), `qnc-script-profile` (3),
conformance prolazi (C5 rijesen), release build. Uzivo nije provjereno. Nije izvedeno (tocke 3-5): polja uz vrstu, izvoz
kroz predloske, uredivanje profila (i promjena profila projekta).
Otkljucano i izvedeno 2026-10-28 (korisnik: "pokvario si transcribe"; nalaz u logu: prijepis radi, ali je nakon Uvezi
novog projekta citanje trajalo 157 s i motor 156 s za 254 s zvuka (ranije 44 s i 33 s), jer su valovi i filmstrip svih
klipova istodobno citali karticu D: i trosili procesor; "da, otkljucaj i popravi, obrisi debug cache"): `tools/qnc-ingest-worker`
(`run_artifacts`, `transcription_running`): dok je najam reda prijepisa (`qnc-transcribe-queue::LEASE`) ziv, valovi i
filmstrip rade kao dok player svira (samo klip otvoren u previewu, najvise jedan radnik); ostali cekaju dok red ne zavrsi.
Najam se cita najvise dvaput u sekundi. `qnc-timeline-artifacts` nije trebalo mijenjati. Debug cache obrisan
(`cargo clean --profile dev`, 36,3 GiB) uz dozvolu korisnika. Conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-28 (korisnik: "nastavi s tockom 3 i pokreni za test"): tocka 3, polja uz vrstu bloka. `qnc-script-db`:
blok cuva vrijednosti polja svoje vrste (`Block.fields`, stupac `fields_json`, starija tablica se prepisuje uz zadrzane
blokove i snimke), operacija `SetFields` (polje koje vrsta nema i izbor izvan izbora profila se odbijaju, prazna vrijednost
je nista; test). `qnc-script-profile` `cue_label_with`: oznaka cuea moze imenovati polje (`{code}` postaje naziv izbora ili
vrijednost; `inews_mos` CG je `*CG {cg_template}`). `qnc-script`: akcije `script_field_text` (nacrt dok se tipka),
`script_field_commit` (nacrti bloka u bazu kad se polje napusti ili Enter), `script_field_set` (izbor odmah); oznaka bloka
uzima polja, Copy daje polja ispod teksta (`Naziv polja: vrijednost`). `qnc-script-panel`: ispod teksta bloka redak po
polju (tekst ili izbor); blok se bira klikom na oznaku ili tekst (okvir bloka vise ne uzima klikove gumba i polja).
Katalog akcija (bez tipki), rjecnik, ugovor `script.database.json` (`fields`). Testovi `qnc-script-db` (8), `qnc-script`
(4), `qnc-script-profile` (3), conformance, release build. Uzivo nije provjereno. Nije izvedeno (tocke 4-5): izvoz kroz
predloske (MOS, dva stupca, tekst), uredivanje profila i promjena profila projekta.
Izvedeno 2026-10-28 (korisnik: tocka 4 prosirena prijedlozima iz Gemini razgovora, in-cue/out-cue i pravila zapisa;
"zavrsi prije planirane radnje, a na kraju mozemo testirati nove mogucnosti"): izvoz scenarija. Nova javna kocka
`crates/qnc-script-export` (+ ugovor `contracts/modules/script-export.module.json`; cista, nista ne cita ni pise): cisti tekst
(za e-mail ili poruku), dvostupacni A/V scenarij kao HTML (slika i upute lijevo, ton i tekst desno, trajanje uz rub; ispis na
A4) i MOS 2.8 `roStorySend` (izgovoreni tekst kao `<p>`, oznake, upute, potpisi i cue kao `<pi>`, obrazac priloga i trajanja
kao `mosExternalMetadata`); upute rezije, oznake i grafika VELIKIM slovima, izgovoreni tekst kako je napisan. `qnc-script`:
in-cue (prve 3 rijeci) i out-cue (zadnje 4) priloga iz prvog i zadnjeg bloka koji ulazi u trajanje priloga, i svakog bloka
cija vrsta ima `cues` (SOT); akcija `script_export` pise `<projekt>_script.txt`, `.html` i `.mos.xml` u mapu exporta projekta
(izvan niti sucelja), `script_open_export` otvara A/V scenarij u pregledniku datoteka. Panel: gumb Export, In-cue/Out-cue
priloga ispod trajanja, IN/OUT ispod SOT-a, link "Exported: ..." na izvezenu datoteku. Premjesteno (ne kopirano):
mapa exporta `qnc-work-settings` `WorkSettings::export_folder` i ime datoteke `export_file_name` (iz `tools/qnc-export`, koji
ih sada koristi); `qnc-newsroom-sheet-db` `StorySheet::shown` (obrazac kako se cita: naziv polja, izbor po nazivu, trajanje
m:ss). Katalog akcija (bez tipki), rjecnik. Testovi `qnc-script-export` (3), `qnc-script` (5), conformance. Uzivo nije
provjereno. Nije izvedeno: status priloga i povijest verzija, provjera po formatu (VTR/VO/SOT), tocka 5.
Izvedeno 2026-10-28 (tocka 5 plana profila scenarija; korisnik: "zavrsi prije planirane radnje"): uredivanje profila scenarija.
Nova kocka `crates/qnc-script-profile-editor` (+ ugovor `contracts/modules/script-profile-editor.module.json`): izbor profila
racunala, promjena naziva, prekidac Locked, vrste blokova (naziv, oznaka, uloga, u koju duljinu se broji, snima li se (samo
spoken), in/out cue, oznaka cuea, polja uz vrstu), dodavanje, uklanjanje i redoslijed vrsta; Save cuva kroz
`qnc-script-profile` (pripremljeni profil kopija pod svojom sifrom), "Use for project" daje profil aktivnom projektu.
Uredivac polja premjesten iz uredivaca obrasca u `qnc-newsroom-sheet::fields_editor` (koriste ga obrazac i profil).
Samostalni program `tools/qnc-newsroom-sheet` ima tabove Story sheet | Script profile; uz aktivni projekt pokazuje njegov
profil i upisuje izabrani kroz `qnc-script-db::set_profile` (vlasnik tablice). Rjecnik `sp.*`, `sheet.tab_*`. Testovi
`qnc-script-profile-editor` (1), `qnc-newsroom-sheet`, `qnc-script`, `qnc-script-db`, conformance, release build. Uzivo
nije provjereno.
Otkljucano i izvedeno 2026-10-28 (dubinski audit kocki scenarija; korisnik: "provjeri je li sve u kockama"; "da, otkljucaj i
kreni s 1-4"): (1) Desni panel: transkript, Video AI i scenarij drzao je `qnc-panel-focus` u svakoj formi, pa su Story i MA
forme bez tih panela citale bazu za njih i pri prvom citanju upisivale profil scenarija u projekt. Nova kocka
`crates/qnc-side-pieces` (+ ugovor): pravi samo dijelove panela koje raspored forme ima (`right_panel` i
`right_panel_tabs`; scenarij donosi i transkript zbog izjava), njihove pollove, radnje i prikaze kao dijeljene snimke
(`Arc`, obnovljene samo kad se poll promijenio); konstante vrsta panela su njezine (`qnc-right-panel` ih ponovno izvozi).
`qnc-panel-focus` drzi samo `side` umjesto tri dijela (`FocusLayout.right_panels`, `qnc-editorial-layout::focus_names`
daje i vrste desnih panela; zlatna tablica tipki nepromijenjena). (2) Opis polja i sifre izasli su iz UI kocki u novu
podatkovnu kocku `crates/qnc-field-spec` (+ ugovor; `FieldSpec`, `FieldKind`, `OptionSets`, `valid_code`, `code_of`,
`new_field_in`); `qnc-settings-form` i `qnc-newsroom-sheet` ih ponovno izvoze, a `qnc-script-db` i `qnc-script-profile`
vise ne ovise o eframe/egui. (3) `qnc-script` (899 redaka u jednoj datoteci) podijeljen: snimanje (mikrofon, take, klip
snimke) je nova kocka `crates/qnc-voice-take`, pisanje izvoza u mapu exporta nova kocka `crates/qnc-script-files` (obje s
ugovorima), prikaz u `view.rs`, citanje u `read.rs`, testovi u `tests.rs`. (4) Postavke se citaju strogo:
`qnc-work-settings` `audio_sample_rate`, `video_picture` (test); take bez njih je kontrolirana greska, a projekt bez
brzine citanja kaze razlog u panelu umjesto tihog 0:00. Testovi dotaknutih kocki, conformance, release build. Uzivo nije
provjereno. Ostaje iz audita (5-8): MOS oznake su u kodu, kopije provjere mape projekta i mjeraca razine, polja bloka
mimo `qnc-settings-form`, provjera raspona klip bloka prema katalogu, program obrasca pise u bazu na niti sucelja.
Izvedeno 2026-10-28 (korisnik: usporedba s obrascem `broadcast_obrazac_tv_prilog.pdf`, "da, slozi profil i doradi izvoz";
zatim "ti profili trebaju biti json ne kodirani dio aplikacije kako bi se mogao prilagodavati"): novi pripremljeni profil
`catalogs/script-profiles/tv_package.json` (TV prilog po obrascu: studio s kamerom i grafikom, VO s B-rollom i prirodnim tonom,
SOT s potpisom, stand-up s kadrom i lokacijom, NAT, odjava novinara s B-rollom i potpisima, odjava voditelja). Sve rijeci
izvoza dolaze iz JSON-a profila, ne iz koda: vrsta bloka ima `speaker` (tko se cuje, s `{polje}`; prazno kad polje koje
imenuje nema vrijednost) i `picture` (redak slike: `{mark}`, `{clip}`, `{start}`, `{end}` i polja; bez njega samo oznaka),
profil ima `labels` (stupci VIDEO/AUDIO/DUR, IN-CUE, OUT-CUE, TRT, ANCHOR, redak snimke `{tc}`, redak cueova `{in}`/`{out}`,
smjer prijelaza na prilog nakon teksta voditelja; profil bez `labels` dobiva zadane engleske rijeci, bez smjera prijelaza).
`qnc-script-profile` (`KindSpec.speaker`, `picture`, `Labels`, `speaker_with`, `picture_with`), `qnc-script-profile-editor`
(retci "tko se cuje" i "redak slike"), `qnc-script-export` (bez ijedne ugradene rijeci; tekuce vrijeme priloga od prvog bloka u
trajanju, smjer prijelaza na prvom bloku priloga nakon voditelja, govornik iznad teksta, polja velikim imenom i vrijednoscu
kako je napisana), `qnc-script` `export_story`. Svi pripremljeni profili dobili su `picture` iste kao dosadasnji izlaz
(`{mark} (ON CAM)`, `{mark} (B-ROLL)`, `{mark}: {clip} {start} - {end}`). Labels se zasad mijenjaju samo u JSON-u (urednik
profila ih ne prikazuje). Testovi `qnc-script-export` (5, novi: svaka rijec iz profila), `qnc-script-profile`, `qnc-script`,
`qnc-script-profile-editor`, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-29 (korisnik: "dodaj labels u urednik profila i ispravi radio profil"): urednik profila scenarija
(`qnc-script-profile-editor`) ima okvir "Words of the written-out script" s retkom za svaku rijec izvoza (`labels`: stupci,
in/out-cue, TRT, vrijeme voditelja, redak snimke `{tc}`, redak cueova `{in}`/`{out}`, prijelaz na prilog; prazno = nema
retka), zakljucan kao i ostatak profila. Radio profil vise nema TV rijeci: stupci CUE / TEXT, PRESENTER, `REC TC {tc}`,
`[PLAY PACKAGE]`, najava i odjava `{mark} (STUDIO)` s govornikom PRESENTER, tekst novinara samo oznaka s govornikom
REPORTER. Rjecnik `sp.labels`, `sp.l.*`, `sp.sp.presenter`, `sp.sp.reporter`. Testovi `qnc-script-profile-editor`,
`qnc-script-profile`, `qnc-script-export`, `qnc-script`, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-29 (korisnik: "slaganje source-segmenata se radi u obrascu za pisanje i snimanje priloga"; "dodamo novo
dugme SubSegment umjesto Talking head u MA audio; to dugme bi dodalo transkriptirani tekst vezan za dodani SubSegment; ako
ima vise dijelova tekst bi se slagao jedan ispod drugoga"): u MA Audio (grupa g) dugme SubSegment zamjenjuje Talking Head
(`contracts/ui/editorial.layout.json`, akcija kataloga `script_subsegment` bez tipke, omogucena kad je klip izabran).
Pritisak uzima IN/OUT prikazanog klipa s rijecima prijepisa izgovorenim tamo: kad je u scenariju izabran blok izjave
(klip vrsta profila), postaje njegov sljedeci dio; inace nastaje novi blok prve klip vrste profila, izabran, pa sljedeci
pritisak dodaje u njega. `qnc-script-db`: `Piece` i `Block.pieces` (stupac `pieces_json`, starija tablica se prepisuje uz
zadrzane blokove), operacija `AddPiece` (blok jednog raspona zadrzava taj raspon kao prvi dio; dio bez raspona i dio
tekstualnog bloka se odbijaju), `Block::of_pieces`/`from_pieces`: klip, IN i pocetni TC prvog dijela, zavrsni TC
zadnjeg, duljina zbroj, tekst rijeci dijelova jedan ispod drugoga; ugovor `script.database.json` (`pieces`).
`qnc-script` (`SUBSEGMENT`, `marked_piece` zajednicki s "+ SOT (IN-OUT)"), `qnc-script-panel` (blok od vise dijelova:
broj dijelova i ukupno trajanje, ispod svaki dio s klipom, TC-ovima i svojim tekstom). Nije izvedeno: slaganje price u
Storyju iz bloka od vise dijelova (Ton segment), brisanje i redoslijed pojedinog dijela, izvoz scenarija po dijelovima
(izvoz pise blok kao jedan raspon s tekstom svih dijelova). Testovi `qnc-script-db` (novi), `qnc-script`,
`qnc-script-panel`, `qnc-editorial-application`, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-29 (korisnik: "u SOT dodati dugme koje bi otvaralo video u source-SubSegment timeline... bazira se na
kodu program-video-timelinea; vise dijelova istog klipa u jednu izjavu, koja bi se u Storyju ponasala kao jedan
virtualni klip"; "ton se mora dobro sloziti: dodati prazne dijelove ako treba razmaknuti tonove; krosfade ili rez,
testirat cemo; slazem se novi tab za njih"): korak 1-3 plana. Blok od vise dijelova ima dugme "Open SubSegment"
(`script_open`): SubSegment se otvara na mjestu source timelinea (`qnc-editorial-blocks` jedan redak, isti broj redaka),
a isti Broadcast Player pusta njegove dijelove kao program (player nije mijenjan). `qnc-program-input` `pieces.rs`
(`load_pieces`, `pieces_loader`, `PieceInput`): dijelovi jedan iza drugoga na V1 i A1 kao Ton segmenti, razmak je Off s
iskljucenim A1 (crno i tisina), A1 svakog klipa na kanalu i razini zadrzanoj na source timelineu (`qnc-clip-sound`,
bez zapisa kanal 1), stopa je stopa prvog dijela (2:1 kao u prici). `qnc-program-playlist` `a1_joins.rs` (`join_a1`,
`A1Join`): na spoju bez razmaka crossfade `audio.crossfade_frames` projekta (zvuk prethodnog dijela pada iza svog OUT-a,
sljedeci raste prije svog IN-a, koliko izvor ima slika), uz razmak dio prije njega pada, a dio poslije raste preko pola
tih slika; rez ne dira zvuk. `qnc-script-db`: `Piece.gap_before` (slike razmaka prije dijela) i `Piece.cut` (rez umjesto
crossfadea, zadano crossfade), operacija `SetJoin` (prvi dio je nema); ugovor `script.database.json`. `qnc-script`
(`subsegment_view`, `piece_inputs`, `SubsegmentView`; radnje `subsegment_cue`, `subsegment_join_gap`,
`subsegment_join_cut`, `subsegment_close`, katalog bez tipki), `qnc-side-pieces` predaje dijelove previewu,
`qnc-source-preview` (`show_pieces`, `shown_pieces`: isti dijelovi se samo cueaju, promijenjeni razmak ili spoj otvara
program ponovno na istom mjestu; normalizacija price ne vrijedi za SubSegment; story/Wrap otvara pricu umjesto njega).
Nova javna kocka `crates/qnc-subsegment-panel` (+ ugovor): dijelovi na jednom `qnc-timeline` s crtom na svakom spoju i
playheadom playera, klik je playhead, ispod za svaki spoj razmak (-/+ jedna slika) i Cut | Crossfade, Close vraca source
timeline; klik na klip u poolu ga takoder vraca. Nije izvedeno (sljedeci korak): val i filmstrip dijelova na tom
timelineu, pomak IN/OUT pojedinog dijela, SubSegment kao virtualni klip u bazi (nova klasa u `virtual_shots`) i novi tab
u poolu, Ton segment u Storyju iz SubSegmenta. Testovi `qnc-program-playlist` (2 nova: crossfade i razmak, rez ne dira
zvuk), `qnc-script-db` (novi: razmak i spoj), `qnc-program-input`, `qnc-script`, `qnc-side-pieces`,
`qnc-source-preview`, conformance. Uzivo nije provjereno.
Dopuna 2026-10-29 (korisnik: "nastavi s valom na SubSegment timeline; on treba sve opcije koje imaju pokrivalice, da
se moze editirati na isti nacin"): prvi dio. Timeline SubSegmenta ima val A1 svojih dijelova (`qnc-program-waveform`
`ProgramWaves` kroz citac artefakata previewa, `SourcePreview::artifact_reader`; val kanala 1 kako je snimljen, ne
kanala zadrzanog za A1). Rub dijela se uredjuje kao rub slota pokrivalice: Ctrl+I / Ctrl+O uzimaju IN ili OUT dijela pod
playheadom, desni klik na spoj uzima IN dijela iza njega (`subsegment_take_edge`), strelice i kotacic ga pomicu za sliku,
bijeli trim playhead pokazuje rub, svaki pomak odmah svira nacrt na rubu (player otvara SubSegment s nacrtom), Enter
zapisuje (`qnc-script-db` `SetPiece`: isti klip, barem jedna slika, razmak i spoj ostaju; timecodeovi i duljina
pomaknuti za isto), Esc odustaje. `qnc-script` (`trim_key`, `drafted`, `trimmed_piece`, `SubsegmentView.a1`/`trim`),
`qnc-subsegment-panel`. Nije izvedeno: zoom reza (slike prije/poslije, fini val, rijeci) kao kod pokrivalica, filmstrip
dijelova, kanal i razina A1 po dijelu, povlacenje ruba misem, premjestanje i brisanje dijela. Testovi `qnc-script-db`
(novi), `qnc-script`, conformance. Uzivo nije provjereno.
Dopuna 2026-10-29 (korisnik: "da, nastavi sa zoomom reza"): dok je rub dijela SubSegmenta uzet, u sredini ekrana je
isti zoom reza kao kod pokrivalica (`qnc-trim-overlay`, slike, fini val i rijeci puni `qnc-trim-stills`, ne kopija):
za IN zadnja slika prije reza (prethodni dio, a iza razmaka ono sto je prije IN-a) i prva slika dijela (oznacena), za
OUT zadnja slika dijela (oznacena) i prva sljedeceg dijela (iza zadnjeg dijela ili prije razmaka ono sto je iza OUT-a);
ispod A1 sekundu sa svake strane reza na osi SubSegmenta s rijecima. Kotacic nad zoomom pomice rub; dvoklik na rijec tu
ne radi rez. `qnc-script` (`subsegment_zoom`, `SubsegmentView.zoom`, `trim_stills`; slike prate nacrt), crta ga
`qnc-subsegment-panel`. Test `the_zoom_of_a_subsegment_edge_shows_the_pictures_either_side_of_its_cut`, conformance.
Uzivo nije provjereno.
Popravak i dopuna 2026-10-29 (korisnik: "nastavi s kanalom i razinom A1 po dijelu"; "bag: OUT prvog subclipa ne
dozvoljava editirati; provjeri mogu li se svi IN i OUT editirati kod svih subklipova desnim misem"): (1) desni klik na
timelineu SubSegmenta uzimao je samo IN dijela iza spoja (crta je bila samo na spojevima), pa OUT dijelova i IN prvog
dijela misem nisu bili dostupni. Sada je crta na svakom IN-u i OUT-u svakog dijela, a desni klik uzima najblizi rub po
kliknutoj slici; na spoju bez razmaka slika prije spoja uzima OUT, slika na ili iza spoja IN (`qnc-subsegment-panel`
`nearest_edge`, test `a_right_click_takes_every_in_and_out_of_every_part`). (2) Svaki dio ima svoj A1: kanal kamere,
razinu (1 dB, -40..+12) i ukljuceno/iskljuceno, u retku dijela ispod timelinea (broj kanala iz spremljenog vala klipa).
Bez izbora kanala vrijedi kanal zadrzan za klip na source timelineu. `qnc-script-db` `Piece.a1_channel`, `a1_level_db`,
`a1_off` (zapis kroz `SetPiece`), `qnc-program-input` `PieceInput.a1`, `qnc-script` (`subsegment_part_channel`,
`subsegment_part_level`, `subsegment_part_off`; val dijela prati kanal i razinu), katalog akcija bez tipki. Svaka promjena
se odmah upise i SubSegment ponovno svira. Nije izvedeno: Ctrl+1 i tipke traka (kao kod pokrivalica) za zvuk dijela,
filmstrip dijelova, povlacenje ruba misem. Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-29 (korisnik: "nastavi s tipkama za zvuk dijela kao kod pokrivalica"; "u zoom dvije slike lijevo i desno
od crvenog playheada koje predstavljaju OUT prethodnog klipa i IN narednog; klik na slicicu mijenja objekt edita, lijevo
OUT, desno IN narednog klipa"): (1) A1 dijela tipkama kao traka pokrivalice (isto preslikavanje `qnc_timeline::LaneKey`):
Ctrl+1 uzima A1 dijela pod playheadom (naziv dijela u retku oznacen kao uzeta traka), gore/dolje kanal kamere, lijevo
iskljuci, desno ukljuci, Ctrl+strelice i Ctrl+kotacic razina za 1 dB, svaki korak odmah svira nacrt, Enter zapisuje
(`SetPiece`), Esc odustaje; uzimanje ruba (Ctrl+I/O, desni klik) pusta uzeti zvuk i obrnuto. `qnc-script` (`sound_key`,
`PartSound`, `SubsegmentView.sound`), `qnc-subsegment-panel`. (2) Zoom reza SubSegmenta: lijevo uvijek zadnja slika
prethodnog dijela (i iza razmaka), desno prva slika sljedeceg; klik na lijevu sliku uzima OUT prethodnog dijela, na
desnu IN sljedeceg (`subsegment_take_edge`); pomaknuti rub koji se napusti zapisuje se prije, a isti dio zadrzava svoj
nacrt. `qnc-program-view` `EditStill.click` (radnja kataloga i cilj; `None` drugdje, zoom pokrivalica isti), crta i
klika `qnc-trim-overlay`. Test zooma prosiren (klikovi, OUT prethodnog dijela iza razmaka, prvi IN i zadnji OUT bez
klika), conformance, `cargo check --workspace`. Uzivo nije provjereno.
Dopuna 2026-10-29 (korisnik: "nastavi s povlacenjem ruba misem"): na SubSegment timelineu uzeti rub dijela (desni klik,
Ctrl+I/O, klik na sliku zooma) vuce se lijevom tipkom misa, po pravilu misa samo uzeto: rub prati pokazivac kao nacrt
(dio zadrzava barem jednu sliku i ostaje unutar svog klipa, player pokazuje sliku na rubu kao uz strelice), pustanje
tipke ga sprema kao Enter. `qnc-subsegment-panel` (uzeti rub je `taken` marker, `drag_target`, test), `qnc-script`
(`DRAG_EDGE`, `drag_edge`), katalog akcija `subsegment_drag_edge` (bez tipke). Testovi, conformance. Uzivo nije provjereno.
Dopuna 2026-10-29 (korisnik: "nastavi s filmstripom dijelova u SubSegmentu"): timeline SubSegmenta na V1 pokazuje
filmstrip svakog dijela po njegovom IN/OUT-u, iza razmaka na svom mjestu, istom kockom kao Wrap redovi
(`qnc-program-filmstrip` `ProgramFilmstrips`, spremljeni filmstripovi kroz citac artefakata previewa, citani u pozadini;
klip bez filmstripa nema slike dok ga radnik ne napravi). `qnc-script` (`subsegment_pictures`, `SubsegmentView.pictures`;
nacrt ruba pomice i slike), `qnc-subsegment-panel` (`TimelinePicture`). Test
`the_filmstrip_of_a_subsegment_lays_each_part_after_its_pause`, conformance. Uzivo nije provjereno.
Popravak 2026-10-29 (korisnik: "pogledaj log... ne kreira SubClip od transkodiranih klipova"; log: `short-save refused
... Mironik 3452 is not imported`): u bazi aktivnog projekta klip je `imported` (procitano samo citanjem), ali AI Media
Assist je katalog procitao prije nego je uvoz zavrsio (katalog forme se cita ponovno tek kad se povrsina aktivira), pa je
redak poola ostao neuvezen i `qnc-short-save` je odbijao SubClip. Sada `qnc-short-save` ne odlucuje po retku poola: je li
klip uvezen provjerava baza u istoj transakciji kao upis (`qnc-virtual-shots`, vec postojeca provjera). Test
`nothing_is_written_without_a_clip_marks_and_a_database`. Nije izvedeno: oznaka stanja klipa u poolu AI Media Assista
ostaje stara dok se povrsina ponovno ne aktivira.
Dopuna i popravak 2026-10-29 (korisnik: "da, dodaj povremenu provjeru kataloga"; "kreirani subklip se ne dozvoljava u
zumu fino podesiti, stalno se vraca na originalni rez"): (1) Otvorena Story/MA forma svake 2 s usporeduje lagani potpis
kataloga s bazom u pozadinskoj niti (`qnc-active-project-read` `catalog_changed`, `CATALOG_CHECK`; samo signature, bez
kartice, slika i medija, §5) i katalog cita ponovno samo kad se razlikuje. (2) Log: IN otvorenog SubClipa uzet (Ctrl+I) i
pomaknut 1909 -> 1915 pa pusten Enterom, ali nijedna operacija nije postojala za izmjenu SubClipa, pa se ponovnim otvaranjem
vracao na 1909. Sada kad se uzeti IN ili OUT otvorenog SubClipa pusti (Enter, klik), rez razlicit od spremljenog zapisuje se
kao njegov novi IN/OUT (`qnc-short-save` `follow`, zapis u log `short-save fine tuning`), a slike IN/OUT se rade ponovno;
nove oznake postavljene s I i O bez uzimanja i dalje su novi SubClip (Add SubClip). `qnc-virtual-shots` operacija
`TrimShort` (`trim_short_now`: samo klasa `short`, OUT iza IN-a, unutar klipa, uvezen klip, slike `pending`), test
`a_fine_tuned_short_keeps_its_new_cut_and_its_stills_are_made_again`; `qnc-virtual-short-cards` `chosen`. C10 Story
aplikacija 504 (isto). Debug cache obrisan (`cargo clean --profile dev`, 29,2 GiB; disk je imao 0 B slobodno). Testovi,
conformance, release build. Uzivo nije provjereno. Nije izvedeno: povlacenje oznake SubClipa misem bez uzimanja ne
mijenja SubClip.
Popravak 2026-10-29 (korisnik: "negdje usput nestao audio vu-metar"; odabrao "panel transporta"): metar nije nestao iz
koda, od 16. 10. bio je samo u traci panela Segmenti (Story), pa ga MA forme (log: rad u MA Audio, SubSegment) nisu imale.
Sada ga ima i panel transporta ispod monitora u svakoj formi (Ingest, MA, Story), lijevo od Play/IN/OUT/Auto/On: razina
izlaza A1/A2 koju player mjeri na uredaju (`qnc-transport-panel` `show(.., levels)`, `qnc-level-meter`); u Storyju ostaje i
u Segmentima. Mjesto transporta sire je u oba ugovora rasporeda (`transport_width` 290 -> 380). Story/MA ploca predaje
`preview.output_levels`, Ingest ploca razine iz `playback` (isti broj redaka). Testovi, conformance, release build. Uzivo
nije provjereno.
Popravak 2026-10-29 (korisnik: "vu-metar se nalazio na traci iznad source timelinea, pogledaj povijest"): povijest pokazuje
jedno mjesto metra, traku panela Segmenti; panel bez segmenata (od 24. 9.) crtao je samo poruku i nije crtao traku, pa u
prici bez segmenata metra i NORM nije bilo. Sada je traka na dnu panela i bez segmenata (`qnc-segment-panel`). Testovi,
conformance, release build.
Vraceno 2026-10-29 (korisnik: "makni metar ispod monitora"): metar u panelu transporta (462b506) uklonjen, `transport_width`
opet 290. Metar je samo na traci panela Segmenti, kao u verzijama od 28. 10.
Izmjena 2026-10-29 (korisnik: "on je trebao biti kocka"; "da, u svim formama"; "vu-metar ne smije biti dio kocke source
timeline, ta kocka koristi vu-metar kocku"): VU-metar je kocka `qnc-level-meter`; kocka `qnc-source-timeline` je koristi i
postavlja na traku iznad timelinea (zaglavlje source docka, iza IN/OUT/Duration, prije gumba forme), pa ga ima svaka forma
sa source timelineom (Ingest, MA, Story). Razine izlaza A1/A2 uzima iz previewa (`SourceTimelineView.levels`, u `sync`).
Traka panela Segmenti vise nema metar (bio bi drugi odmah iznad), NORM ostaje; `levels` uklonjen iz `qnc-segment-panel`,
`qnc-right-panel` i Story ploce. Forme nisu narasle. Testovi, conformance, release build.
Vraceno 2026-10-29 (korisnik: "nevalja, analiziraj polozaj u zadnjoj pushanoj verziji"): u `origin/libav-decoder`
(`2627516`, 28. 10.) metar (kocka `qnc-level-meter`) je na traci panela Segmenti, u sredini, lijevo od NORM, iznad reda
programa na dnu panela. Metar na traci source timelinea (dface57) uklonjen; stanje je kao u pushanoj verziji, uz traku i
kad prica nema segmenata (8fa1de4).
Izmjena 2026-10-29 (korisnik: "sada tu kocku prebaci na sredinu source timeline, zadrzi dimenzije zajedno s dugmetom
NORM"): nova kocka `crates/qnc-level-bar` (+ ugovor `contracts/modules/level-bar.module.json`): metar (kocka
`qnc-level-meter`) i NORM desno od njega, 240 x 22 kao na traci Segmenti, centrirano u retku koji pozivatelj da; klik na
NORM je `toggle_loudness_normalize`. Kocka `qnc-source-timeline` je postavlja u sredinu svoje trake iznad timelinea, u
svakoj formi; razine iz previewa (`SourceTimelineView.levels`), stanje NORM-a daje fokus Storyja/MA
(`offer_normalize`, `qnc-panel-focus`), Ingest nema pricu pa je NORM ondje ugasen. Traka panela Segmenti vise nema metar
ni NORM (srednja dugmad i dalje skrivena); `levels` uklonjen iz `qnc-segment-panel`, `qnc-right-panel` i Story ploce.
Testovi, conformance, release build.
Dopuna 2026-10-29 (korisnik: "nastavi s premjestanjem i brisanjem dijela SubSegmenta"): svaki redak dijela ispod
SubSegment timelinea ima `<` i `>` (dio mijenja mjesto s prethodnim ili sljedecim) i Delete (dio izlazi iz SubSegmenta,
klip ostaje u projektu). Pauze i spojevi (rez/crossfade) ostaju na svojim mjestima izmedu dijelova; dio koji postane prvi
nema pauzu ni spoj. Zadnji dio bloka se ne brise sam (poruka `db.script_last_piece`: ukloni se cijeli blok).
`qnc-script-db` operacije `MovePiece`, `RemovePiece` (`move_piece`, `remove_piece`, test
`a_part_moves_past_its_neighbour_and_leaves_the_joins_stay_in_their_places`), `qnc-script` radnje `subsegment_part_move`,
`subsegment_part_remove` (upis odmah, SubSegment svira ponovno, uzeti rub ili zvuk dijela se pusta), katalog akcija bez
tipki, `qnc-subsegment-panel`, rjecnik. Testovi, conformance, release build. Uzivo nije provjereno.
Popravak i dopuna 2026-10-29 (korisnik: "nastavi sa SubSegmentom kao virtualnim klipom u poolu"; "subsegment timeline ne
prihvaca klik na filmstrip za postavljanje playholdera"): (1) klik na sliku dijela na SubSegment timelineu davao je izbor
segmenta (`SelectVirtual`), koji panel nije citao; sada je playhead kao klik na redak (`qnc-subsegment-panel`). (2) MA Audio
(grupa g) ima u poolu tabove All, SubClip i SubSegment (`contracts/ui/editorial.layout.json`, akcija kataloga
`editorial_tab_subsegment` bez tipke). SubSegment je virtualni klip scenarija, ne nova tablica: tab pokazuje blokove
izjave od vise dijelova (`qnc-script` `ScriptView::pool_cards`, `SubsegmentCard`: oznaka, prvi klip, broj dijelova,
duljina, poster prvog klipa), klik ga otvara na mjestu source timelinea (`script_open`). Tab je siv bez SubSegmenta
(`qnc-pool-tabs` `LibraryTab::Subsegment`, `Contents.subsegments`), prazna poruka u ugovoru
(`subsegment_empty_message`). C10 Story/MA ploca 94 -> 93. Testovi, conformance, release build. Uzivo nije provjereno.
Nije izvedeno: Ton segment u Storyju iz SubSegmenta (Story nema scenarij, pa ni tab).
Izmjena 2026-10-29 (korisnik: "izbaci iznad source timeline dio koji si dodao, nepotreban je"): SubSegment timeline vise
nema redak naslova ("SubSegment: n parts, n frames" i Close) iznad sebe; source timeline vraca klik na klip u poolu
(`qnc-subsegment-panel`; sifre `subsegment.title`, `subsegment.close` uklonjene iz rjecnika).
Izvedeno 2026-10-29 (korisnik nakon audita: "gdje se izgubio J i L"; "da, napravi tako, ali dodaj i selekciju misom i
povlacenje ili srednjim tockom misa"): nalaz: od 21. 10. rez tona M markera imao je samo jedan pomak (L pokrivalice prije
ILI J pokrivalice poslije), pa J i L zajedno (preklapanje dvaju ambijenata) nije bilo moguce zadati, a operacija baze
`SetCoverOverlap` ostala je bez pozivatelja. Sada ton reza ima dvije strane, samo u zoomu (pravilo 22. 10.): Ctrl+M pa
Ctrl+2 uzima ton reza (obje strane: jedan rez tona, a kad postoje i J i L, preklapanje se mice cijelo), Ctrl+O samo L
pokrivalice prije (gdje njezin ton zavrsava), Ctrl+I samo J pokrivalice poslije (gdje njezin ton pocinje), Ctrl+2 opet
obje; lijevi klik na crvenu crtu strane na A2 uzima tu stranu, povlacenje je pomice, kotacic i strelice kao do sada;
svaki korak se odmah cuje, Enter sprema obje strane u jednom koraku Undo, Esc odustaje. Na A2 su obje strane crvene
(uzeta debela), spojene na vrhu A2; spremljeni rez s J i L ostaje vidljiv. `qnc-program-db` operacija `SetCutSoundEdges`
(`set_cut_sound_edges`, `cut_edges`); `MoveMarkerPicture` cuva oba ruba tona na svom mjestu u prici (test promijenjen:
nakon L 3 i pomaka slike 14 -> 18 pokrivalica poslije zadrzava pocetak tona na 14, J 4; prije J 1). `qnc-program-segments`
`cut_part.rs` (`SoundEdge`, `CutSound`, `take_sound_edge`, `take_sound_edge_at`, `drag_sound_edge`, `cut_sound_frames`),
`qnc-program-commands` (`TakeCutEdge`, `DragCutEdge`), `qnc-program-view` (`MarkerPin.sound_tail/sound_lead`,
`TakenPart::SoundLead/SoundTail`), `qnc-timeline` (`TimelineMarkerPin.sound_tail/sound_lead` umjesto `sound_at`,
`TimelineIntent::TakeCutEdge`, `take_cut_edge_drag`, `paint_sound_sides`; dok je ton uzet, povlacenje mice samo stranu
tona, nikad sliku), `qnc-mouse-map` i `contracts/input/mouse.json` (`click_cut_edge` -> `timeline_take_cut_edge`,
`drag_cut_edge` -> `timeline_drag_cut_edge`; mapa misa prosirena uz odluku korisnika), katalog akcija (bez tipki),
`qnc-segments-keys` (`cut_edge`, Ctrl+I/O dok je ton uzet), `qnc-panel-focus`, `qnc-segment-panel`, rjecnik. Testovi
`a_cut_has_the_j_of_the_cover_after_and_the_l_of_the_cover_before_together`,
`the_j_and_the_l_of_a_cut_move_apart_and_are_written_together`, svi testovi dotaknutih kocki, conformance, release build.
Uzivo nije provjereno. Nije izvedeno (sljedeci korak, korisnik: "ton moze biti A1 ili A2"): J i L tona na A1 (spoj dvaju
segmenata), baza jos nema mjesta za njih.
Dopuna 2026-10-29 (korisnik, sa slikom zooma: "treba tu opciju dodati i u zoom overlay"): dok je ton reza uzet (Ctrl+M
pa Ctrl+2), prozor trim zooma na A2 pokazuje J (gdje ton pokrivalice poslije pocinje) i L (gdje ton pokrivalice prije
zavrsava) crveno, uzetu stranu debelo, spojene na vrhu A2; prozor obuhvaca obje strane i sekundu oko njih. Klik blizu crte
uzima tu stranu, povlacenje je pomice (iste sistemske radnje kao na Wrap retku: `timeline_take_cut_edge`,
`timeline_drag_cut_edge`), kotacic kao strelice. `qnc-program-view` `ZoomSound.cut_sound` (`ZoomCutSound`),
`qnc-program-segments` (`zoom_cut_sound`, prozor zooma), `qnc-trim-overlay` (`paint_cut_sound`), ugovor modula. Testovi,
conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-10-29 (korisnik: "nastavi s J i L na A1... treba omoguciti da se i na zumu mogu selektirati audio kanali za
J/L"; "svaki rez je marker, spoj dva klipa cini marker, OUT prethodnog i IN trenutnog"; "prostor izmedu dva markera zovemo
slot"): rez dvaju klipova na A1 ima J i L kao rez M markera na A2. `qnc-program-db`: `story_parts.a1_lead_frames` (J zvuka
klipa poslije reza) i `a1_tail_frames` (L zvuka klipa prije), stare price dobivaju 0; operacija `SetJoinSound`
(segment poslije reza: njemu J, prethodnom L, jedan Undo korak; lead i rez bez prethodnog klipa se odbijaju,
`story.join_none`). `qnc-program-playlist` `cross_a1`: na rezu se A1 dvaju klipova mijesa preko prozora J..L koliko izvori
imaju slika (`join_a1` SubSegmenta koristi isto). `qnc-program-input` (`SoundDraft.joins`: nacrt se cuje prije Entera).
`qnc-program-view`: rez dvaju klipova bez M markera na istom mjestu je pin `join:<segment poslije>` (`join_pins`,
`SegmentsView.joins`), ne omeduje slot, ne pomice se cijeli ni po slici (slika ide Editom segmenta), brisanje se odbija.
`qnc-program-segments` (`cut_part.rs`): rez se uzima Ctrl+M ili desnim klikom, u zoomu Ctrl+1 uzima njegov A1 zvuk
(Ctrl+2 A2 zvuk M markera), Ctrl+I samo J, Ctrl+O samo L, strelice/kotacic/povlacenje pomicu, Enter zapisuje, Esc
odustaje. `qnc-timeline`: rez dvaju klipova crta se samo dok je uzet ili dok mu je A1 zvuk odmaknut od slike; strane A1
crveno na traci A1 (iznad slike), A2 na traci A2; klik i povlacenje strane rade na traci uzetog zvuka (`lane_band`,
`cut_side_at`). Zoom reza (`qnc-trim-overlay`): oznake A1 i A2 su gumbi dok je rez uzet (`ZoomSound.taken_cut`), klik je
`select_audio_a1`/`select_audio_a2` na taj rez (kao Ctrl+1/Ctrl+2), traka uzetog zvuka ima crveni okvir oznake, strane
reza crtaju se i hvataju i na A1. Panel Segmenti predaje i reze dvaju klipova (`qnc-segment-panel`). Rjecnik:
`story.msg.join_taken`, `story.msg.join_moves_with_edit`, `cut_sound_*` s `{lane}`. Testovi `qnc-program-db` (48),
`qnc-program-playlist` (21), `qnc-program-segments` (62, novi `the_a1_sound_of_a_cut_between_two_clips_has_a_j_and_an_l`),
conformance, release build. Uzivo nije provjereno. Nije izvedeno: J/L reza koji pada na granicu Wrap redova crta se samo na
retku na kojem je strana (drugi redak ne pokazuje stranu iza granice).
Popravak 2026-10-29 (korisnik: "pogledaj log, ne rade ti gumbi A1 ni A2"): log: klik na A1/A2 u zoom prozoru reza stizao je
do price (`CutPart` s uzetim markerom), ali ga je odbijalo pravilo od 22. 10. (ton ili slika reza samo u uvecanom slotu),
jer se zoom prozor reza vidi i dok je marker uzet na Wrap retku. Sada gumb zoom prozora koji imenuje vec uzeti rez uzima
njegov ton i izvan uvecanog slota (`qnc-program-segments` `take_cut_part`, `from_trim_zoom`); tipke Ctrl+1/Ctrl+2 na Wrap
retku i dalje pomicu cijeli marker. Klik na A1 uz M marker usred segmenta i dalje kaze da tamo A1 nema rez (dva klipa se ne
spajaju). Test `the_lane_buttons_of_the_trim_zoom_take_the_sound_of_the_taken_cut`, conformance, release build.
Otkljucano i izvedeno 2026-10-29 (korisnik: "zoom treba jednako raditi na svim formama gdje postoji marker, ne samo na
pokrivalicama... kako cemo podvuci ton izjave pod pokrivalicu s J, ili staviti sliku tona preko offa"; "marker predstavlja
timecode... klipu X ton pocinje na timecode Y, slika Z"; "ok, odkljucaj"; zatim "zoom ne treba pokazivati timekodove,
timekodovi su alat za opis reza, u bazu zapisujemo timekodove"): rez spoja dvaju klipova je rez kao M marker, a rez su OUT
klipa prije i IN klipa poslije, slika i ton svaki svoj. (1) Cijeli rez spoja (Ctrl+M, desni klik, strelice, kotacic) je roll:
`qnc-program-db` operacija `RollJoin` (OUT segmenta prije i IN segmenta poslije za isto, svaki u svojim slikama izvora; prica
zadrzava duljinu; marker izmedju starog i novog reza se odbija `story.join_past_marker`, marker na rezu ide s njim; jedan Undo
korak); na Wrap retku rez ide samo izmedju susjednih rezova (markeri i spojevi). (2) Samo slika spoja (u zoomu Ctrl+I/O kao kod
M markera): slika klipa poslije pocinje ranije (J slike, `v_lead_frames`) ili slika klipa prije traje dulje (L slike, novi
stupac `story_parts.v_tail_frames`, stare price 0), ton ostaje; operacija `SetJoinPicture` (oba u jednom Undo koraku); slika
Offa je crno. `qnc-program-playlist` `picture_lead.rs` slaze J i L slike za Ton i Off. Nacrt se vidi i cuje prije Entera
(`qnc-program-input` `SoundDraft.pictures`, `qnc-program-segments` `join_picture_draft`), razdvojena slika i ton crtaju se
kao do sada (`a1_splits`, sada za J i L slike). (3) Uklonjen zaseban put J slike: Ctrl+Shift+J (akcija
`select_segment_picture_lead` iz kataloga i pomoci), `overlap.rs`, operacija `SetSegmentPictureLead`; J slike Ton/Off je
sada slika reza spoja. Ton reza (A1 na spoju, A2 na M markeru) ostaje kako je bio. Timecodeovi se ne prikazuju u zoomu
(pokusaj s kockom `qnc-cut-edges` uklonjen na zahtjev korisnika). Ugovor `story.database.json` (`picture_lead`, `join_roll`),
rjecnik. Nije izvedeno: M marker koji stoji tocno na spoju i dalje mice pokrivalice, ne klipove; slike u zoomu za sliku spoja
pokazuju spremljenu pricu, ne nacrt L/J slike. Testovi `qnc-program-db` (50), `qnc-program-playlist` (22),
`qnc-program-segments` (63), `qnc-program-input`, zlatna tablica tipki, conformance. Uzivo nije provjereno.
Odluka i izvedba 2026-10-29 (korisnik: "pokrivalice necemo tretirati kao novi sloj nego kao overwrite slike i A2, tako cemo
sve imati u jednom sloju i moci raditi sve vrste rezova, isti sustav kao program timeline / ravna playlista"; "skoro nista se
ne mijenja, samo pokrivalica radi overwrite"; "da, kreni s prvom tockom, ali trenutno stanje pushaj kao punu verziju"):
stanje prije je pushano kao grana `libav-decoder` (d38ed3a) i oznaka `v-2026-10-29-pre-overwrite-covers`. Baza, segmenti,
markeri i slotovi ostaju; pokrivalica na svom rasponu prepisuje sliku programa i A2. Korak 1: rez slike postoji samo gdje se
slika programa mijenja (`qnc-program-segments` `no_picture_cut`): spoj dvaju klipova ispod pokrivalice ima samo rez A1
(`story.msg.join_under_cover`), M marker izmedju dva prazna slota nema rez slike ni A2, samo omeduje slotove i mice se cijeli
(`story.msg.marker_no_picture_cut`); razdvojena slika spoja ispod pokrivalice se ne crta. Test
`a_picture_cut_is_only_where_the_picture_changes`, conformance. Nije izvedeno: korak 2 (rez na rubu pokrivalice s istim
dijelovima kao spoj: roll i slika prema segmentu ispod), korak 3 (prikaz). Uzivo nije provjereno.
Izvedeno 2026-10-29 (korisnik: "nastavi s drugom tockom, ali dodaj da postoje dvije vrste undo: vremenski undo, vracanje po
vremenskom slijedu, i undo selected: ako je nesto selektirano da se to undo ili redo bez obzira kad je bilo"): (1) Druga tocka
(rez na rubu pokrivalice kao spoj klipova): uz overwrite pokrivalice postojece operacije vec to daju (cijeli rez M markera
mijenja pokrivalicu i otkriva sliku segmenta ispod, samo slika `MoveMarkerPicture`, A2 J/L na strani s pokrivalicom); bez
nove izmjene. (2) Undo selected (v5 `story/object_history.rs` je imao samo jednu razinu za pokrivalicu): `qnc-program-db`
operacije `UndoSelected` / `RedoSelected` (`object_type` segment, marker, cover; novi modul `object_undo.rs`): koraci
vremenskog undoa su povijest; undo selected trazi najnoviji korak koji je mijenjao taj objekt i vraca samo njegov red kakav je
bio prije, sljedeci trazi stariji; redo selected vraca ono sto je undo uzeo; povijest objekta je u `story_object_history`
(korak do kojeg je dosao i uzeti redovi), brise se kad ga druga izmjena promijeni, a sva kad se radi vremenski Undo/Redo. Undo
selected je i sam izmjena price, pa ga vremenski Undo ponistava; nakon njega slotovi se slazu iznova, pravilo "bez rupe"
vrijedi. Odabrani objekt (`qnc-program-segments` `selected_object`): uzeti rez (M marker; za spoj klip poslije), uzeta ili
izabrana pokrivalica, izabrani marker, pokrivalica izabranog slota, izabrani segment. Katalog (QNC preset, storyboard):
`undo_selected` Ctrl+Backspace, `redo_selected` Ctrl+Shift+Backspace, grupa pomoci "Općenito"; gumbi "Undo sel." i
"Redo sel." u traci panela Segmenti lijevo od Undo/Redo (ukljuceni kad je nesto izabrano). Testovi
`undo_selected_takes_back_the_changes_of_one_object_whenever_they_were`, `undo_selected_takes_the_selected_object`,
conformance. Nije izvedeno: undo/redo misem. Uzivo nije provjereno.
Odbaceno i izvedeno 2026-10-30 (korisnik: "nisu ni geste ni mis adekvatni na predlozeni nacin"; "dva prsta na touchpadu
igra ulogu Ctrl+, sto znaci da selektira markere, isto radi i desni klik misa"; "napravi audit komandi"; Backspace "undo
selektiranog"): undo okomitim povlacenjem misa i okomitim swipeom touchpada (8c52904) vracen (2e4e145). Backspace (i
Ctrl+Z, Ctrl+U, gumb Undo: ista akcija `undo_object`) radi undo izabranog kad je nesto uzeto ili izabrano (uzeti rez,
uzeta ili izabrana pokrivalica, izabrani marker, pokrivalica izabranog slota), inace vremenski undo; Shift+Backspace isto
za redo. Izabrani segment ne racuna se (uvijek postoji). Obican klik na Wrap redak pusta izabrani marker kao i izabranu
pokrivalicu, pa Backspace opet radi vremenski undo. `qnc-program-segments` (`picked_object`, `commands.rs`). Ctrl+Backspace
ostaje undo izabranog ukljucujuci segment. Selekcija: desni klik misa i dodir s dva prsta na touchpadu (Windows ga salje
kao desni klik) su Ctrl+ (marker cijeli, traka pokrivalice = slot, pokrivalica, A1/A2), vec po mapi misa. Test
`backspace_undoes_what_is_picked_and_otherwise_in_time`, conformance. Uzivo nije provjereno.
Dopuna 2026-10-30 (korisnik: "u zumu na sredini playheada ispod slika postavi tri dugmeta Undo OK Cancel, tako cemo
izbjeci zabune postavljanja playheada i spremanja"; "OK ili Save ili Done, zavisi koju terminologiju koristis"): prozor trim
zooma (Wrap redovi, source timeline, SubSegment) ispod slika ima red Undo | Save | Cancel, centriran na crti reza (unutar
prozora). Svako dugme je ista radnja kao tipka na panelu u fokusu: Undo = Backspace (`undo_object`), Save = Enter
(`activate_focused_item`), Cancel = Esc (`clear_focus`). Naziv Save jer aplikacija Enter vec zove spremanjem.
`qnc-trim-overlay` (`paint_buttons`, ovisi o `qnc-ui-text`), rjecnik `common.save`. Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-10-30 (korisnik: dugmad "treba biti centrirano iznad playheada"): Undo | Save | Cancel stoje iznad crvene crte
reza kroz A1/A2 (`cut_line_x`), bez vala na crti izmedju slika.
Dopuna 2026-10-30 (korisnik: "kada se pokrene play u zumu bi se trebao kretati sinhrono playhead"; "odnosno wave bi se trebao
pomicati"; "playhead stoji, wave putuje u zumu"): dok player svira, prozor zooma (sekunda sa svake strane) je centriran na
potvrdjenoj slici playera, pa bijela crta playheada stoji u sredini, a val i rijeci putuju ispod nje; crvena crta reza crta se
samo dok je rez u prozoru. U pauzi prozor je opet oko reza. Os playera mora biti os zooma: klip za source IN/OUT, program za
Wrap redove i SubSegment. `qnc-source-preview` `playing_at`, `qnc-program-view` `zoom_window` i `ZoomSound.playhead`,
`qnc-program-segments` `SourcePick::with_playing`, `qnc-script` `subsegment_zoom` (rate, playhead), `qnc-trim-overlay`;
aplikacija isti broj redaka. Fini val cita se s medija za prozor koji se mice, pa moze kasniti iza playa. Testovi,
conformance. Uzivo nije provjereno.
Popravak 2026-10-30 (korisnik: "kada se play pauzira, playhead na zumu treba stati na tom mjestu, a ne vratiti na IN marker...
nisu sinhroni"; "kada pomicemo misem ili strelicama bude sinhroni, ali u playu nisu"; log: fini val citao se tek kad je prozor
vec odmakao, 0,3-1,9 s po citanju, pa je val za vrijeme playa bio prazan): (1) nakon playa prozor ostaje na slici na kojoj je
player stao, dok se rez ne pomakne (`qnc-program-view` `ZoomFollow`; `SourcePreview::playing_at` daje i stoji li player);
pomak reza vraca prozor oko reza. (2) Dok prozor prati play, zvuk se cita deset prozora unaprijed (`zoom_reach`;
`qnc-trim-stills`; source zoom ima cijeli klip kao komad, Wrap i SubSegment komade do tog dosega). Debug cache obrisan uz
dozvolu korisnika (`cargo clean --profile dev`, 36,5 GiB). Uzivo nije provjereno.
Audit 2026-10-30 (korisnik: "store sync/b-roll nekad prihvati nekad ne, nije stabilno, provjeri procedure i log"): log
zadnjih Syncova: Enter je svaki put spremio pokrivalicu kad je pritisnut (13:20 i 13:24, program ponovno otvoren nakon
upisa); neuspjesi su bili prije Entera. (1) IN pritisnut dok je Sync pauziran ili zaustavljen bio je odlozen: Space je
nastavio stari Sync, a Enter bi spremio stari IN (playhead 0 umjesto 56 i 94). (2) Klip otvoren iz poola s vlastitim IN-om
naoruzavao je Sync bez pritiska I, a IN naoruzan na jednom klipu ostajao je kad se pokaze drugi, pa je Space pustao Sync
tudjeg IN-a na prikazanom klipu. (3) Poruke su govorile "Delete discards", a od 21. 10. Sync nacrt odbacuje Esc; Delete u
poolu nije radio nista i nista nije rekao. Popravljeno: novi IN (I, rez ili pomak IN-a istog klipa) odbacuje stari Sync
(nista nije bilo upisano) i naoruzava novi; izbor drugog klipa ne dira Sync koji radi ili ceka Enter, a IN naoruzan na
drugom klipu se zaboravlja (`qnc-program-segments` `arm_sync_on_new_in`, `qnc-sync-cover` `forget_other_clip`); poruke
kazu Esc (rjecnik `sync_resumes`, `sync_at_end`, `sync_stopped`); Delete na svakom panelu dok Sync nacrt ceka kaze da ga
odbacuje Esc (`qnc-panel-focus`). Test `a_new_in_drops_a_paused_or_stopped_sync_and_another_clip_does_not_arm`,
zlatna tablica tipki, conformance. Uzivo nije provjereno.
Dopuna 2026-10-30 (korisnik: "J marker radi samo na A2, treba raditi i na A1 i V, isto tako L"; log: A1 na M markeru izmedju
dvije pokrivalice odbijen, "No two clips meet on A1 at this cut"; izbor "1, kreni"): (1) A1 u zoomu na M markeru unutar
jednog segmenta uzima najblizi rez gdje se spajaju dva klipa i njegov A1 (J/L kao do sada: Ctrl+I J, Ctrl+O L, strelice,
kotacic, Enter, Esc), umjesto odbijanja; samo dok na markeru nista nije pomaknuto (inace ostaje poruka kao prije)
(`qnc-program-segments` `take_cut_part`). (2) Zoom reza ima gumb V gore lijevo na slikama dok je rez uzet: klik uzima samo
sliku reza (`select_mark_in` na taj rez, isto kao Ctrl+I u zoomu); uzeta slika ima crveni okvir gumba
(`qnc-program-view` `ZoomSound.picture_taken`, `qnc-trim-overlay` `paint_take_button`, isti za A1/A2/V). Test
`a1_of_an_m_marker_within_a_segment_takes_the_nearest_cut_of_two_clips`, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-30 (korisnik: "po novome baza ti moze biti zajednicka, a stupac vrsta klipa ti govori sto je"; "slika
tona moze uci u off i na pocetku i na kraju, audio tona A1 ne moze uci u off, samo A2 kao J ili L"; odluke: A2 Tona je
tih dok se ne produlji u Off, mijesanje s pokrivalicom provjerit ce se u testu, flat playlista, nema migracije): stanje
prije je pushano (oznake `v-2026-10-30-pre-shared-clip-table`, `v-2026-10-30-db-schema`). (1) Segmenti i pokrivalice su
redovi jedne tablice `story_items` (`role` segment | cover; segment: `kind` tonovi/offovi, `sort_index`, `lead`, `active`;
pokrivalica: `slot_id`, `timeline_*`; oba: izvor `source_in_frame`/`source_out_frame`/`source_fps_*`, slika J/L, zvuk A1 i
A2 s kanalom, razinom, off, J i L); `story_parts` i `story_covers` uklonjene, javni pogledi `public_story_parts` i
`public_story_covers` ostaju (citaju `story_items`), novi `public_story_items`. Prica starijih tablica se brise, ne pretvara
(i njezini markeri, slotovi, povijest i undo koraci). `qnc-program-db` (svi SQL upisi i citanja, undo i undo izabranog na
novoj tablici), `qnc-virtual-shots` cita pokrivalice kroz `public_story_covers`. (2) Novi segment pamti A2 zadrzan na source
timelineu (`CreateSegment.a2_*`, `NewSegment`). A1 Tona ne ulazi u Off: `SetJoinSound` odbija L Tona prije Offa i J Tona
poslije Offa (`story.ton_a1_not_into_off`), a uredivanje reza A1 na tom spoju nema tu stranu (`qnc-program-segments`
`cut_sides`). A2 Tona ulazi u Off kao L (Ton prije Offa, `a2_tail_frames`) ili J (Ton poslije Offa, `a2_lead_frames`):
nova operacija `SetJoinA2` (jedan Undo korak; drugi spojevi odbijeni `story.ton_a2_only_into_off`); u zoomu reza spoja
Ctrl+2 (ili A2) uzima taj zvuk, strelice/kotacic/Ctrl+I/O kao kod M markera, Enter zapisuje. `qnc-program-playlist`
`ton_a2.rs`: A2 Tona je tih ispod Tona i cuje se samo u Offu, iz izvora iza OUT-a (L) ili prije IN-a (J), na svom kanalu i
razini, s fadeom na vanjskom kraju (`audio.crossfade_frames`); gdje A2 drzi pokrivalica, ona ostaje (pokrivalica radi
overwrite A2). Slika Tona u Off vec postoji (J/L slike spoja, 29. 10.). Ugovori `story.database.json` (`story_items`,
`ton_into_off`), `program-db`, `program-input`, `program-segments`, rjecnik. Testovi `qnc-program-db` (52, novi
`only_the_a2_of_a_ton_goes_into_an_off_its_a1_does_not`), `qnc-program-playlist` (23, novi
`the_a2_of_a_ton_is_heard_only_where_it_goes_into_an_off`), `qnc-program-segments` (68), conformance. Nije izvedeno: nacrt A2
Tona prije Entera (cuje se nakon upisa), mijesanje A2 Tona s pokrivalicom u istom Offu (sada pokrivalica preuzima), crtanje
J/L A2 Tona na Wrap retku. Uzivo nije provjereno.
Dopuna 2026-10-30 (korisnik: "dodaj nacrt A2 Tona prije Entera"): dok je A2 spoja Ton/Off uzet u zoomu, svaki korak
(strelice, kotacic, povlacenje strane) odmah svira pricu s nacrtom J/L A2 Tona; Esc vraca spremljeno, Enter zapisuje.
Isti put kao nacrt J/L pokrivalica: `qnc-program-input` `SoundDraft.overlaps` vrijedi za pokrivalice i segmente (jedna
tablica, id-evi se ne preklapaju), `qnc-program-segments` `cut_sound_draft` daje dva klipa spoja s nacrtom. Testovi
`the_a2_of_a_ton_goes_into_the_off_before_it_and_is_drafted_before_enter`, `a_sound_draft_changes_only_its_own_segment_or_cover`
(prosiren), conformance. Uzivo nije provjereno.
Dopuna 2026-10-30 (korisnik: "dodaj crtanje J/L A2 Tona na Wrap retku"): spremljeni J/L A2 Tona u Off crta se na Wrap
retku kao kod pokrivalica: rez spoja crven preko slike, strana A2 crveno na traci A2 (`qnc-timeline`: rez dvaju klipova
crta se i kad mu je A2 odmaknut, ne samo A1). Rez na granici redova crta se i na pocetku sljedeceg reda kad mu strana
zvuka (L) ide u taj red (`qnc-segment-panel` `program.rs`; vrijedi i za L pokrivalice i A1). Strane dolaze iz
`cut_sound_frames` (od prethodnog koraka). Test `the_stored_a2_of_a_ton_into_an_off_is_on_the_cut_between_them`,
conformance. Uzivo nije provjereno.
Izvedeno 2026-10-30 (korisnik: "aplikacija treba znati jesu li snimljeni i gdje offovi, mora ih imenovati pravilno kako bi
ih mogla ukljuciti u draft"; "ako korisnik vise puta snima isti off, dosnimava ga kao drugi kanal, treci, cetvrti... jedan
viskanalni zapis, a korisnik bira koji ce koristiti, kao A1 na video zapisu"; "da, kreni s prvim korakom"): korak 1, zapis
glasa. Svaki OFF blok ima jedan zapis: ime se daje kod prve snimke, `<projekt>_<oznaka bloka>` (`Mironik_OFF2`), i ostaje
kad se blokovi premjeste; svaka snimka je svoj WAV `<ime>_T<n>.wav`, a klip `<ime>.mxf` se iznova slozi poslije svake
snimke sa svim snimkama kao mono kanalima (snimka n je kanal n, svi od pocetka klipa, kraca zavrsava tisinom, klip traje
koliko najdulja, timecode prve snimke; najvise 8). Klip se radi u privremenoj mapi sustava i pise preko svoje datoteke u
mapi projekta (mapa ne dopusta brisanje ni preimenovanje). Esc dok snimka traje baca je: nista se ne upisuje, njezina
datoteka ostaje prazna i sljedeca snimka pise preko nje. `qnc-script-db`: tablice `script_voice` (ime, klip, izabrana
snimka) i `script_voice_takes` (WAV, duljina, timecode, tekst bloka kad je snimano), pogledi, operacije `ReadVoices`,
`AddVoiceTake` (samo sljedeci broj, najvise 8, ime i klip ostaju od prve snimke; nova snimka postaje izabrana; blok cuva
klip, duljinu izabrane snimke i timecode prve), brisanje bloka brise i zapis (datoteke ostaju). `qnc-voice-take`
(`Earlier`, `record_name`, `cancel`), `qnc-voice-clip` (vise zvukova, `MAX_CHANNELS`), `qnc-voice-record` (`cancel`, prazna
ostavljena datoteka smije se pisati), `qnc-script` (snimka cita zapis bloka iz baze, upis kroz `add_voice_take`, Esc
`clear_focus`). Ugovori `script.database.json` (`voice`), `script-db`, `voice-clip`, `voice-take`, rjecnik. Provjereno:
testovi `qnc-script-db` (2 nova), `qnc-voice-take`, `qnc-voice-clip`, conformance; probni klip od dvije snimke (2 s i
3,3 s) u scratchpadu: 3 toka (MPEG-2 i dva mono PCM 24-bit), 3,3 s, kanal 1 tisina od 2 s, kanal 2 zvuk do kraja,
ponovno pisanje preko klipa radi. Nije izvedeno (korak 2): izbor kanala i stanje snimke u panelu scenarija; (korak 3)
Draft XML. Uzivo nije provjereno.
Izvedeno 2026-10-30 (korisnik: "da, kreni s korakom 2"): izbor snimke i stanje zapisa glasa u panelu scenarija. Svaki OFF
blok (snimana vrsta) ispod naslova pokazuje stanje: nije snimljen; snimljen; tekst promijenjen nakon izabrane snimke
(tekst bloka nije tekst s kojim je izabrana snimka snimljena); snimka nije u mapi projekta (klip ili WAV izabrane snimke
ne postoji na ovom racunalu). Uz stanje su gumbi T1..Tn, izabrana je osvijetljena, pod misem duljina, timecode i tekst
snimke; klik bira snimku (radnja kataloga `script_voice_take`, bez tipke, cilj `blok:snimka`). `qnc-script-db` operacija
`SetVoiceChosen` (samo snimka koju zapis ima; blok dobiva duljinu izabrane snimke), `qnc-voice-take` `voice_file`
(datoteka zapisa u mapi glasa projekta, nista ne cita), `qnc-script` (citanje zapisa i provjera datoteka u pozadinskoj
niti uz scenarij, `VoiceRow`, `VoiceStatus`), `qnc-script-panel`, rjecnik. Testovi `qnc-script-db` (izbor snimke),
`qnc-script` (stanje), conformance. Uzivo nije provjereno. Nije izvedeno: izbor snimke tipkama (strelice kao kod A1),
preslusavanje izabranog kanala (zapis glasa nije u katalogu projekta), korak 3 Draft XML.
Izvedeno 2026-10-30 (korisnik: "prije svega treba da klik na neki off pleja ton kako bi korisnik mogao odluciti koji ton
ima prednost"; "da, A, i tab Voice u poolu... slika off (broj)"; "nije fiksno XDCAM, projekt moze biti XAVC, 50i ili
50p"; "iz offa se koristi samo ton... bitan je ton i timecode i FPS"): zapis glasa je klip kataloga projekta vrste
`voice` i pusta ga Broadcast Player. `qnc-content-store`: stupac `clips.clip_kind` (`camera`|`voice`, postojeci klipovi
`camera`), `public_clips.clip_kind`, operacija `PublishVoice` (konacan zapis klipa vrste voice: uvezen gdje je, nikad
odabran ni u redu uvoza; zapis s jos jednom snimkom zamjenjuje prethodni pod istim id-em; klip kamere istog id-a se
odbija), popis i statistika kataloga i `qnc-content-read` (`summaries`, `signature`) ne pokazuju klipove glasa (pool kartica
i Ingest ih ne vide). `qnc-media-metadata` `EvidenceKind::QncWritten` (datoteka koju je QNC sam zapisao, bez probea).
`qnc-voice-clip`: `describe` (zapis kataloga iz onoga sto je zatrazeno od kodera: slika MPEG-2 4:2:2 BT.709 tocnog broja
slika, mono PCM 24-bit po snimci iste duljine, timecode prve snimke), `sample_rate` (zvuk u frekvenciji projekta) i
`fields`: slika slijedi projekt (`video.fps`, novi `qnc-work-settings` `video_fields` iz `video.field_order`; 1080i50 je
25 slika, isprepleteno, gornje polje prvo). Slika je samo pregled ("OFF n" na crnom): od OFF-a se koristi ton, timecode i
FPS, pa kodek slike ostaje XDCAM HD422 i za XAVC projekt (koder H.264 4:2:2 10-bit u ovom ffmpegu ne postoji). Nova kocka
`crates/qnc-voice-catalog` (+ ugovor): zapis glasa kao klip kataloga kroz jedinog pisca baze sadrzaja.
`qnc-voice-take` (`clip_id` = `voice-<naziv zapisa>`, `describe_record`). `qnc-script`: nakon snimke klip ulazi u katalog;
klik na T1..Tn (i karticu taba Voice) upise zapis u katalog (i zapis napravljen prije ove promjene), izabere snimku i
pusti klip na preview monitoru s A1 na kanalu te snimke (A2 isti kanal iskljucen); `qnc-source-timeline` `want_a1`
(traka A1 pokazuje i pamti kanal koji se cuje, kao izabrani kanal klipa). Tab Voice u poolu MA Audio (grupa g,
`editorial_tab_voice` bez tipke, `qnc-pool-tabs` `Voice`, kartica "OFF n · Tn (n takes)", prazna poruka u ugovoru).
Testovi `qnc-voice-catalog` (klip je potpun, nova snimka zamjenjuje zapis, pool ga ne vidi), `qnc-pool-tabs`,
conformance; probni 1080i50 klip: mpeg2video 4:2:2, field_order tt, 25/1. Uzivo nije provjereno. Nije izvedeno: izbor
snimke tipkama, korak 3 Draft XML.
Popravak 2026-10-30 (korisnik: "pogledaj log"; monitor nakon klika na take: "Invalid QNC source reference"): adresa klipa
snimke u katalogu nije bila u obliku koji izvor cita (`qnc-source-contract` trazi kodirani put: `_` je `%5F`, `.` je `%2E`),
pa je izvor odbio otvaranje. `qnc-voice-take` (`canonical`): u katalog ide kodirana adresa, scenarij i dalje cuva obicnu.
`qnc-content-store` `publish_voice`: zapis glasa koji je vec u katalogu s drugim oblikom adrese iste datoteke dobiva novu
(prije je pravilo "isti klip ne smije promijeniti medij" odbilo ponovnu objavu); klipovi kamera i dalje ne smiju. Testovi
`the_catalog_gets_the_clip_address_as_the_source_reference_writes_it`, `qnc-voice-catalog` (adresa se zamijenila),
conformance. Uzivo nije provjereno.
Izvedeno 2026-10-30 (korisnik: "kreni s korakom 3, Draft XML"): nacrt price iz scenarija MA Audio. Gumb "Draft XML" uz
Export u panelu scenarija (akcija kataloga `script_draft`, bez tipke) pokrece `tools/qnc-export --draft` izvan niti
sucelja (`qnc-export-launch`); izlaz je FCP7 XML `<projekt>_draft.xml` u mapi exporta projekta, isti zapis kao Export to
XML, a status daje link na datoteku. Nova kocka `crates/qnc-script-draft` (+ ugovor `contracts/modules/script-draft.module.json`;
nista ne cita ni pise): blokovi koji ulaze u trajanje priloga redom citanja postaju dijelovi jednog programa: snimljeni OFF
je klip svog zapisa glasa s izabranom snimkom na A1 (njezin kanal), OFF bez snimke je pauza njegova vremena citanja (crno i
tisina), izjava je raspon svog klipa ili dijelovi SubSegmenta (unutar bloka spojevi kao u SubSegmentu, izmedju blokova rez);
najava, odjava i biljeske se izostavljaju. `block_pieces`, `piece_inputs` i `reading_seconds` premjesteni su iz `qnc-script`
u tu kocku. Alat prije slaganja upisuje sve zapise glasa u katalog (zapis napravljen prije nego je katalog znao glas dobiva
ga sada) kroz `qnc-voice-catalog`; `qnc-voice-take::describe_record` i `qnc-voice-catalog::publish_record` primaju postavke
projekta umjesto lokatora slika, pa ih koriste i aplikacija i alat. Program slaze `qnc-program-input::pieces_loader`
(pauza smije biti i prije prvog dijela). Nije izvedeno: pauza OFF-a bez snimke na samom kraju scenarija se izostavlja;
pokrivalice (B-roll) nisu u nacrtu. Testovi `qnc-script-draft` (2), `qnc-script`, `qnc-voice-take`, `qnc-voice-catalog`,
`qnc-program-input`, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-31 (korisnik: "da, Send to Story, preskoci nesnimljene, dodaj na kraj ili u MA Video... tu bi mogao fino
podesiti kostur price, i tako posloženi poslati na Story... ako nesto zeznem u Storyju uvijek se mogu vratiti na MA Video i
poslati ponovo cijeli kostur ili samo neki njegov dio (ako je samo dio onda je rewrite odabranog dijela)"): korak 1, slanje
kostura na kraj price. Panel scenarija (MA Audio) ima uz Draft XML gumb "Send to Story" (akcija kataloga
`script_send_story`, bez tipke): blokovi koji ulaze u trajanje priloga, redom citanja, dodaju se na kraj price u jednom
koraku (jedan Undo vraca cijeli kostur): snimljeni OFF kao Off segment klipa zapisa glasa, A1 na kanalu izabrane snimke (A2
isti kanal iskljucen), dio izjave kao Ton segment s A1 dijela ili zvukom zadrzanim za klip (`qnc-clip-sound`, bez zapisa A1
kanal 1, A2 kanal 2). Nesnimljeni OFF se preskace. Zapisi glasa najprije idu u katalog (kao kod Draft XML-a).
`qnc-script-draft` `skeleton`/`SkeletonPart`; `qnc-program-db` operacija `AppendSegments` (`AppendSegment`: vrsta, klip,
IN/OUT, A1, A2, porijeklo; stopa izvora je spremljena stopa klipa; dio koji ne prode provjeru ne ostavlja nista) i stupac
`story_items.origin` (`script:<blok>:<dio>`; prici napravljenoj prije dodaje se bez brisanja), za kasnije ponovno slanje
dijela na njegovo mjesto; `qnc-script` (`send_story`, poruka nakon upisa), `qnc-script-panel`, rjecnik. Nije izvedeno
(sljedeci koraci): kostur u MA Video (fino podesavanje prije slanja), slanje dijela kostura kao prepisivanje njegovih
segmenata u prici (pokrivalice iznad njih trazi odluku). Testovi `qnc-script-draft` (novi `skeleton`), `qnc-program-db`
(novi `a_skeleton_is_appended_in_one_step_...`), `qnc-script`, conformance. Uzivo nije provjereno.
Izmjena i izvedba 2026-10-31 (korisnik: "i dalje radis gresku zbog koje smo presli na flat montazu... u MA Video to jos nisu
segmenti u smislu Storyja... to su klipovi, source-story, elementi koji imaju isti status i mogucnosti upravljanja kao source
klipovi"; "nova forma S-Story, nova grupa K (kostur)"; "S-Story poseban zapis koji je ujedno sigurnosni zapis ako korisnik
pokvari Story"; "da, otkljucavam shell, kreni s 1 i 2"): (1) Vracen commit 4089fb0 (MA Video kao Story ploca na vlastitoj
radnoj prici `story-draft.db` sa segmentima): elementi kostura nisu segmenti. MA Video je opet kakav je bio. (2) Nova forma
S-Story, grupa `k` (kostur): `apps/qnc-s-story` (`qnc-app.json`, samostalni exe `qnc-s-story`), javni desktop adapter
`crates/qnc-s-story-desktop-adapter`, ugovor aplikacije `contracts/applications/s-story.application.json`, grupa `k` u
`contracts/ui/editorial.layout.json` (zasad kopija MA Video forme, desni panel prazan), shell (otkljucan) samo Cargo ovisnost
na adapter i unos u tablicu tvornica (`apps/qnc-app`), root Cargo, conformance zna grupu `k`, katalog aplikacija osvjezen
(a, b, e, g, k, l, o). Launch bar pokazuje S-Story kad ga template projekta izabere. Nije izvedeno (koraci 3-5): vlastiti
zapis S-Storyja (elementi kao izvorni klipovi: klip, IN/OUT, A1/A2, J/L, razmak ili rez, porijeklo), kostur na jednom ravnom
timelineu (kocka SubSegmenta), Send to S-Story iz MA Audija i Send to Story iz S-Storyja. Testovi, `--check-contracts`,
conformance, release build.
Popravak 2026-10-31 (korisnik: "krivo saljes, treba slati u S-Story, a ti si poslao u Story"): gumb panela scenarija (MA Audio)
je "Send to S-Story" (`script_send_story`) i vise ne dira Story. Nova javna kocka `crates/qnc-skeleton-db` (+ ugovori modula
`skeleton-db.module.json` i baze `skeleton.database.json`; modul tablica posrednika, tablica `skeleton_elements` i pogled
`public_skeleton_elements`; conformance: na popisu javnih DB vlasnika): zapis S-Storyja odvojen od scenarija i Storyja. Element
je izvorni klip (`ton` izjava ili `off` zapis glasa) s IN/OUT, A1/A2 (kanal, razina, off), J/L A1, razmakom ili rezom prema
prethodnom i porijeklom `script:<blok>:<dio>`; `Append` dodaje na kraj u jednom koraku (neispravan element ne upisuje nista),
`Read` daje redom. `qnc-script` `send_story` pise elemente tamo (zvuk kao do sada: dio scenarija ili zvuk zadrzan za klip,
A2 OFF-a iskljucen); ovisnost o `qnc-program-db` uklonjena. Nije izvedeno: prikaz i uredivanje kostura u S-Storyju, Send to
Story iz S-Storyja. Testovi `qnc-skeleton-db` (2), `qnc-script`, `qnc-ui-text`, conformance.
Izmjena 2026-10-31 (korisnik: "S-Story ide poslije MA Videa, stavi ga K kako i jest"): MA Video je prioritetna grupa `j`
(`apps/qnc-media-assist-video/qnc-app.json`; kljuc forme u ugovoru rasporeda ostaje `l`), pa je redoslijed a, b, e, g, j
(MA Video), k (S-Story), o (Story); katalog aplikacija osvjezen. Projekt cuva slijed aplikacija iz vremena kad je napravljen,
pa novi redoslijed dobiva kad mu se template ponovno izabere u Projectu. Provjereno citanjem aktivnog projekta (primjer
`qnc-skeleton-db/examples/read.rs`, samo citanje): Send to S-Story je upisao 5 elemenata (OFF1, tri izjave, OFF2); S-Story
ih jos ne prikazuje.
Izvedeno 2026-10-31 (korisnik: "da, kreni"; "sve to vec postoji"): S-Story prikazuje i uredjuje kostur na jednom ravnom
timelineu. Uredjivanje dijelova SubSegmenta premjesteno je (ne prepisano) iz `qnc-script` u novu javnu kocku
`crates/qnc-subsegment-edit` (+ ugovor): rubovi dijela (Ctrl+I/O, desni klik, povlacenje, strelice, kotacic) sa zoomom
reza, A1 dijela, pauza i spoj prije dijela, premjestanje i brisanje; svaku izmjenu vraca kao `PieceWrite`, a pise je
vlasnik (scenarij u `qnc-script-db`, kostur u `qnc-skeleton-db`). `qnc-script` je koristi; `qnc-script-draft::inputs_of`.
`qnc-skeleton-db`: operacije `Update`, `Move`, `Remove` (`change`). Nova kocka `crates/qnc-skeleton` (+ ugovor): elementi
kostura citani iz baze projekta izvan niti sucelja, kao izvorni klipovi jedan iza drugoga, otvoreni u istoj kocki
uredjivanja; svaka izmjena odmah se upise u kostur (scenarij ni Story se ne mijenjaju). Novi panel
`crates/qnc-skeleton-panel` (+ ugovor): popis elemenata s duljinom i Open; klik na element otvara kostur na njegovom
pocetku. Desni panel grupe k je `skeleton_panel` (`qnc-side-pieces` `SKELETON_PANEL`, `qnc-right-panel`,
`qnc-panel-focus` `FocusView.skeleton`); timeline kostura stoji na mjestu source timelinea istim panelom kao SubSegment
(`qnc-subsegment-panel`), svira ga isti Broadcast Player. Conformance: grupa k mora imati skeleton_panel. Testovi
`qnc-subsegment-edit` (7), `qnc-skeleton`, `qnc-skeleton-db` (3), `qnc-script`, conformance. Uzivo nije provjereno. Nije
izvedeno: Send to Story iz S-Storyja (cijeli kostur ili dio kao rewrite; pokrivalice iznad dijela cekaju odluku).
Izmjena 2026-10-31 (korisnik: "opet nisi razumio... slaganje klipova je u desnom panelu, na isti nacin kao i u storiju, jedan
ispod drugoga"; "napravi kopiju storija u s-story"; "sve sto ima stori... prakticno dupliciraj stori"): S-Story (grupa k) je
kopija Storyja na vlastitom zapisu, bez lead-ina i lead-outa. Vraceni su 423fbf7, 4329174 (osim reda grupa: MA Video ostaje
`j`) i 67b2295: kostur kao ravni timeline na mjestu source timelinea, `qnc-skeleton`, `qnc-skeleton-panel`,
`qnc-skeleton-db`, `qnc-subsegment-edit` (uredjivanje SubSegmenta je opet u `qnc-script`). Ponovno primijenjen 4089fb0, ali za
grupu k umjesto l: raspored grupe k je raspored Storyja (tabovi All, SubClip, B-roll, Segment; Talking Head, Voice over,
B-roll | Sync/B-roll, Add SubClip, Export HI-res) uz dugme "Send to Story", desni panel `draft_segment_panel` (isti panel
Segmenti, Wrap redovi jedan ispod drugoga, markeri, slotovi, pokrivalice); zapis je prica u `story-draft.db` uz `project.db`
(posrednik baze, `ProjectDbTarget::draft_story`), bez lead-ina i lead-outa. MA Audio "Send to S-Story" dodaje kostur na kraj
te price; S-Story "Send to Story" salje svoje segmente na kraj Storyja. Grupa l (MA Video) je opet prazna. Conformance: k mora
imati `draft_segment_panel`, l `none`. Testovi `qnc-program-db`, `qnc-program-segments`, `qnc-script`, `qnc-side-pieces`,
`qnc-db-broker`, `qnc-panel-focus`, conformance. Uzivo nije provjereno. Nije izvedeno: slanje dijela price kao prepisivanje.
Nalaz i popravak poruke 2026-10-31 (korisnik: S-Story "nije spojen broadcast player"; monitor: "Program source has no
transport binding"): kopija Storyja radi; prica S-Storyja cita OFF iz mape projekta i izjave Mironik 3452/3429 s kartice
`volume-a8155186` (FX6 CFexpress), a ta kartica nije bila spojena (na D: je bio drugi disk, serijski 29C991CF). Player tada
nema vezu za izvor i cijela prica se ne otvara. `qnc-player-launcher`: poruka sada imenuje izvor koji nije spojen. Provjereno
citanjem baze (privremeni primjer, uklonjen). Kod kopije Storyja nije mijenjan.
Izvedeno 2026-10-31 (korisnik: "ovi klipovi u s-story nisu segmenti, oni su jos uvijek subklipovi... status segmenata dobit
ce tek kad ih posaljemo u Story"; "od istih elemenata u MA Audio moze se sloziti vise s-storija... tab u kojemu ce biti
verzije price"; "automatski verzija 1, 2..., uvijek nova verzija, kreni"): koraci 1 i 2 plana. (1) Zapis verzija S-Storyja:
vracen `crates/qnc-skeleton-db` (+ ugovori modula i baze) s verzijama: tablice `skeleton_versions` (broj 1, 2, ..., broj se
ne ponavlja) i `skeleton_elements` (stavka pripada verziji, redoslijed unutar verzije), pogledi `public_skeleton_versions`,
`public_skeleton_elements`; operacije `AppendVersion` (nova verzija u jednom koraku, prazna se ne pravi), `ReadVersions`,
`Read { version_id }`, `Update`, `Move` (unutar verzije), `Remove`; stavka je SubClip (klip, IN/OUT, A1/A2, J/L, pauza ili
rez, porijeklo), ne segment price; razvojna tablica bez verzija se brise. Conformance: na popisu javnih DB vlasnika. (2) MA
Audio "Send to S-Story" (`qnc-script`) svaki put pravi novu verziju; scenarij i Story se ne mijenjaju. Testovi
`qnc-skeleton-db` (3), `qnc-script`, conformance. Nije izvedeno (koraci 3-6): tab Stories u poolu S-Storyja, redovi verzije
u desnom panelu, otvaranje stavke uvecane na mjestu source timelinea, Send to Story; S-Story zasad i dalje pokazuje
kopiju Storyja (`story-draft.db`), koja se uklanja u koraku 4.
Izvedeno 2026-10-31 (korisnik: "automatski verzija 1, 2..., uvijek nova verzija, kreni"; korak 3 plana S-Storyja): pool S-Storyja
(grupa k) ima prvi tab Stories (`editorial_tab_stories`, bez tipke) s verzijama price: kartica "Version n · m elements", duljina
elemenata po stopi njihovih klipova, poster prvog klipa; tab je siv bez verzija. Klik na karticu (`skeleton_open_version`, cilj id
verzije) otvara verziju (zasad se samo pamti; desni panel s redovima verzije je korak 4). Nova kocka `crates/qnc-skeleton-versions`
(+ ugovor `contracts/modules/skeleton-versions.module.json`): verzije i njihovi elementi iz `qnc-skeleton-db` citani izvan niti
sucelja svake 2 s, kartice, otvorena verzija; nista ne pise. Drzi je `qnc-side-pieces` kad forma ima panel S-Storyja;
`FocusView::tab_cards` daje kartice SubSegment, Voice i Stories (ploca isti broj redaka). Kartica koja nije klip je jedna
neutralna `qnc_clip_row::TabCard` (premjestena iz `qnc-script` `SubsegmentCard`, koji je sada njezino ime). Testovi
`qnc-skeleton-versions`, `qnc-pool-tabs`, `qnc-side-pieces`, `qnc-panel-focus`, conformance. Uzivo nije provjereno.
Izvedeno 2026-10-31 (korak 4 plana S-Storyja; korisnik: "elementi S-Storyja imaju status SubClipa, ne segmenta; nema M markera,
slotova ni pokrivalica, samo poredak"): desni panel S-Storyja (grupa k) je `skeleton_panel`: otvorena verzija price kao Wrap
redovi jedan ispod drugoga, istim panelom kao Story (`qnc-segment-panel`), red po elementu (Ton ili Off po tome sto je), pauza
prije elementa ga pomice, bez markera, slotova i pokrivalica; val A1 i filmstrip elementa istim citacima kao SubSegment
(`qnc-program-waveform`, `qnc-program-filmstrip`, 1:1 slike na osi verzije). Bez izabrane verzije prikazuje se najnovija.
Elementi sviraju jedan iza drugoga u istom Broadcast Playeru kao SubSegment (`preview.show_pieces`); klik na red je playhead,
Space play/pauza, strelice korak. Sve s redova S-Storyja (mis i te tipke dok redovi imaju fokus) ide verziji, nikad Storyju
(`qnc-panel-focus` prije svega ostalog; `qnc-side-pieces` `act_on_rows`); ostale tipke (Tab...) rade kao do sada, a dijelovi
price ispod S-Storyja i dalje citaju praznu radnu pricu, ne Story. `qnc-skeleton-versions` (`lay_out`, `act_on_rows`,
`piece_inputs`), `qnc-right-panel` (`SKELETON_PANEL`, `Shown.versions`), raspored grupe k: tabovi Stories, All, SubClip,
B-roll, u docku samo Add SubClip; conformance: k mora imati skeleton_panel. Nije izvedeno: zoom elementa na mjestu source
timelinea (korak 5), uredivanje (gore/dolje, brisanje, pauza/rez), Send to Story (korak 6); dugmad trake panela (Undo,
Export...) na redovima S-Storyja ne rade nista. Testovi `qnc-skeleton-versions` (2), `qnc-side-pieces`, `qnc-panel-focus`
(zlatna tablica tipki), conformance, release build. Uzivo nije provjereno.
Izmjena 2026-10-31 (korisnik: "opet nepotrebno pises los kod... kopija Storyja koja ima sve u kompletu treba biti
polaziste"; "1-da" (svaka verzija je svoja kopija Storyja); "All je uvijek prvi.. Stories iza njega"): zamjenjuje korak 4
iznad (redovi elemenata kao zaseban prikaz). Svaka verzija S-Storyja je prica za sebe, kopija Storyja bez lead-ina i
lead-outa, u `story-draft-<n>.db` uz `project.db` (`qnc-db-broker` `draft_file(n)`, `DRAFT_PREFIX`,
`ProjectDbTarget::draft_story(n)`, `draft_version()`); pise je isti modul price (`qnc-program-db`), pa S-Story ima sve sto
ima Story (Wrap redovi, V/Home, markeri, slotovi, pokrivalice, play kroz OFF-ove, zoom, Undo). `qnc-skeleton-db` drzi samo
popis verzija (`skeleton_versions`, broj 1, 2, ..., ne ponavlja se; razvojna tablica elemenata se brise). MA Audio "Send to
S-Story" (`qnc-script`) pravi novu verziju i u njezinu pricu dodaje kostur kao segmente (`AppendSegments`). Pool S-Storyja:
tabovi All, Stories, SubClip, B-roll, Segment; kartice "Version n" (`qnc-skeleton-versions`), klik otvara verziju: Wrap
izlazi, panel Segmenti cita pricu te verzije (`qnc-program-segments` `switch_story`), preview je svira
(`qnc-source-preview` `set_draft_story`); bez izbora otvara se najnovija. Desni panel grupe k je `draft_segment_panel`
(panel Segmenti); `skeleton_panel` i redovi elemenata uklonjeni (`qnc-right-panel`, `qnc-panel-focus` route). "Send to
Story" salje segmente otvorene verzije na kraj Storyja. Ugovori `skeleton.database.json`, `skeleton-db`,
`skeleton-versions`. Stara jedna radna prica `story-draft.db` vise se ne cita. Testovi `qnc-db-broker`, `qnc-skeleton-db`,
`qnc-skeleton-versions`, `qnc-side-pieces`, `qnc-panel-focus` (zlatna tablica tipki), `qnc-program-db`,
`qnc-program-segments`, `qnc-script`, `qnc-source-preview`, `qnc-editorial-application`, conformance. Debug cache obrisan
(`cargo clean --profile dev`, 34,8 GiB; disk je imao 1,8 GB slobodno). Uzivo nije provjereno.
Popravak 2026-10-31 (korisnik: "desni klik misa (Ctrl+) pozicionira se na kraj wrap reda, a pravilo je da se pozicionira na
blizi marker... ovdje su subklipovi, svaki ima IN/OUT markere"): log: desni klikovi na 87, 120, 130 i 147 uzeli su isti rez na
kraju reda. Wrap red je crtao samo rezove unutar sebe bez reza na svom pocetku (v5 `local_marker_frame`: rez na granici samo
na kraju ranijeg reda), pa je svaki red mogao uzeti samo svoj kraj (OUT). Sada rez na granici pripada objema redovima: OUT
klipa ranijeg reda i IN klipa ovoga, pa desni klik uzima blizi od IN-a i OUT-a svog reda (`qnc-segment-panel` `program.rs`,
i trim playhead na pocetku reda). Vrijedi za S-Story i Story. Nije mijenjano (zakljucano pravilo 20. 10.): zakljucani
pocetni i zavrsni marker price se ne uzimaju, pa IN prvog i OUT zadnjeg klipa misem se ne uzimaju. Testovi, conformance.
Uzivo nije provjereno.
Dopuna 2026-10-31 (korisnik: "da, i prvi IN i zadnji OUT u S-Storyju... svi elementi imaju svoj in i out"): u prici bez
lead-ina i lead-outa (S-Story) zakljucani pocetni i zavrsni marker price su IN prvog i OUT zadnjeg elementa. Panel ih ne
zakljucava za mis, a desni klik (i Ctrl+M) na njih uzima rub tog segmenta kao Ctrl+Shift+I/O (klip se otvara na source
timelineu s uzetim IN-om ili OUT-om, strelice i kotacic pomicu, Enter zapisuje `TrimSegment`, Esc odustaje).
`qnc-program-view` `clip_edge_of` (zakljucani marker bez leada na tom kraju), `qnc-segment-panel` (`locked` samo kad nije
rub klipa), `qnc-program-segments` (`take_clip_edge`, `take_edge_of`). U Storyju (s leadovima) zakljucano pravilo od 20. 10.
ostaje. Test `with_no_leads_the_story_start_and_end_are_the_first_in_and_the_last_out`, conformance. Uzivo nije provjereno.
Izmjena 2026-10-31 (korisnik: "ovo nije story"; "da, tako je, kreni"; "dvoklik na jedan od elemenata s-stori wrapa treba u
source timeline otvoriti 15% prethodnog i narednog klipa a u sredini odabrani klip, kao kod pokrivalica; odabrani klip se
ponasa kao Slot"): zamjenjuje prethodni zapis (zakljucani pocetak i kraj price kao rub klipa; taj kod je uklonjen). U
S-Storyju svaki element je SubClip sa svojim IN-om i OUT-om, bez pravila Storyja (rez dvaju klipova, roll, zakljucani
markeri, slotovi). Wrap redak elementa crta samo njegov IN i OUT (pripadaju elementu po id-u, ne po slici); desni klik uzima
blizi, strelice i kotacic ga pomicu za sliku klipa, povlacenje misem takoder, trim playhead i zoom reza pokazuju nacrt (slika
prije i nova prva, nova zadnja i slika poslije), Enter zapisuje `TrimSegment` (elementi iza se pomicu za razliku), Esc
odustaje; dok je jedan rub uzet, drugi se odbija. Prvi IN i zadnji OUT uzimaju se kao i ostali. Dvoklik na element otvara ga
na mjestu source timelinea: element u sredini (70 %), po 15 % prethodnog i sljedeceg; u zoomu se uzimaju samo IN i OUT
srednjeg elementa. Nove stvari: `qnc-program-view` (`element_pins`, `element_edge`, `ELEMENT_IN`/`ELEMENT_OUT`,
`SegmentsView.element_edges`, `zoom_element`), `qnc-program-segments` (`element_trim.rs`, `Take::ElementEdge`, zoom
elementa), `qnc-segment-panel` (rubovi elementa umjesto markera), rjecnik. Story nije mijenjan, osim rezova na granici redova
iz prethodnog popravka (rez pripada objema redovima). Test `every_s_story_element_has_its_own_in_and_out`, conformance.
Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "još uvijek praviš nove kocke... moraš koristiti stare kocke za zoom i zoom u source
timeline"; "u s-story svaki klip tretiraj kao slot... off=slot, ton=slot"; "funkcije koje imamo sa slotovima treba
koristiti u kompletu"; "produljenje slota produljuje ukupnu priču, skraćenje skraćuje"): zamjenjuje dva zapisa od
31. 10. (zakljucani pocetak i kraj kao rub klipa; svaki element svoj IN i OUT). Audit: `element_trim.rs`, pinovi
elemenata (`element_pins`, `element_edge`, `zoom_element`), filtar `own_element` u panelu i S-Story grana zooma bili
su djelomicna kopija postojeceg ruba slota i segmenta (bez skoka na kljucnu sliku, bez pravila kraja klipa,
vlastite slike reza) i uklonjeni su (git revert a0786228 i 616ff280, zapisi u AGENTS.md ostaju kao povijest). Sada
u S-Storyju (prica verzije `story-draft-<n>.db`) na svakoj granici dvaju klipova stoji M marker, pa je svaki klip
jedan slot, a postojeci kod slota i markera radi s njim kao s pokrivalicom: `qnc-program-db` `clip_slots.rs`
(`ensure_clip_borders` u `finalize_story`: markeri su tocno granice klipova; u pricu verzije `MoveMarker` granice je
roll dvaju klipova (`roll_join`), `TrimSlot` mijenja IN ili OUT klipa u slotu i pricu (`trim_segment`), `MoveSlot`
zadrzava klip, prethodni svira dalje, sljedeci pocinje kasnije); `qnc-program-view` `SegmentsView.clips_are_slots`
i `clip_covers`: slike reza (`edit_stills`, `trim_stills`) uzimaju Ton klip slota kao njegovu pokrivalicu, kraj
price se mice s klipom; `qnc-program-segments`: rub slota u S-Storyju i na zadnjem klipu, razliku ne preuzima
prazni ni zadnji slot nego se pomakne sve iza (`SlotTrim.ripple`), dvoklik povecava slot klipa (70/15/15) kao slot
pokrivalice; samo slika granice u zoomu je J ili L slike klipa (`SetJoinPicture`), A1 granice postojeci J/L spoja.
Story nije mijenjan. Nije izvedeno: A2 granice u zoomu (`SetCutSoundEdges`) jos ne pise klipove u S-Storyju. Testovi `in_an_s_story_every_clip_is_a_slot_and_the_slot_edits_write_the_clip`,
`in_an_s_story_the_slot_tools_make_the_clip_and_the_story_longer_or_shorter`. Uzivo nije provjereno.
Dopuna 2026-11-01 (korisnik: "desni klik ne ide na IN markere, samo na OUT"; "na prvom klipu na IN treba dodati IN, a
na zadnjem OUT, kako smo dodali na segmentima"): u S-Storyju desni klik na rez uzima rub klipa na kliknutoj strani reza
kao Ctrl+I/O (IN klipa iza reza, OUT klipa prije), i IN prvog i OUT zadnjeg klipa (pocetak i kraj price nisu
zakljucani za desni klik u S-Storyju). Story ostaje: desni klik uzima marker cijeli. `qnc-timeline` `events.rs`
(desni klik na marker nosi kliknutu sliku), `qnc-program-commands` `TakeMarkerAt`, `qnc-program-segments`
(`clip_edge_at`, `refresh.rs`). Zlatna tablica misa: 1 redak promijenjen (vrijednost slike). Testovi, conformance.
Nalaz iz loga: monitor bljeska jer D: (serijski 29C9-91CF) javlja "Permission denied" za `XDROOT\Clip\Mironik 3584.MXF`
(ffmpeg "Invalid argument", "could not find header partition pack key"), a slike reza ponavljaju dekodiranje svakih
~100 ms. Uzivo nije provjereno.
Ispravak 2026-11-01 (korisnik: "pomjesao si markere... klik lijevog misa na lijevoj polovici klipa - playhead na IN tog
klipa, na desnoj polovici - playhead na OUT tog klipa"): u S-Storyju odlucuje kliknuta polovica klipa, ne najblizi rez.
Lijevi klik na Wrap retku stavlja playhead na IN kliknutog klipa (lijeva polovica) ili na njegov OUT, zadnju sliku (desna
polovica); desni klik (Ctrl+) po istom pravilu uzima IN ili OUT tog klipa. Story ostaje: lijevi klik je playhead na
kliknutu sliku. `qnc-program-segments` (`clip_half`, `clip_half_frame`, `clip_edge_at`, grana `Cue`). Test
`in_an_s_story_the_slot_tools_make_the_clip_and_the_story_longer_or_shorter` prosiren. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "klik na lijevi dio prvog offa playhead postavlja na OUT umjesto na IN"; "na prvi klip
postaviti marker na pocetku i na kraju zadnjeg klipa kao na Story wrap"): log: klik na lijevu polovicu dao je IN, ali dok je
IN klipa uzet bijela trim crta crtala se i na kraju klipa (kraj se mice s kracim ili duljim klipom), pa je izgledalo kao
playhead na OUT-u. Sada uzeti IN u S-Storyju ima samo jednu trim crtu, na IN-u (`qnc-program-segments` `refresh.rs`, test).
Marker na samom pocetku ili kraju reda (prvi IN, zadnji OUT) crta se 1,5 px unutar reda, pa se vidi (`qnc-timeline`
`paint_markers`; vrijedi i za Story). Testovi, conformance. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "pomicu se u svim klipovima, pogledaj crvene linije... to nije ponasanje slotova i
pokrivalica"): nacrt ruba klipa u S-Storyju pomicao je sve rezove iza uzetog ruba, a Wrap redovi ostalih klipova stoje, pa su
se rezovi crtali usred tudih klipova. Sada se u nacrtu mice samo uzeti rub: OUT klipa; uzeti IN stoji (klip klizi ispod
njega). Ostali klipovi i njihovi rezovi ostaju gdje jesu, kao kod slota pokrivalice; Enter i dalje upisuje pricu dulju ili
kracu (`qnc-program-segments` `SlotTrim::moved`, test prilagoden). Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "IN je zarobljen, ne moze se pomicati"; "napravi usporedbu story - s-story"): usporedba:
u Storyju uzeti pocetak slota stoji na mjestu jer pokrivalica klizi ispod njega (slot pokriva isti dio price); u S-Storyju je
IN rub samog klipa, pa se mora micati kao IN na source timelineu. Sada uzeti IN klipa u S-Storyju u nacrtu ide gdje ga
strelice i kotacic salju (rez i trim crta, `SlotTrim::drafted_in`); OUT kao i prije; ostali rezovi stoje; Enter upisuje.
Story nije mijenjan. Test u `in_an_s_story_the_slot_tools_make_the_clip_and_the_story_longer_or_shorter`. Uzivo nije
provjereno.
Popravak 2026-11-01 (korisnik: "zoom kocka nije original, ne pomice se audio trak, koristi originalne kocke"; play opet radi
nakon ponovno utaknute kartice): zoom uzetog ruba klipa u S-Storyju pokazivao je spremljenu pricu oko nepomicnog reza, pa
zvuk nije isao. Sada je to zoom IN/OUT-a source timelinea (isti `ZoomSound` na osi klipa): zvuk samog klipa na kanalu i
razini njegova A1, prozor oko nacrtanog ruba, pa zvuk tece ispod reza dok ga strelice i kotacic pomicu
(`qnc-program-segments` `clip_edge_zoom`, test). Story nije mijenjan. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "gdje su dugmici, u originalu su bili na sredini"): prozor zooma bio je odrezan na kraju
klipa ili price, pa je rez na OUT-u stajao na desnom rubu, a dugmad Undo | Save | Cancel (centrirana na rezu) s njim.
Sada zoom ima sekundu sa svake strane i na kraju (iza kraja je prazno), a dugmad su uvijek u sredini zooma, izmedju
dviju slika (`qnc-program-view` `zoom_window`, `qnc-trim-overlay`). Vrijedi za sve zoomove. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "pokvario si Story, usporedi sa zadnjom pushanom verzijom"): log: u Storyju desni klik na
pocetku Wrap reda uzimao je rez sa susjednim klipom (granica reda) i strelice su ga rollale. Uzrok je izmjena od 31. 10.
(fce5aa88: rez na granici reda pripada objema redovima), napravljena za S-Story, a vrijedila je i za Story. Usporedba s
pushanom verzijom (origin/libav-decoder 6674ea2c): ostale izmjene za S-Story u Storyju ne rade (`is_s_story`,
`clips_are_slots`). Sada Story opet crta i uzima rez na granici samo na kraju ranijeg reda (v5 `local_marker_frame`), a
S-Story na oba reda (`qnc-segment-panel` `program.rs`). Ostaju i u Storyju: marker na samom rubu reda crta se malo unutra,
zoom ima sekundu sa svake strane i na kraju, dugmad zooma u sredini. Testovi, conformance. Uzivo nije provjereno.
Audit 2026-11-01 (korisnik: "vrati Story iz pushane verzije"; odabrao "prvo audit"): log (tab storyboard, Story): desni
klik na slici 4 i 210 uzima isti rez dvaju klipova (join) na kraju prvog Wrap reda, jer desni klik po zakljucanom pravilu
od 20. 10. uzima najblizi marker bez obzira na udaljenost; to je isto i u pushanoj verziji. Usporedba koda Storyja s pushanom
(6674ea2c): sve izmjene S-Storyja ukljucuju se samo uz pricu verzije (`is_s_story`, `clips_are_slots`, `draft_story`);
u Storyju su razlike: izmjene od 30. 10. (jedna tablica `story_items`, A2 Tona u Off i njegovo crtanje), te danas marker na
rubu reda malo unutra, zoom sekunda sa svake strane na kraju i dugmad zooma u sredini. Popravljeno: b1dbb306 je izbacio i
pravilo od 30. 10. da se rez cije L ide u sljedeci red crta i na pocetku tog reda; vraceno (`qnc-segment-panel`
`program.rs`). Nista nije vraceno na pushanu verziju. Uzivo nije provjereno.
Vraceno 2026-11-01 (korisnik: "i dalje to nije ispravna verzija, pogledaj commit od jucer ujutro"): Story je vracen na
`39ce3c06` (31. 10. 12:30; Story kod isti kao u 51be166d od 00:10): zoom prozor odrezan na kraju klipa i price kao prije
(`qnc-program-view` `zoom_window`), dugmad zooma centrirana na rezu (`qnc-trim-overlay` iz tog commita), marker na rubu reda
bez pomaka unutra (`qnc-timeline`). Usporedba s tim commitom: preostale razlike u kockama Storyja ukljucuju se samo u
S-Storyju (`is_s_story`, `clips_are_slots`, `is_draft`, `draft_story`). Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "touchpad pomice playholder kad s dva prsta idem lijevo-desno i gore-dolje, a ne bi trebalo"):
karta touchpada `contracts/input/touchpad.json`: pomak s dva prsta okomito je `none` (prije `jog`, 18. 10.), vodoravno ostaje
`jog` kao u pushanoj verziji (korisnik: "treba pomicati, ali samo lijevo-desno"), na svim timelineima. Pinch ostaje `level`; dodir s dva prsta ostaje desni klik (Ctrl+). `qnc-touchpad` uzima pokret samo kad
njegova uloga nesto radi, pa pomak gore-dolje ide panelu (popis Wrap redova se lista kao inace). Kotacic misa (cijeli zarezi)
nije mijenjan. Test `only_a_two_finger_swipe_across_moves_the_playhead_along_is_left_to_the_panel`, conformance. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik: "u obrascu novi projekt ne vidi se gdje je prompt za tekst"; "svaki enter treba prebacivati
fokus na novi red, tab i shift+tab kao do sada"): (1) Kursor se crtao (log: egui trazi crtanje za treptanje), ali bojom
svijetlog izgleda egui-ja (tamnoplava) koju tema nije mijenjala, pa se na tamnom polju nije vidio. Sada je boje teksta teme,
u svim formama (`qnc-desktop` stil, `qnc-desktop-theme::apply_settings_look`). (2) Enter u obrascu (`form_next_field`) nikad
nije stigao: katalog tipki odbija svaku tipku dok polje drzi tipkovnicu. Nova funkcija kocke tipkovnice
`qnc-keyboard-shortcut::pressed_while_typing` pita za tipku obrasca kao da polje ne drzi tipkovnicu; ploca Projecta je
koristi (C10 isti). Enter iz imena projekta ide na prvo polje za upis, iz polja na sljedece; polja izbora se preskacu, zadnje
polje nema sljedece. Tab i Shift+Tab nisu mijenjani. Zvuk pri tipkanju nije nadjen u kodu. Test
`enter_in_a_typed_field_is_the_next_field_of_the_form`, conformance. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "u AI Media Assist dugme Transcript prebaciti u sucelje Media Poola po uzoru na Ingest";
"ostaje prikaz Media Poola, ali se ispod dodaje dugme"; "to vrijedi na ALL tabu, kada se otvori tab Transcript toga nema"):
Transcribe (`transcribe_marks`) vise nije u zaglavlju source timelinea grupe e nego u traci ispod Media poola, kao
Cancel | Select ispod preglednika u Ingestu, i samo dok je otvoren tab All; na ostalim tabovima pool zauzima cijelu visinu.
Nova javna kocka-panel `crates/qnc-action-bar-panel` (+ ugovor `contracts/modules/action-bar-panel.module.json`): traka
dugmadi koje raspored daje mjestu, zdesna, prvo jako. `qnc-board` `split_place_below`; `contracts/ui/editorial.layout.json`
grupa e `pool_actions` (tabovi na kojima se vidi, dugmad); `qnc-editorial-layout` (`PoolActions`, `POOL_ACTIONS`,
`board_layout(tab_selected)`, `action_bar`). Forma ploce isti broj redaka (C10 93): mjesto koje nema svoj panel ide traci.
Traka nije u krugu Tab tipke (dugmad source docka su). Test
`ai_media_assist_has_transcribe_under_its_pool_on_the_all_tab_only`, conformance. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "na ovoj formi tab SubClip ne bi koristio kao mjesto gdje ce se prikazivati odabrani
dijelovi tonova koji nastaju selekcijom teksta transcripta... primjereniji bi bio Talking Head, jer ce to biti polaziste za
talking head"): u AI Media Assistu (grupa e) tab poola za dijelove spremljene iz prijepisa zove se Talking Head (rjecnik
`pool.tab.talking_head`, `contracts/ui/editorial.layout.json`); radnja i sadrzaj taba isti (`editorial_tab_virtual`).
Ostale forme zadrzavaju SubClip. Testovi `qnc-ui-text`, `qnc-editorial-layout`, conformance. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "pojmovi u TV kucama: SOT za izjavu, VO za off, NAT ili AMB za atmosferu... da bi se drzali
pro pojmova trebali bi ih i koristiti"; odluka "VO, i napravi odmah i hrvatski rjecnik"): engleski rjecnik koristi SOT (prije
Ton / Talking Head), VO (prije Off / Voice over / OFF) i NAT; vrsta segmenta na Wrap retku i u tabu Segment ide kroz rjecnik
(`qnc-program-commands` `SegmentKind::label`, sifre `story.kind.sot`, `story.kind.vo`), tab AI Media Assista je SOT
(`pool.tab.sot`), pripremljeni profil scenarija `general_tv` ima oznaku i cue VO. Vrijednosti u bazi (`tonovi`/`offovi`),
sifre radnji i tipke nisu mijenjane; profil vec spremljen u projektu zadrzava OFF. Hrvatski rjecnik `contracts/text/hr/`
(cetiri dijela, sve sifre engleskog; Izjava, Off, Atmosfera, Pokrivalice); `qnc-ui-text` slaze rjecnik iz dijelova (sifra
dvaput je greska). Jezik je postavka racunala: `data/ui-language.json` `{"language": "hr"}`, bez nje engleski. Testovi
`qnc-ui-text` (svaka sifra u svakom rjecniku), `qnc-script` (oznake VO), conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-11-01 (korisnik: "u projektu treba biti opcija izbora jezika sucelja"; odluke: postavka racunala s
biracem u Projectu, primjena nakon ponovnog pokretanja; Project otkljucan samo za to): Project Advanced ima odjeljak
Interface / Sucelje s izborom "Interface language (this computer)" (`contracts/ui/project.layout.json` blok `ui_language`,
rjecnik). Nova javna kocka `crates/qnc-language-choice` (+ ugovor `contracts/modules/language-choice.module.json`): jezici za
koje QNC ima rjecnik, izabrani jezik racunala i napomena dok to nije jezik u kojem QNC radi; pise samo postavku racunala
`data/ui-language.json`, nikad projekt ni bazu. `qnc-ui-text`: `languages`, `chosen_language`, `choose_language` (jezik bez
rjecnika se odbija). Forma Projecta isti broj redaka (C10 1142): odjeljak koji nije njezin ide kocki. Test
`the_chosen_language_is_kept_for_this_computer_and_only_one_qnc_knows`, conformance. Uzivo nije provjereno. Project je
ponovno zamrznut.
Dopuna 2026-11-01 (korisnik: "prevedi i preostale natpise na hrvatski"): u hrvatskom rjecniku prevedeno je 24 natpisa koji
su ostali engleski (Izvoz, Pokrij slot, Prepisi, Pozicija, AI zvuk / AI slika, Umetak, Prve/Zadnje rijeci, REZ, ISKLJ.,
Snimi, Zaustavi, Radni naziv, Pretek, Poveznica, Uvoz medija, Izvor reprodukcije, H.264 protok, Gotova postavka). Ostaju
nazivi formata i kodeka, IN/OUT, SubClip, Template, AI, NORM, Auto/On i nazivi aplikacija. Samo `contracts/text/hr/`.
Test `qnc-ui-text`. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "u AI Media Assist prvo dugme iznad source timeline treba biti Dodaj izjavu... ta izjava se
prikazuje u tabu Izjava u media poolu"): u grupi e prvo dugme zaglavlja source timelinea zove se Add statement / Dodaj
izjavu (`story.add_statement`, `contracts/ui/editorial.layout.json`); radnja ista (`save_virtual_shot`), a spremljeno ide u
tab SOT / Izjava (`editorial_tab_virtual`). Ostale forme zadrzavaju Add SubClip. Testovi `qnc-ui-text`,
`qnc-editorial-layout`, conformance. Uzivo nije provjereno.
Dopuna 2026-11-01 (korisnik: "ne prebacuje na tab Izjava, nego ostaje na tabu Prijepis... zato sto korisnik vjerojatno
treba kreirati jos izjava"; log: izjava spremljena u 19:38:18, pool je presao na Izjava, korisnik se kliknom vratio na
Prijepis): izjava spremljena dok je otvoren tab Prijepis ostavlja pool na Prijepisu; s ostalih tabova pool i dalje prelazi
na SubClip / Izjava (`qnc-editorial-application` `short_saved`, isti broj redaka).
Dopuna 2026-11-01 (korisnik: "kada korisnik klikne na izjavu tekstualni prikaz treba oznaciti IN tekst i OUT tekst"): u
panelu prijepisa prva rijec izmedu IN i OUT prikazanog klipa dobiva crvenu zagradu "[" ispred, zadnja "]" iza (rijeci
izmedu i dalje podloga i podcrtano), a kad se IN/OUT promijeni (klik na izjavu, rez), tekst izjave se jednom dovede na
sredinu vidljivog dijela panela. Vrijedi i za IN/OUT postavljen I/O i rezom. `qnc-transcript-panel` (`marked_ends`,
`line_words`). Test `the_in_and_out_words_are_the_first_and_last_between_the_marks_across_lines`, conformance. Uzivo nije
provjereno.
Dopuna 2026-11-01 (korisnik: "da, neka play izjave stane na OUT-u"; log: izjava 556-816 svirala je do 980): play klipa
koji krene prije korisnikova OUT-a stane na OUT-u (pauza i playhead na slici OUT-a); play koji krene na ili iza OUT-a
svira dalje, a prica i SubSegment (program) se ne diraju. Vrijedi na source timelineu svake forme. `qnc-source-panel`
`stop_on_out`, `qnc-source-timeline` `sync` prima prikazani timeline (Story i Ingest isti broj redaka). Testovi dotaknutih
kocki, conformance. Uzivo nije provjereno; test same pauze treba pravi player.
Dopuna 2026-11-01 (korisnik: "za istu izjavu otvori overlay upozorenje s pitanjem vec kreirana, overwrite - cancel"; log:
ista izjava 1663-2187 spremljena cetiri puta): Dodaj izjavu (`save_virtual_shot`) za isti klip i isti IN/OUT kao vec
spremljena izjava (ne B-roll) nista ne pise, nego u sredini ekrana otvara info prozor (`qnc-choice-box`) "Izjava je vec
kreirana" s Prepisi (`statement_overwrite`: ista izjava se ponovno sprema i njezine slike rade iznova, nikad druga) i
Odustani (`statement_cancel`, zadano). `qnc-short-save` (`start` prima spremljene izjave, `question`, `answer`),
`qnc-source-timeline` (`SourceTimelineView.question`, `with_question`, prozor crta `show_with` i vraca odabranu radnju),
`qnc-editorial-application` (isti broj redaka), katalog radnji (bez tipki), rjecnik. Test
`the_same_statement_is_asked_about_and_cancel_writes_nothing`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-01 (korisnik: "obrisi visak istih izjava"): `qnc-virtual-shots` operacija `ForgetShort` /
`forget_short_now` (brise samo red klase `short`, B-roll kadar nikad; datoteke slika ostaju) i primjer
`examples/forget_shorts.rs` (aktivni projekt, kroz pisca modula). U aktivnom projektu
`2026_11_01_78fa621b...` obrisane su izjave `clip-ec4623fe-..._shot_002`, `_003`, `_004` (Mironik 3585, 1663-2187);
`_shot_001` ostaje. Usput: testni programi uvijek koriste osnovni jezik (`qnc-ui-text` `host`,
`qnc-dev-diagnostics::unit_test` javan), jer su testovi koji traze engleski tekst padali kad je racunalo izabralo hrvatski.
Testovi `qnc-virtual-shots`, `qnc-ui-text`, `qnc-dev-diagnostics`, conformance.
Izmjena 2026-11-01 (korisnik, MA Audio: "trenutna oznaka SubClip nije opravdana... to su Izjave"): u MA Audiju (grupa g) tab
poola `editorial_tab_virtual` je SOT / Izjave (`pool.tab.sot`, kao u AI Media Assistu); hrvatski naziv taba je sada
mnozina "Izjave" u obje forme. Story, S-Story i MA Video zadrzavaju SubClip. Samo raspored i rjecnik.
Izmjena 2026-11-01 (korisnik, MA Audio: "slozi dugmad po logickim cjelinama i u hrvatskoj verziji koristi hrvatske
izraze"): (1) Panel scenarija (`qnc-script-panel`): prvi red je naslov i naziv profila (skracen, nikad ispod gumba) te
izlaz scenarija zdesna Kopiraj, Izvoz, Nacrt XML, Posalji u S-Story; drugi red "U scenarij iz klipa (IN-OUT):" i po
gumb za svaku vrstu klipa profila (+ IZJAVA, + STAND-UP, + ATMOSFERA). Editor teksta: najprije gumbi vrsta teksta
(+ NAJAVA, + OFF, + ODJAVA), razmak, pa ispravljanje (Zamijeni, Ocisti); `qnc-text-source-editor` `EditorButton.group`
i razmak izmedu skupina. (2) Pripremljeni profili `general_tv`, `tv_package`, `agency_online`: kratki nazivi i cue su
sifre rjecnika (`sp.s.vo`, `sp.s.sot`, `sp.s.su`, `sp.s.standup`, `sp.s.nat`, `sp.s.lead`, `sp.c.*`): engleski VO, SOT,
SU, NAT; hrvatski OFF, IZJAVA, STAND-UP, ATMOSFERA, UVOD. `inews_mos` ostaje u izrazima iNEWS-a. (3) MA Audio: SubSegment
je "Slozena izjava", tab "Slozene izjave", Dodaj SubClip je "Dodaj izjavu". (4) Aktivni projekt
`2026_11_01_78fa621b...` ima ponovno dan profil `general_tv` (hrvatski nazivi), primjer
`qnc-script-db/examples/use_profile.rs` (isto sto "Use for project" u uredivacu profila). Testovi dotaknutih kocki,
conformance. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik: "sagledao sam siru sliku MA Audio... preimenuj grupu G u H"): MA Audio je prioritetna
grupa `h` (`apps/qnc-media-assist-audio/qnc-app.json`); kljuc forme u ugovoru rasporeda ostaje `g`, kao sto je MA Video
grupa `j` s kljucem `l`. Redoslijed: a, b, e, h (MA Audio), j (MA Video), k (S-Story), o (Story); grupa g je slobodna.
Katalog aplikacija osvjezen (`qnc-app-catalog refresh apps target\release data\application-catalog.json`). Projekt cuva
slijed aplikacija iz vremena kad je napravljen; novi dobiva kad mu se template ponovno izabere u Projectu. Conformance,
`--check-contracts`.
Izvedeno 2026-11-01 (korisnik: "kreiraj novu formu G... namjena ista kao MA Audio, ali dizajn orijentiran na pisanje priloga
i kreiranje dokumenta koji se salje na potvrdu Uredniku i Lektoru... osnova je desni panel iz MA Audio, prikazuje se preko
cijele forme; Media pool, Broadcast Player i source timeline idu u overlay prozor"): nova forma MA Script, prioritetna grupa
`g`: `apps/qnc-ma-script` (`qnc-app.json`, samostalni exe `qnc-ma-script`), javni desktop adapter
`crates/qnc-ma-script-desktop-adapter`, ugovor aplikacije `contracts/applications/ma-script.application.json`, shell samo
Cargo ovisnost na adapter i unos u tablicu tvornica (`apps/qnc-app`), root Cargo, katalog aplikacija osvjezen (a, b, e, g,
h, j, k, o). Kljuc forme MA Audio u ugovoru rasporeda je sada `h` (kao njegova grupa; `apps/qnc-media-assist-audio`,
adapter, conformance), a kljuc `g` je MA Script. Raspored grupe g (`contracts/ui/editorial.layout.json`): isti dijelovi kao
MA Audio (tabovi poola, gumbi source timelinea, `script_panel`) i `media_overlay` (natpis gumba i naslov prozora, rjecnik
`ma_script.media`, `ma_script.media_title`). Ploca grupe g je desni panel preko cijele forme ispod trake s gumbom "Media
pool i player"; gumb otvara prozor preko forme (92 % zaslona) s monitorom i transportom lijevo, tabovima poola i
karticama desno i source timelineom na dnu, istim kockama kao na ploci MA Audija; X ili isti gumb ga zatvara. Nova javna
kocka `crates/qnc-media-overlay` (+ ugovor `contracts/modules/media-overlay.module.json`): traka s gumbom i prozor;
otvorenost je izbor prikaza ovog procesa (egui memorija), nigdje se ne pise. `qnc-board`: `Layout::block`, `with_face`,
`split_place_above`. `qnc-editorial-layout`: `GroupComposition.media_overlay`, `MEDIA_OVERLAY_BAR`,
`media_overlay_layout`. `qnc-editorial-blocks` crta ista mjesta na ploci i u prozoru (C10 isti, 93). Conformance: grupa g
mora imati `script_panel` i `media_overlay`, MA Audio (h) `script_panel`. Projekt dobiva MA Script kad ga template
izabere u Projectu. Nije izvedeno: dokument za potvrdu (urednik, lektor), drukciji izgled scenarija; prozor nema tipku
(zasad samo gumb). Testovi `qnc-board`, `qnc-media-overlay`, `qnc-editorial-layout` (novi
`ma_script_has_the_script_over_the_whole_form_and_the_media_in_its_overlay`), `qnc-app`, conformance, `--check-contracts`,
release build. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik, MA Script: "dvije grupe dugmadi, jedna za kreiranje price, druga za kreiranje i upravljanje
dokumentima"; prva grupa "centrirana, plivajuca, u donjoj trecini ekrana"; "na samom vrhu naziv priloga (Radni naziv iz
baze), ukupno odobreno trajanje, trajanje tekuceg dijela i koliko je preostalo"; "na startu nema edit panela, pojavi se
iznad grupe dugmica kad korisnik klikne"; "najava se zasivi kad je pohranjena"; "za svaki naredni off povecava se broj";
"IZJAVA otvara overlay prozor"; "bolji naziv za atmosferu bi mogao biti NETON"): desni panel grupe g je
`script_writer_panel`, nova javna kocka-panel `crates/qnc-script-writer-panel` (+ ugovor
`contracts/modules/script-writer-panel.module.json`) preko cijele forme; traka s gumbom "Media pool i player" uklonjena
(`qnc-media-overlay` vise nema `bar`, prozor otvara panel kroz `set_open`; `MediaOverlay` ima samo naslov). Vrh: radni
naziv (polje `slug` obrasca priloga, `qnc-script` `ScriptView.title`), "Odobreno / Napisano / Preostalo" (preostalo
crveno kad je preko), voditelj zasebno, desno dokumenti: Na pregled (`script_export`: tekst, A/V HTML i MOS za
pregled), Kopiraj, Nacrt XML, Posalji u S-Story. Sredina: blokovi price istim crtanjem kao panel scenarija
(`qnc-script-panel::paint_block` je javan). Donja trecina: plutajuca centrirana grupa, jedno dugme po vrsti profila
scenarija projekta, redom profila (opca TV: NAJAVA, OFF, IZJAVA, STAND-UP, NETON, ODJAVA); vrsta voditelja (najava,
odjava) siva kad je vec u prici. Tekstualna vrsta otvara editor iznad grupe (naslov sljedeci broj, npr. "OFF 3"; klik na
tekstualni blok otvara ga za uredivanje), Spremi (`script_save_text`, nova radnja kataloga bez tipke u `qnc-script`:
zamijeni blok iz kojeg je tekst uzet ili dodaj blok vrste, nakon upisa editor je prazan) i Odustani. Vrsta klipa otvara
prozor medija i iznad grupe redak "Dodaj u pricu" za IN-OUT oznacen na source timelineu (`script_add_clip`). Koja je
vrsta otvorena je izbor prikaza ovog procesa (egui memorija). Hrvatski: ATMOSFERA je NETON (`sp.s.nat`, `sp.k.nat`,
`sp.f.nat`, `sp.sp.nat`). `qnc-side-pieces` `SCRIPT_WRITER_PANEL` (scenarij i transkript), `qnc-right-panel`,
conformance (g mora imati `script_writer_panel` i `media_overlay`), C10 `qnc-editorial-blocks` 93 -> 92. MA Audio
(grupa h) nije mijenjan. Nije izvedeno: dokument za urednika i lektora kao zaseban obrazac (zasad postojeci export
scenarija), tipke za grupu price. Testovi `qnc-script-writer-panel`, `qnc-side-pieces`, `qnc-editorial-layout`,
`qnc-media-overlay`, `qnc-script`, `qnc-ui-text`, conformance. Uzivo nije provjereno.
Popravak 2026-11-01 (korisnik, sa snimkom: "donja dugmad i gornje informacije ODOBRENO - TRAJANJE - PREKORACENJE trebaju
centrirani na sredini; to treba biti izdvojeni uocljivi tekst jer je bitna informacija"): dugmad grupe price bila su
uza od izracunate sirine pa grupa nije bila u sredini; sada je svako dugme jednako siroko (132 px) i grupa je tocno
centrirana. Trajanja su zaseban red na sredini vrha, veca slova: "Odobreno · Trajanje · Preostalo" (preostalo u boji
fokusa), a kad je prilog dulji od odobrenog "Prekoracenje" crveno (rjecnik `writer.approved`, `writer.length`,
`writer.left`, `writer.over` umjesto `writer.lengths`). `qnc-script-writer-panel`. Uzivo nije provjereno.
Izmjena 2026-11-01 (korisnik, prozor medija MA Scripta: "mijenja se samo izgled ne i funkcionalnost koju je nasljedio od
MA Audio"; "monitor gore, ispod media pool, ispod prikaz transcripta, ispod source timeline"): prozor medija je jedan
stupac: monitor (trecina visine), red s tabovima poola i transportom, media pool (cetvrtina visine), panel prijepisa
(ostatak) i source timeline na dnu. Panel u prozoru daje ugovor rasporeda (`media_overlay.panel`: `transcript_panel`),
isti panel prijepisa kao u AI Media Assistu. `qnc-editorial-layout` (`MediaOverlay.panel`, `media_overlay_layout`,
`panel_at`: vrsta panela i tabovi na mjestu ploce), `qnc-editorial-blocks` (desni panel i panel prozora kroz istu granu,
C10 isti 92). Conformance prolazi; test `ma_script_has_the_script_over_the_whole_form_and_the_media_in_its_overlay`
prosiren i prolazi (nakon ponovne instalacije Build Tools 2022 s MSVC v143 i Windows 11 SDK). Debug cache obrisan uz
dozvolu korisnika (`cargo clean --profile dev`, 12,3 GiB; disk je bio pun). Release `qnc-app` izgraden bez znacajke
`libav` (LLVM/libclang nije instaliran; player je zaseban program izgraden ranije s njom), pa slike zooma reza u
aplikaciji idu kroz ffmpeg naredbu. Uzivo nije provjereno. Nije
izvedeno: "Dodaj u pricu" u prozoru (redak ostaje ispod prozora).
Dopuna 2026-11-02 (korisnik: "dugme dodaj u pricu treba biti na overlay prozoru"): prozor medija MA Scripta ima iznad source
timelinea traku "Dodaj oznaceni IN-OUT u pricu kao:" s gumbom za svaku vrstu klipa profila (+ IZJAVA 2, + STAND-UP, + NETON);
vrsta otvorena svojim gumbom price je jaki gumb; bez IN/OUT-a gumbi su sivi. Dodavanje (`script_add_clip`) zatvara prozor i
vrstu, natrag na scenarij. Traka je panel prozora iz ugovora rasporeda (`media_overlay.bar`: `script_clip_bar`), crta je
`qnc-script-writer-panel` `show_clip_bar` kroz `qnc-right-panel`; vrsta `SCRIPT_CLIP_BAR` u `qnc-side-pieces`;
`qnc-editorial-layout` (`MediaOverlay.bar`, mjesto u `media_overlay_layout`, `panel_at`), rjecnik `writer.clip_bar*`. Forma
nije mijenjana (C10 isti). Redak ispod prozora na scenariju ostaje. Testovi `qnc-editorial-layout` (test prozora prosiren),
`qnc-ui-text`, conformance, release build (bez `libav`). Uzivo nije provjereno.
Otkljucano i izvedeno 2026-11-03 (korisnik: "gledam prepis... nedostaje puno teksta"; "da, otkljucaj i napravi prag prema
klipu"; otkljucani `qnc-transcribe-run` i `qnc-transcribe-jobs`): log: Mironik 3608 (13,5 min) imao je iznad praga -42 dBFS
samo 464 s od 809 s, tihi klipovi istog reda 1,3 od 20 s, 5,7 od 36 s, 7,8 od 49 s; izjava je snimljena na -46 do -55 dBFS
(izmjereno ffmpegom s kartice, samo citanje), a isti Parakeet na izrezanim rupama daje cijeli tekst. Fiksni prag zamijenjen
pragom prema klipu: sum je tihi deseti percentil desetinki sekunde bloka (`qnc-transcribe-jobs` `noise_dbfs`), prag je 6 dB
iznad najnizeg suma klipa do tog mjesta, nikad visi od -42 (glasan klip kao prije) ni nizi od -66 dBFS (`clip_floor_dbfs`,
`FLOOR_CEILING_DBFS`); red prepisa (`qnc-transcribe-run` `hear_while_read`) ga racuna dok cita i pise ga u log uz sum.
Primjer `qnc-transcribe-jobs/examples/pieces.rs` racuna isto. Izmjereno na CH1 s kartice: 3608 808 s od 809 s (prag -58,4),
3615 20,1 od 20,1 s, 3610 33,0 od 36,3 s, 3611 46,0 od 49,4 s. Prepisi napravljeni prije ostaju dok se klip ponovno ne
prepise. Test `the_floor_follows_the_noise_of_its_clip`, conformance, release build. Uzivo nije provjereno.
Dopuna 2026-11-03 (korisnik: "da, napravi preklapanje komada"): nakon praga prema klipu prepis Mironik 3608 imao je 2 rupe;
jedna (747-757 s) je govor na granici komada od 30 s bez stanke za rez, koji Parakeet na samom isjecku prepozna. Red prepisa
(`qnc-transcribe-run` `hear_while_read`) svaki komad daje motoru 2 s ranije (`OVERLAP_SECONDS`; i preko granice blokova
citanja), a od rijeci tog komada zapisuje samo one koje pocinju na ili iza kraja zadnje zapisane rijeci (`kept_until`,
`after_kept`; jedna slika tolerancije vremena motora; redak presjecen u dva pise se ponovno iz preostalih rijeci). Prvi komad
i trim zoom nisu mijenjani. Test `a_piece_heard_again_from_earlier_writes_only_the_words_the_cut_took`, conformance, release
build. Uzivo nije provjereno.
Popravak 2026-11-03 (log probe: prepis 3608 popunio je 750 s, 1797 rijeci, ostala je jedna rupa oko 539 s gdje motor ni na
samom isjecku ne prepozna govor; na granici komada oko 168 s rijec "i" upisana je dvaput, motor ju je u preklopu stavio 5
slika kasnije): prva nova rijec komada ne upisuje se kad je ista kao zadnja upisana (bez velikih slova i interpunkcije) i
pocinje unutar pola sekunde iza nje (`qnc-transcribe-run` `ECHO_FRAMES`, `same_word`, `kept_until` daje i tekst zadnje
rijeci); ponavljanje unutar jednog komada ("ne, ne,") ostaje. Test `the_last_word_heard_again_a_little_later_is_not_written_twice`,
conformance, release build. Uzivo nije provjereno.
Dopuna 2026-11-03 (korisnik: "najdulji klip, nedostaje pocetak"; "da, napravi normalizaciju za motor"): Mironik 3608 imao je
prvu rijec na 8,6 s; pocetak intervjua (-47 do -56 dBFS) motor je dobio, ali Parakeet u njemu nije cuo rijeci, a isti zvuk
pojacan za 20 dB dao je "Ja sam Stanko Karaman..." (whisper i pitanje "Molim vas, predstavite se"). Motor sad dobiva svoju
kopiju zvuka na razini govora: glasni deseti dio sekunde (90. percentil RMS) dignut na -20 dBFS, nikad stisan, najvise
+30 dB, iznad 0,8 vrh se meko savija prema punoj skali (`qnc-transcribe-jobs` `at_speech_level`); vrijedi za red prepisa i
zoom (`qnc-transcribe-run` `Job::transcribe`, jedino mjesto gdje se pise WAV za motor) i primjer `transcribe_wav`. Ton price,
playera i exporta nije diran. Izmjereno (Parakeet, CH1 3608): pocetak daje "Molim vas predstavite se. Ja sam stanko karaman,
inace fotorekorter novinar, cetrdeset dva godine...", rupa oko 539 s daje "I onda je to doslo do onda je buklo i tako.";
isjecak 9-24 s isti tekst kao prije. Test `quiet_speech_reaches_the_engine_at_the_level_of_speech`, conformance, release
build. Uzivo kroz aplikaciju nije provjereno.
Izmjena 2026-11-03 (korisnik, MA Script: "klik na najavu otvara tekst box, treba ukljuciti prompt u tekst, isto i za ostale
akcije"; "overlay prozor: oko monitora puno izgubljenog prostora, posteri nepregledni, tekst nema dovoljno prostora"; izabrao
raspored "Monitor i posteri gore"): (1) Kad se editor price otvori za vrstu (NAJAVA, OFF...) ili za tekstualni blok, odmah
preuzme tipkovnicu, jednom, pa klik drugdje zadrzava svoj fokus (`qnc-text-source-editor` `focus`, `qnc-script-writer-panel`
pamti za koju vrstu i blok je editor otvoren). (2) Prozor medija: gore monitor tocno 16:9 i uz njega tabovi poola,
transport i posteri; ispod panel prijepisa cijelom sirinom; dolje traka "Dodaj u pricu" i source timeline
(`qnc-editorial-layout` `media_overlay_layout`, `OVERLAY_TOP_SHARE` 0,42). `qnc-board`: nova velicina `Size::Across { ratio }`
(dio vodoravne podjele sirok koliko visina trazi za omjer slike). Testovi `qnc-board` (novi
`a_picture_across_a_horizontal_split_is_as_wide_as_its_height_asks`), `qnc-editorial-layout`, conformance, release build.
Uzivo nije provjereno.
Popravak 2026-11-03 (korisnik, MA Script: "trebamo slozeni timeline koji prikazuje slozeni subsegment kako bi se mogao fino
slagati"; log: Otvori slozenu izjavu pokrenuo je player na SubSegmentu, a prozor medija ostao je zatvoren): slozeni timeline
(`qnc-subsegment-panel`, rubovi dijelova, zoom reza, A1 dijela, pauze i spojevi) stoji na mjestu source timelinea, a u MA Scriptu
je source timeline samo u prozoru medija. Gumb Otvori na bloku price (klip ili slozena izjava) sada otvara i prozor medija
(`qnc-script-writer-panel`). Conformance, release build. Uzivo nije provjereno.
Popravak i dopuna 2026-11-03 (korisnik: "nevidi se cijeli owerlay... ujedno klipovi bi trebali biti slozeni kao sto radimo s
pokrivalicama"; snimka zaslona: prozor medija izlazio je desno i dolje preko ruba ekrana): (1) `qnc-media-overlay`: prozor je
uvijek u sredini ekrana (`anchor`, ne pomice se, ne pamti stari polozaj), a ono sto se u njemu crta dobiva tocno njegovo tijelo
(dijete s tim pravokutnikom i odrezom), pa sadrzaj vise ne siri prozor preko ekrana. (2) Slozena izjava: svaki dio je na
slozenom timelineu traka kao pokrivalica, u boji svoje duljine (iste granice `cover_lengths`) s duljinom `sekunde:slike`,
uvijek vidljiva; dio ciji je rub ili A1 uzet je izabran. Klik na traku stavlja playhead na IN dijela. `qnc-timeline`
`TimelineLayerFlags.covers_shown` / `with_covers_shown` (trake pokrivalica preko filmstripa uvijek vidljive; drugdje kao prije),
`qnc-subsegment-panel` (prima `cover_lengths`, ovisi o `qnc-segment-panel` i `qnc-program-view`), `qnc-script`
`SubsegmentView.rate` (stopa klipa prvog dijela). Testovi `qnc-timeline` (zlatna tablica misa), `qnc-subsegment-panel`,
`qnc-media-overlay`, conformance, release build. Uzivo nije provjereno.
Popravak 2026-11-03 (log: play slozene izjave nije radio jer kartica D: javlja "fatal device hardware error" na Mironik
3608.MXF; korisnik: "da, popravi oboje"): (1) `qnc-trim-stills`: slika zooma reza koja ne uspije stavlja svoj klip na 5 s
odmora (`REST`, `resting`): za to vrijeme nista od tog klipa se ne dekodira, zatim se njegove neuspjele slike pokusaju
ponovno (prije: pokretanje ffmpega na pokvarenu karticu svakih ~100 ms; nakon ponovno utaknute kartice slike se vrate same).
(2) `tools/qnc-player-runner`: poruke klijentu na stdout idu kroz `say` (writeln + flush), pa klijent koji je zatvorio
kanal daje gresku i proces zavrsava uredno umjesto panica "failed printing to stdout: The pipe is being closed". Player i
`qnc-app` izgradeni s `libav` (bindgen dobiva MSVC i Windows SDK include kroz `BINDGEN_EXTRA_CLANG_ARGS`). Testovi
`qnc-trim-stills`, conformance. Uzivo nije provjereno.
Dopuna 2026-11-03 (korisnik: "uredivanje subsegmenta treba imati sve mogucnosti kao kod pokrivalica i slota"; log: play
radi, rad samo klikovima, uredivanje se nije zapisivalo): slozena izjava se uredjuje po pravilu misa pokrivalica i slota.
(1) Desni klik na spoju dvaju dijelova bez pauze uzima cijeli rez (kao M marker): OUT dijela prije i IN dijela poslije
pomicu se zajedno (roll, duljina izjave ostaje), svaki unutar svog klipa; strelice, kotacic i povlacenje bilo kojeg od dva
ruba ga pomicu, zoom pokazuje zadnju sliku prije i prvu poslije reza, Enter ili lijevi klik sprema oba dijela. Rubovi uz
pauzu, prvi IN i zadnji OUT uzimaju se sami; samo jedna strana reza u zoomu (klik na lijevu/desnu sliku) ili Ctrl+I/O.
(2) Lijevi klik dok je rub, rez ili A1 dijela uzet sprema kao Enter (prije je samo pomicao playhead). (3) Svaka radnja
slozene izjave pise se u player log (`subsegment-key`). `qnc-script` (`Trim.roll`, `roll_to`, `write_trim`,
`SubsegmentView.roll`), `qnc-subsegment-panel` (`nearest_edge` daje `in`/`out`/`cut`, test). Testovi, conformance, release
build s libav. Uzivo nije provjereno.
Dopuna 2026-11-03 (korisnik: "klik na duljinu trajanja ne selektira slot"; log: roll reza radi, klik na traku samo je stavljao
playhead na IN dijela): klik na traku s duljinom bira dio kao slot (nova radnja kataloga `subsegment_select`, bez tipke):
traka je istaknuta, playhead na IN dijela; Ctrl+I/O i Ctrl+1 tada rade na izabranom dijelu (inace na dijelu pod
playheadom), Delete ga izbacuje iz izjave (zadnji dio ne), Esc ga pusta, klik izvan trake pusta izbor i samo stavlja
playhead. `qnc-script` (`SUBSEGMENT_SELECT`, `chosen`, `part_in_hand`, `SubsegmentView.chosen`), `qnc-subsegment-panel`.
Testovi, conformance, release build s libav. Uzivo nije provjereno.
Dopuna 2026-11-04 (korisnik: "provjeri radi li L i J ton" -> u slozenoj izjavi ih nije bilo; "da, dodaj J i L"; "desni klik
na ton u zumu trebao bi raditi isto"; "to je isto postojeca kocka"): spoj dvaju dijelova slozene izjave ima J i L tona A1
kao rez u prici, istim kockama: spajanje tona `qnc-program-playlist` `cross_a1` (kroz `a1_lead_frames`/`a1_tail_frames`
segmenata programa slozene izjave; J ili L zamjenjuje crossfade spoja; uz pauzu nista), crtanje strana J/L na timelineu
`qnc-timeline` (`TimelineMarkerPin.a1_lead/a1_tail`, uzeta strana kao `TimelineMarkerPart::Sound/SoundLead/SoundTail`),
zoom reza `qnc-trim-overlay` (A1 gumb, strane, povlacenje). Zapis: `qnc-script-db` `Piece.a1_lead_frames` (J zvuka dijela
nakon spoja) i `a1_tail_frames` (L zvuka dijela prije), pripadaju spoju kao pauza (ostaju uz pomak ruba i premjestanje
dijelova, prvi dio ih nema), operacija `SetJoinSound`, test. Uredivanje: s uzetim cijelim rezom Ctrl+1, klik na A1 u zoomu
ili desni klik na ton A1 u zoomu uzima ton spoja (obje strane); Ctrl+I samo J, Ctrl+O samo L, Ctrl+1 opet obje; strelice,
kotacic, klik i povlacenje strane u zoomu pomicu; svaki korak odmah se cuje (sekundu prije reza); Enter ili lijevi klik
sprema, Esc odustaje (`qnc-script` `JoinSound`, `JoinSide`, `join_key`, `set_value`; `qnc-side-pieces` predaje vrijednost
radnje). `qnc-trim-overlay`: desni klik na ton trake A1/A2 u svakom zoomu reza dok je rez uzet radi isto sto gumb A1/A2
(Ctrl+1/Ctrl+2), i u Storyju. `qnc-program-input` `PieceInput.sound_lead/sound_tail`, `qnc-script-draft`. Testovi
`qnc-script` (pomak tona spoja), `qnc-script-db` (J/L spoja), conformance, release build s libav. Uzivo nije provjereno.
Dopuna 2026-11-04 (korisnik: "u skriptu transcribirani tekst treba imati vremenske oznake kako bi se moglo montirati po
tekstu"; "svaka rijec, nedovoljno je svaka recenica"): izjava u scenariju (panel scenarija MA Audio i MA Script) pokazuje
rijeci svog dijela iz prijepisa klipa u bazi projekta, svaka rijec s izvornim timecodeom iznad sebe, nova recenica s
razmakom. Klik na rijec otvara izjavu na mjestu source timelinea (kao slozenu izjavu, i izjavu od jednog raspona; u MA
Scriptu i prozor medija) s playheadom na rijeci; dvoklik na dvije rijeci istog dijela postavlja IN dijela na pocetak ranije i
OUT na kraj kasnije rijeci (rez po tekstu, upis kroz `SetPiece`; izjava od jednog raspona postaje svoj prvi dio). Prijepisi
se citaju uz scenarij izvan niti sucelja (`qnc-script` `read.rs`). Nove radnje kataloga bez tipki `script_word_cue`,
`script_word_cut`. `qnc-script` (`TimedWord`, `timed_words`, `BlockRow.words`, `WORD_CUE`, `WORD_CUT`, `open_as_subsegment`),
`qnc-script-db` (`set_piece` za izjavu od jednog raspona), `qnc-script-panel`, `qnc-script-writer-panel`, rjecnik. Test
`a_statement_shows_the_words_of_its_range_with_their_timecodes`, conformance, release build s libav. Uzivo nije provjereno.
Nije izvedeno (ceka plan): fino prepisivanje odabranog dijela po slovima.
Dopuna 2026-11-04 (korisnik: "nisu implementirane sve funkcije za slot, pogledaj pokrivalice kocke"): dio slozene izjave
dobiva funkcije slota iz pokrivalica. (1) Ctrl+S (izabrani dio, inace dio pod playheadom) ili desni klik / Ctrl+klik na
traku s duljinom uzima dio cijeli: strelice i kotacic ga pomicu po izjavi kao slot s oba markera (dio zadrzava svoju sliku i
ton; dio prije svira dulje ili kraće, odnosno pauza prije raste ili se smanjuje; dio poslije pocinje kasnije ili ranije u
klipu, odnosno pauza poslije se smanjuje ili raste; duljina izjave ostaje), bijele crte na oba ruba, odmah se cuje, Enter ili
lijevi klik sprema (`SetPiece` / `SetJoin` susjeda), Esc odustaje. Prvi i zadnji dio se ne pomicu cijeli (kao slot uz pocetni
i zavrsni marker). (2) Desni klik na A1 uzima ton dijela na kliknutom mjestu (kao Ctrl+1). Nova radnja kataloga
`subsegment_take` (bez tipke), rjecnik `subsegment.slide_locked`. `qnc-script` (`slide`, `slide_key`, `slide_pieces`,
`slide_range`, `SubsegmentView.slide`), `qnc-subsegment-panel`. Test `a_part_slides_as_a_slot_...`, conformance, release
build s libav. Uzivo nije provjereno. Nije izvedeno: dvoklik za uvecani prikaz dijela, Undo za slozenu izjavu (baza
scenarija nema korake undoa).
Dopuna 2026-11-04 (korisnik: "J i L se mogu raditi na nivou slota tona... dva susjedna slota na istom video rezu... izmedju
njih rupa, da se uskladi ritam izgovora kad se slazu slozene izjave"; "prvi J drugi L"; "ne treba biti tisina nego nivo 0"):
J i L spoja slozene izjave mogu biti i negativni: na istom rezu slike ton dijela prije moze zavrsiti prije reza (L < 0), a ton
dijela poslije poceti poslije njega (J < 0); izmedju njih je rupa u kojoj ton ostaje na mjestu, na razini 0 (ruta A1 ostaje,
`fade_out`/`fade_in` s prozorom od jedne slike; slika nedirnuta). Pozitivni J/L krizaju se kao prije. `qnc-program-playlist`
`hole_a1` (test `a_hole_at_a_cut_puts_both_sounds_at_level_zero_and_keeps_the_pictures`), `qnc-program-input` (negativne
strane u rupe), `qnc-script-db` (`a1_lead_frames`/`a1_tail_frames` i `SetJoinSound` s predznakom), `qnc-script`
(`JoinSound` s predznakom: obje strane zajedno pomicu rez tona ili preklapanje/rupu cijelu, jedna strana sama, svaki ton
ostaje unutar svog dijela), `qnc-subsegment-panel` i zoom crtaju stranu s obje strane reza. Testovi, conformance, release
build (app, player, prerender s libav, export). Uzivo nije provjereno.
Izmjena 2026-11-04 (korisnik: "u zumu klik na sliku treba biti selektiranje slota, a kad je slot selektiran desni klik na
ton selektira zonski marker"; "treba biti selekcija slota"; log: klik na sliku uzimao je rub `subsegment_take_edge`):
zamjenjuje pravilo od 29. 10. (lijeva slika OUT, desna IN). Klik na sliku u zoomu slozene izjave bira slot (dio) kojem slika
pripada (`subsegment_select`; dok je rub ili ton uzet, playhead se ne mice i uzeto ostaje). S izabranim slotom desni klik na
ton A1 u zoomu (ili gumb A1, Ctrl+1) uzima ton tog slota na rezu: dio prije njegov L (kraj tona), dio poslije njegov J
(pocetak tona); bez izbora obje strane. Gumbi tona u zoomu postoje za svaki rub uz spoj (OUT prije, IN poslije, cijeli
rez), ne samo za cijeli rez. Rub se i dalje uzima desnim klikom na timelineu i Ctrl+I/O. `qnc-script` (`subsegment_zoom`
klikovi, `join_key` `side_of`, zoom `taken_cut`), test zooma promijenjen. Testovi, conformance, release build. Uzivo nije
provjereno.
Popravak 2026-11-04 (korisnik: "klik na sliku ne prebacuje indikaciju fokusa (crveni obrub)"): u zoomu slozene izjave
crveni okvir (`EditStill.cut`) stoji na slici izabranog slota cim je slot izabran (`qnc-script` `follow_subsegment`). Audit
zoom kocke `qnc-trim-overlay` (pasivna, puni je vlasnik prikaza: `qnc-program-segments` za Wrap redove, rub segmenta i
source IN/OUT, `qnc-script` za slozenu izjavu): klik na sliku postoji samo u slozenoj izjavi, okvir fokusa, gumbi V/A1/A2 i
`taken_cut` puni svaki vlasnik na svoj nacin, Undo u zoomu ne radi za slozenu izjavu, rez dvoklikom na rijec samo u source
zoomu. Plan ujednacavanja ceka korisnika.
Izvedeno 2026-11-04 (korisnik: "da, kreni s 1-3"; zoom univerzalan za sve opcije montaze): (1) Fokus odlucuje kocka:
`qnc-program-view` `ZoomSound.focus_side` (`false` strana prije reza, `true` poslije); `qnc-trim-overlay` crveno uokviruje
slike strane u fokusu (bez nje vlastiti okvir vlasnika `EditStill.cut`). (2) Klik na sliku u svakom zoomu s uzetim rezom
salje jednu univerzalnu radnju `zoom_pick_side` (cilj rez, vrijednost 0/1), osim kad vlasnik slici da vlastitu radnju (slot
slozene izjave); novi id u katalogu tipki. (3) Story i S-Story (`qnc-program-segments`): `SegmentCommand::ZoomSide`
(`qnc-program-commands`), stanje `zoom_side` (novi uzeti rez ga brise); s uzetim tonom reza klik na sliku pomice samo ton te
strane (L omota prije, J omota poslije), inace sljedeci desni klik na ton (ili gumb A1/A2, Ctrl+1/Ctrl+2) uzima samo ton te
strane; fokus u zoomu je strana tona koja se mice, inace kliknuta strana. Slozena izjava (`qnc-script`) puni `focus_side`
iz strane tona spoja ili izabranog slota (zamjenjuje okvir iz prethodnog popravka). Test
`a_picture_clicked_in_the_zoom_is_the_side_whose_sound_is_taken`, testovi dotaknutih kocki, conformance, release build s
libav. Uzivo nije provjereno. Nije izvedeno: zoom source IN/OUT nema uzeti rez (klik na sliku tamo ne radi nista), Undo za
slozenu izjavu, dvoklik na rijec izvan source zooma.
Popravak 2026-11-04 (korisnik: "ne radi slot... svaki klip kad se odabere slot moze slot produljivati ili skracivati a da to ne utjece na drugi slot"; "sve smo to vec imali u pokrivalicama"; log: nakon `subsegment_select 0` desni klik na spoj uzeo je cijeli rez `0:cut`, pa je roll mijenjao i susjedni dio): u slozenoj izjavi s izabranim slotom desni klik na rez uzima rub tog slota (OUT kad je izabran dio prije reza, IN kad je izabran dio poslije), ne roll; strelice, kotacic i povlacenje mijenjaju samo duljinu tog dijela, ostali dijelovi zadrzavaju svoju i pomicu se iza njega, kao rub slota pokrivalice. Bez izabranog slota desni klik na spoj i dalje uzima cijeli rez. Zoom nije mijenjan. `qnc-script` (`TAKE_EDGE`). Testovi `qnc-script`, release build s libav. Uzivo nije provjereno.
Popravak 2026-11-04 (korisnik: "prikaz dva susjedna slota nije ispravan... J i L ne rade samostalno... samo selektirana strana se mice, to ne smije utjecati na susjedni slot"; "mozda trebamo neovisno odredivanje IN/OUT tona + L i J"): (1) Dok je uzet vlastiti rub dijela slozene izjave (bez rolla), na timelineu se mice samo taj rub; ostali dijelovi stoje gdje su spremljeni do Entera (kao rub slota pokrivalice), a player i zoom i dalje sviraju nacrt (playhead se preslikava na prikaz). (2) S izabranim slotom ton se odreduje neovisno o susjedu: Ctrl+I je J izabranog slota (pocetak njegova tona, spoj prije njega), Ctrl+O njegov L (kraj tona, spoj poslije); prelazak na drugi spoj najprije spremi pomaknuti; klik na stranu u zoomu uzima stranu izabranog slota; slot bez spoja na toj strani javlja `subsegment.no_join_sound`. `qnc-script` (`follow_subsegment`, `join_key`), rjecnik. Testovi `qnc-script`, `qnc-ui-text`, conformance, release build s libav. Uzivo nije provjereno.
Dopuna 2026-11-04 (korisnik: "ne postoji povratna veza izmedu slota i teksta; kada se u timelineu slozene izjave klikne na slot, trebao bi se selektirati tekst"): u panelu scenarija (MA Audio, MA Script) rijeci dijela izabranog na timelineu otvorene slozene izjave su oznacene (podloga i podcrtano) i tekst se jednom dovede na sredinu pogleda; rijec ispod playheada je osvijetljena dok se pusta ili korakom mice. `qnc-script-panel` (`paint_block`; cita `ScriptView.subsegment`). Nije izvedeno (ceka plan): brisanje rijeci u tekstu koje bi preslozilo slot na timelineu (trazi fina vremena rijeci, a za slova vremena koja motor prijepisa ne daje). Conformance, release build s libav. Uzivo nije provjereno.
Dopuna 2026-11-04 (korisnik, MA Script, snimka prozora medija: "nema teksta u overlayu gdje se slazu slozene izjave... tu treba biti interaktivan"): dok je izjava scenarija otvorena kao slozena izjava, mjesto prijepisa u prozoru medija pokazuje tekst te izjave istim blokom kao scenarij (`qnc-script-panel` `show_open_statement` -> `paint_block`): dijelovi s rijecima i njihovim timecodeovima, rijeci izabranog slota oznacene, rijec pod playheadom osvijetljena, klik na rijec playhead, dvoklik na dvije rijeci IN/OUT dijela. Bez otvorene izjave ostaje prijepis prikazanog klipa. `qnc-right-panel` (`TRANSCRIPT_PANEL`). Conformance, release build s libav. Uzivo nije provjereno.
Popravak i dopuna 2026-11-04 (korisnik: "jedno rjesavas, a drugo nestaje... funkcije IN/OUT u svakom timelineu moraju biti u funkciji"; log: I i O na otvorenoj slozenoj izjavi isli su source timelineu, koji tada nema klip, pa nisu radili nista; klik na rijec dok je rub uzet samo je pomicao playhead; zatim "dodaj u zoomu da klik na tekst pozicionira wave tocno na mjesto playheada"): (1) I i O na timelineu slozene izjave daju dijelu pod playheadom IN ili OUT na playheadu (OUT iza prikazane slike), odmah upisano; kad je rub tog dijela uzet, rub ide tamo kao nacrt (Enter sprema); playhead u pauzi javlja `subsegment.mark_in_pause`. (2) Klik na rijec u scenariju ili u tekstu prozora medija dok je rub dijela uzet pomice IN na pocetak, OUT na kraj rijeci (nacrt). (3) Zoom reza: jedan klik na rijec (prije dvoklik) pomice uzeti rub na rijec i prozor se centrira na novi rub; u source zoomu kao prije kroz `transcript_cut_word`, u slozenoj izjavi nova radnja `subsegment_zoom_word` (unutar uzetog dijela). `qnc-script` (`mark_part`, `zoom_word`, `WORD_CUE`), `qnc-script-panel` (cilj rijeci nosi i kraj), `qnc-trim-overlay`, `qnc-subsegment-panel`, katalog, rjecnik. Nije izvedeno: klik na rijec u zoomu Wrap redova (Story) jos nista ne radi. Testovi, conformance, release build s libav. Uzivo nije provjereno.
Dopuna 2026-11-04 (korisnik: "dodaj klik na rijec i u zoomu Storyja"): u zoomu Wrap redova (Story, S-Story) klik na rijec stavlja uzeti rez (M marker, rub slota, strana J/L) na blizu granicu rijeci (pocetak ili kraj), istim koracima kao strelice, pa vrijede sva pravila uzetog reza; Enter sprema, Esc odustaje. `qnc-trim-overlay` (rijec u svakom zoomu, `at` segments), `qnc-panel-focus` (`route`, `transcript_cut_word` na segments, najvise 500 koraka). Testovi, conformance, release build s libav. Uzivo nije provjereno.
Audit 2026-11-04 (korisnik: "napravi dubinski audit svega vezanog za timeline, prepis i sve alate i napravi objedinjeni model"): `docs/97-timeline-unified-model.md`: pet prikaza (source, Wrap/program/uvecani slot, slozena izjava, zoom reza, rijeci), 12 razlika koje krse pravilo 0.12 (strana reza tri znacenja, trake u zoomu bez kanala/razine, zoom salje panel segments za svakog vlasnika, J/L tri hvatanja i dva modela, rub pet izvedbi, SubSegment mimo mape misa, rijeci sest putova), model (objekti na osi, jedan skup radnji, jedna mapa gesta za timeline i zoom, vlasnik daje samo os, pravila objekta i upis) i plan u sest koraka. Kod nije mijenjan.
Izvedeno 2026-11-04 (korisnik: "kreni s korakom 1, ali provjeri prije hoce li to biti monolit... trebao bi biti u kockama; dodaj i opcije pokrivanja reza... predvidi ih; provjeri koje alate koriste suvremene aplikacije za montazu, a mi ih nemamo"): korak 1 plana `docs/97` bez centralnog dispatchera (bio bi monolit, odjeljak 0.11): nova javna kocka `crates/qnc-edit-verbs` (+ ugovor `contracts/modules/edit-verbs.module.json`): rjecnik radnji timelinea i zooma (Cue, Keep, Drop, Undo, TakePicture, TakeSound A1/A2, PickSide, TakeSoundSide, DragSoundSide, Word) <-> sistemske radnje kataloga, tamo i natrag, na panelu vlasnika; bez stanja, pravila vlasnika i crtanja. `qnc-trim-overlay` svaki dogadaj gradi kroz nju na panelu svog vlasnika (prije hardkodirani `segments` za V, A1/A2, desni klik na ton i J/L). Ponasanje isto. `docs/97` dopunjen: podjela modela u kocke (3.5; predvidene `qnc-edit-cut`, `qnc-edit-edge`, `qnc-edit-lane`, `qnc-edit-words`), prekrivanje reza (3.6; predvidene `qnc-cut-dissolve`, `qnc-cut-punch-in`, `qnc-cut-smooth` + `qnc-frame-interp`, nijedna nije napisana) i alati drugih programa koje QNC nema (3.7; JKL shuttle, trim mode, slip, lift/extract, match frame, snapping, brzina, keyframeovi razine i ducking, ciscenje govora, titlovi i potpisi, zamagljivanje lica, reframe 9:16 i drugi). Testovi `qnc-edit-verbs` (svaka radnja tamo i natrag), `qnc-trim-overlay`, `qnc-panel-focus`, `qnc-program-segments`, conformance, release build s libav.
Dopuna 2026-11-04 (korisnik: "dovrsi korak 1"): zlatna tablica pravila 0.12 u `qnc-timeline` (`events.rs`, test `the_timeline_and_its_zoom_send_the_same_verbs`): za geste koje timeline i zoom dijele (klik, klik dok je nesto uzeto, slika i A2 uzetog reza, klik i povlacenje J/L linije) timeline kroz mapu misa salje tocno radnju koju zoom gradi kroz `qnc-edit-verbs`, na panelu vlasnika; `Verb::of` cita `segments_keep_edit` i `source_keep_mark` kao Keep. Promjena retka trazi odluku korisnika. Kod ponasanja nije mijenjan. Preostale razlike zapisane za korak 3 (`docs/97`). Testovi `qnc-timeline` (27), `qnc-edit-verbs`, conformance.
Izvedeno 2026-11-04 (korisnik: "da, kreni s korakom 2", docs/97): timeline slozene izjave na zajednickoj mapi misa. `qnc-subsegment-panel` vise ne prevodi geste rukom: salje `qnc_timeline::event` i `take_marker_drag_event` za panel reda programa (`segments`, isto sto Wrap red), samo radnje s popisa `qnc_script::ROW_ACTIONS` (srednji klik M marker, Wrap zoom i Edit segmenta tu ne idu nikamo); otpustanje povlacenja i dalje sprema. Vlasnik osi `qnc-script` (novi `pointer.rs`, `own_action`) prevodi te radnje u svoje: klik playhead ili Keep, klik na traku bira dio, desni klik ili Ctrl+klik na traku uzima dio, desni klik na A1 ton dijela (samo s frameom pokazivaca; Ctrl+1 ostaje dio u ruci), desni klik kod ruba najblizi rub ili rez, povlacenje uzetog ruba, J/L klik i povlacenje (`join_key`), kotacic i Ctrl+kotacic. `nearest_edge` i `drag_target` premjesteni iz panela u `qnc-script` (testovi s njima). Ponasanje isto. Testovi `qnc-script` (15, novi `the_mouse_maps_row_actions_are_the_subsegments_own`), conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-04 (korisnik: "da, kreni s korakom 3", docs/97): trake A1/A2 u zoomu iste kao na timelineu. `qnc-timeline`: javni alati trake `LaneLook`, `paint_lane_button_of`, `paint_lane_heard`, `lane_channel_column` (timeline ih i sam koristi). `qnc-trim-overlay`: oznaka A1/A2 je uvijek gumb (crven kad je off, zuti rub dok je uzeta), uz nju razina i, dok je traka uzeta, stupac kanala; klik na oznaku s uzetim rezom uzima ton reza, bez reza traku kao oznaka timelinea (na redu programa ton pod playheadom), klik na kanal bira kanal. `qnc-edit-verbs`: glagoli `TakeLane` i `PickChannel`; zlatna tablica (`qnc-timeline` events) prosirena za oznaku trake i kanal. Izbor traka u zoomu: source (`SourceTimeline::lanes_shown` -> `SourcePick::with_lane_choices` -> `ZoomSound.lane_choices`, `qnc-program-view` `ZoomLane`), prica (uzeti `part_sound`), slozena izjava (A1 uzetog dijela; `qnc-script` `sound_key` prima klik na kanal). Testovi, conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-04 (korisnik: "da, kreni s korakom 4", docs/97): jedno pravilo tona reza. Nova javna cista kocka `crates/qnc-edit-cut` (+ ugovor `contracts/modules/edit-cut.module.json`): `SoundSide` (Both, Lead = J komada poslije reza, Tail = L komada prije), `CutSound` (lead/tail s predznakom) s `moved` (s rupama: strane same ili zajedno pomicu rez tona; bez rupa: jedan rez tona ostaje jedan, preklapanje se mice cijelo), `bounded`, `sides_at`, `dragged`, `nearer`, `moving`, `of_piece`, `focus`. `qnc-program-segments` (`cut_part.rs`: `SoundEdge` je `SoundSide`; pomak, povlacenje i bliza strana kroz kocku) i `qnc-script` (`JoinSide` je `SoundSide`; `JoinSound::moved`, strana izabranog slota, bliza strana i crvene linije zooma kroz kocku) vise nemaju vlastite kopije. Ponasanje isto: testovi `qnc-program-segments` (72), `qnc-script` (15), nova kocka (3), conformance, release build s libav. Slika reza ostaje kod vlasnika; geste nisu mijenjane (zakljucana mapa misa).
Izvedeno 2026-11-04 (korisnik: "da, kreni s korakom 5", docs/97): jedno pravilo ruba komada. Nova javna cista kocka `crates/qnc-edit-edge` (+ ugovor `contracts/modules/edit-edge.module.json`): `Edge` (In, Out), `Range` (komad [from, to) u klipu) s `nudged` (odbije kad bi presao drugi rub ili izasao iz klipa), `moved`, `dragged` (koliko moze, komad zadrzava sliku), `at`, `picture`, te `rolled` (roll dva komada koliko oba mogu). `qnc-source-mark-focus` (`nudge`, `drag`; time i rub segmenta Ctrl+Shift+I/O) i `qnc-script` (strelice i kotacic, povlacenje, `roll_to`) vise nemaju vlastite kopije. Ponasanje isto: testovi `qnc-source-mark-focus` (7), `qnc-source-timeline`, `qnc-source-panel`, `qnc-panel-focus`, `qnc-program-segments` (72), `qnc-script` (15), nova kocka (3), conformance, release build s libav. Ostaje: rub slota u prici (`SlotTrim`, os price s rippleom).
Izvedeno 2026-11-04 (korisnik: "da, kreni s korakom 6", docs/97): jedno znacenje rijeci. Nova javna kocka `crates/qnc-edit-words` (+ ugovor `contracts/modules/edit-words.module.json`): `Word` s `goal` (uzeti IN na pocetak rijeci, OUT na kraj, rez na blizu granicu) i `WordPair` (dvije rijeci istog mjesta: od pocetka ranije do kraja kasnije, kojim god redom). Koriste je `qnc-source-timeline` (`cut_word`), `qnc-script` (`WORD_CUT`, klik na rijec dok je rub dijela uzet, `zoom_word`) i `qnc-panel-focus` (rijec u zoomu Storyja, uzeta source oznaka na rijec). Novo: klik na rijec u prijepisu dok je source IN ili OUT uzet donese oznaku na rijec (prije je samo pomicao playhead), isto kao u zoomu; `qnc-transcript-panel` uz pocetak salje i kraj rijeci. Testovi `qnc-edit-words` (2), `qnc-script`, `qnc-source-timeline`, `qnc-panel-focus` (zlatna tablica tipki), conformance, release build s libav. Uzivo nije provjereno.
Popravak 2026-11-04 (korisnik, snimka zooma: "u zoomu su pobjegli gumbi, nisu centrirani; nakon Spremi nemoj zatvoriti; dodaj dugme koje radi isto sto ESC"): gumbi zooma su uvijek u sredini prozora (prije na rezu, pa su na rubu prozora bjezali u stranu). Gumbi su Ponisti | Spremi | Zatvori (Esc): Spremi je nova radnja `zoom_save` (`qnc-edit-verbs` `Verb::Save`, katalog bez tipke) koja upise uzeto kao Enter i ostavi ga uzetim - Story (`qnc-program-segments` `save_and_keep`: rub slota ili M marker s istim dijelom (cijeli, slika, ton i strana) ponovno uzet nakon upisa i ponovnog citanja price, `take_again`), slozena izjava (`qnc-script` `save_and_keep`: rub, rez, ton spoja, ton dijela, dio cijeli), source IN/OUT ostaje uzet; Zatvori (Esc) pusta uzeto (rjecnik `zoom.close`). Testovi, conformance, release build s libav. Uzivo nije provjereno.
Plan 2026-11-04 (korisnik: "pushaj kao novu verziju danasnjeg datuma; napravi plan za dodavanje opcija koje nam nedostaju"): verzija pushana kao oznaka `v-2026-11-04` (grana `libav-decoder`). `docs/98-missing-tools-plan.md`: ostaci objedinjavanja (rub slota na `qnc-edit-edge`, J/L slike u slozenoj izjavi, klik lijevo/desno od reza - odluka korisnika, `control read header timed out` iz loga 4. 11.), alati po skupinama s predvidenim kockama (brzina montaze: JKL shuttle, snapping, match frame, trim mode, slip, lift/extract, insert; ton: krivulja razine, ducking, ciscenje govora; slika i grafika: potpisi, titlovi iz prijepisa, zamagljivanje lica, prekrivanje reza, brzina, reframe; organizacija: biljeske markera, dva monitora) i prijedlog redoslijeda. Kod nije mijenjan.
Izvedeno 2026-11-04 (korisnik: "kreni s tockom 1", docs/98 tocka 0): (1) `control read header timed out` (log 4. 11.: nakon skoka player 2-2,6 s nije odgovarao): `qnc-broadcast-engine` `output.rs` - zvuk klipa nakon skoka koji ton u memoriji ne pokriva otvarao je dekoder za svaki kanal kamere redom na niti playera; sada ih otvara u pozadini (`DecodeInput::open_in_background`, `Opening`), a dok se otvaraju zvuk ceka kao dekoder bez uzoraka. (2) J/L slike spoja slozene izjave: `qnc-script-db` `Piece.v_lead_frames`/`v_tail_frames` i operacija `SetJoinPicture` (pripadaju spoju kao pauza: ostaju uz pomak ruba i premjestanje dijelova, prvi dio ih nema; test), `qnc-program-input` `PieceInput.picture_lead`/`picture_tail` (slika kao J/L slike price, ton ostaje), `qnc-script` (`JoinSound.picture`: V u zoomu uzetog reza spoja uzima sliku, A1/Ctrl+1 ton; Ctrl+I J, Ctrl+O L, strelice i kotacic, Enter i Spremi pisu `write_join`, Esc odustaje; nacrt se odmah vidi; bez rupa). (3) Rub slota u prici ne prelazi na `qnc-edit-edge` (rub izmedju M markera na osi price, ne komad u klipu). (4) Klik lijevo/desno od uzetog reza ceka odluku korisnika. Testovi `qnc-script-db`, `qnc-script`, `qnc-program-input`, `qnc-broadcast-engine`, `qnc-broadcast-player`, conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-04 (korisnik: "kreni s tockom 2", docs/98): (1) Snapping: nova javna kocka `crates/qnc-snap` (+ ugovor): povuceni IN/OUT na source timelineu, M marker i strana J/L na Wrap retku lijepe se za playhead, druge markere i rubove pokrivalica i komada unutar 8 piksela; `qnc-timeline` (`snap_targets`); tipka S (`toggle_snapping`, QNC preset, source_timeline i storyboard) ukljucuje i iskljucuje za ovaj proces (zapis u player log). (2) Match frame: nova javna kocka `crates/qnc-match-frame` (+ ugovor; cista, omjer price i izvora 2:1 i 1:2): tipka F (`match_frame`) na panelu Segmenti otvara klip i sliku pod Wrap playheadom na source timelineu (pokrivalica ispred segmenta), drugdje stavlja Wrap playhead na mjesto u prici gdje se prikazana slika izvora koristi (ponovno F: sljedece mjesto); `qnc-panel-focus` `match_frame`; tablica tipki zna F. (3) Slip: `qnc-edit-edge` `Range::slipped`; prica: operacija `SlipSegment` (`qnc-program-db`: isti klip i duljina, IN/OUT pomaknuti, markeri, slotovi i pokrivalice ostaju; jedan Undo korak; test) i `qnc-program-segments` `slip.rs` (Ctrl+Shift+S `select_slip` uzme segment pod Wrap playheadom, strelice pomicu, Enter/Spremi pise, Esc odustaje; nacrt se ne svira prije Entera); slozena izjava: `qnc-script` `slip_key` (isti glagol za dio u ruci, nacrt se odmah svira, Enter/Spremi `set_piece`). Katalog i rjecnik. Testovi `qnc-snap`, `qnc-match-frame`, `qnc-edit-edge`, `qnc-program-db` (56), `qnc-program-segments` (72), `qnc-script`, conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-04 (korisnik: "kreni s tockom 3", docs/98 1.1): JKL shuttle. Nova javna cista kocka `crates/qnc-shuttle` (+ ugovor `contracts/modules/shuttle.module.json`): L naprijed 1x (play playera, slika i ton), ponovno L 2x, 4x, 8x; J unazad 1x, ponovno 2x, 4x, 8x; K stani. Svaka brzina osim 1x naprijed mice playhead sama (cue za cueom, samo slika, od mjesta gdje je brzina pocela). `qnc-panel-focus` (`shuttle_key` na pocetku `route`, s bilo kojeg panela; `shuttle_tick` u `poll`; `busy` dok shuttle radi, pa forma crta svakih 15 ms); Space i klik na timeline zaustavljaju shuttle. Katalog: `shuttle_back` J, `shuttle_stop` K, `shuttle_forward` L (QNC preset, source_timeline i storyboard); tablica tipki zna K. Ingest zasad nema shuttle (tipke ne rade nista). Nije izvedeno: zvuk pri 2x i unazad, dinamicki trim (docs/98 1.4). Testovi `qnc-shuttle`, `qnc-panel-focus` (zlatna tablica tipki), `qnc-keyboard-shortcut`, conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-05 (korisnik: "kreni s dinamickim trimom", docs/98 1.4): dinamicni trim. Nova javna cista kocka `crates/qnc-trim-play` (+ ugovor `contracts/modules/trim-play.module.json`): `TrimLoop` (play oko uzetog reza, 2 s prije do 2 s poslije, ukrug: vrati, sviraj, opet; zavrsava kad netko drugi pauzira) i `moved_by`. Shift+K (`trim_play_around`, QNC preset, source_timeline i storyboard) s uzetim rezom (IN/OUT, rub slota, M marker, strana J/L, rub dijela slozene izjave) pusta petlju oko reza; K zaustavlja i uzeti rub ide na mjesto gdje je playhead stao, istim koracima kao strelice (pa vrijede pravila uzetog reza; najvise 500 slika), Enter/Spremi upisuje, Esc odustaje. JKL u trimu: J i L vrte, zoom prati playhead i za vrijeme shuttlea (`qnc-source-preview` `set_scrubbing`: `playing_at` broji shuttle kao play), K pomakne rub. Space i klik na timeline zaustavljaju bez pomaka ruba. `qnc-panel-focus` (`trim.rs`, `route`, `poll`). Testovi `qnc-trim-play`, `qnc-panel-focus` (zlatna tablica tipki), `qnc-source-preview`, `qnc-keyboard-shortcut`, conformance, release build s libav. Uzivo nije provjereno.
Izmjena 2026-11-05 (korisnik: "u skriptu nepotrebno je ispisivati vremenski prikaz, samo stvara nepreglednost"): u bloku izjave scenarija (MA Audio, MA Script) iznad rijeci vise nema timecodea; vrijeme rijeci je samo pod misem, klik i dvoklik na rijec rade kao prije (`qnc-script-panel`). Rez po slovima (fino vrijeme u pozadini samo za tekst prikazan u scenariju) nije izveden, ceka plan.
Izvedeno 2026-11-05 (korisnik: "bilo bi super da postoji finiji timecode koji bi omogucio rezanje po slovima u skriptu, automatski u pozadini samo na odabranom tekstu koji se prikazuje u skriptu"; "cisti Rust bez drugih jezika"; "da, preuzmi i kreni"): vremena slova. Model CLASSLA wav2vec2-xls-r-parlaspeech-hr (hrvatski, slova, CER oko 2 %) preuzet u `data/models/wav2vec2-parlaspeech-hr` (`model.safetensors` 1,26 GB, `config.json`, `vocab.json`; izvan gita; licenca na stranici modela nije navedena, provjeriti prije beta paketa). Nove kocke (s ugovorima modula): `crates/qnc-wav2vec2` (wav2vec2 CTC u cistom Rustu, candle 0.9: sedam konvolucija, projekcija, pozicijska konvolucija s weight norm, 24 transformera, glava slova; vjerojatnosti slova svakih 20 ms), `crates/qnc-ctc-align` (cisto prisilno poravnanje CTC Viterbi, razmak izmedu rijeci neobavezan, znak koji model nema bez vremena), `crates/qnc-letter-align` (ugovor dodatka `contracts/plugins/letter-align.v1.json`, `qnc.letter-align` 1, obje strane), program `tools/qnc-align-wav2vec2` (model ucitan jednom, ispod normalnog prioriteta), `crates/qnc-letter-db` (modul tablica posrednika: `word_letters`, `public_word_letters`; ugovor baze `letter-db.database.json`; conformance: na popisu javnih DB vlasnika), `crates/qnc-letter-times` (u pozadini, samo dok player miruje: rijeci izjava koje scenarij prikazuje bez slova, nizovi do 20 s, govor s kanala govora projekta kroz `qnc-sound-range`, 16 kHz WAV u praznoj privremenoj mapi, poravnanje, zapis; neuspjele rijeci se u istom radu ne traze ponovno). AI katalog racunala ima `letters` (program i model; `qnc-ai-catalog` `installed_letter_aligner`). `qnc-script` cita slova uz prijepise, stavlja ih na rijeci (`put_letters`: slovo traje od svog pocetka do pocetka sljedeceg) i pokrece pozadinski posao; `qnc-script-panel`: klik na slovo stavlja playhead na njega, dvoklik na dva slova postavlja IN na prvom i OUT na kraju drugog (isti `script_word_cut`), pod misem slovo i vrijeme. Provjereno: model sam prepisuje probni isjecak (Mironik 3608, 12 s) gotovo tocno ("molim vas predstavite ja sam stankao karaman inace fotoreporter novinar 42 godine u novinarstvu"), poravnanje poznatog teksta daje vrijeme svakog slova (npr. Stanko 4,92-5,28 s); 12 s govora s ucitavanjem modela oko 7,6 s. Testovi `qnc-ctc-align`, `qnc-letter-db`, `qnc-script`, conformance, release build. Uzivo kroz aplikaciju nije provjereno; zoom reza jos nema slova.
Dopuna 2026-11-05 (korisnik: "dodaj slova i u zoom reza"): zoom reza (Story, S-Story, source IN/OUT, slozena izjava) pokazuje slova rijeci na mjestu gdje su izgovorena, s crticom na pocetku svakog; slovo pod misem je osvijetljeno, klik na njega stavlja uzeti rez na slovo (uzeti IN na pocetak slova, OUT na kraj, rez na bliziju granicu; isti glagol kao rijec). Rijec sa slovima traje od prvog do kraja zadnjeg slova (finije od poravnanja po valu, koje ostaje za rijeci bez slova). Rijeci u prozoru zooma bez slova dobivaju ih u pozadini (`qnc-letter-times` `Need::few`). Pravilo trajanja slova (od pocetka do pocetka sljedeceg) je jedno, u `qnc-letter-db` (`WordLetters::spans`, `letters_of`), i koriste ga scenarij i zoom. `qnc-program-view` `ZoomWord.letters`, `qnc-zoom-words`, `qnc-trim-overlay`. Debug cache obrisan (`cargo clean --profile dev`, 23,2 GiB; disk je imao 1,3 GB). Testovi, conformance, release build. Uzivo nije provjereno.
Popravak i dopuna 2026-11-05 (korisnik: "u timeline ne rade osnovne komande strelica lijevo-desno ni kotac misa... sve funkcije moraju raditi i u timeline i u zumu"; "trebala bi i obrnuta selektivnost: pozicija playheada postavljena na slova"): log je pokazao da strelice i kotacic na timelineu slozene izjave stizu do scenarija, ali ih je obradivao samo dok je uzet rub dijela; bez njega playhead je stajao. Sada bez uzetog: strelice i kotacic pomicu playhead za sliku, Home na pocetak, Alt+strelice na prethodnu/sljedecu granicu dijela, Alt+I na IN dijela pod playheadom, kao na source timelineu (`qnc-script` `trim_key`). U scenariju i u tekstu prozora medija slovo pod playheadom je osvijetljeno (rijec sa slovima; bez slova cijela rijec kao prije; `qnc-script-panel`). Mjerenje 61 crtanja u sekundi dok je slozena izjava otvorena nakon ponovnog pokretanja se nije ponovilo (2-6 u sekundi); privremeni zapis uklonjen. Testovi, conformance, release build. Uzivo nije provjereno.
Izvedeno 2026-11-05 (korisnik: "treba standardizirati sve timeline i pripadajuci zum.. sve opcije moraju koristiti iste
kocke"): audit i plan u `docs/97` odjeljak 5 (tablica radnji po timelineu i zoomu, sest koraka); korak 1 izveden. Nova
javna cista kocka `crates/qnc-edit-step` (+ ugovor `contracts/modules/edit-step.module.json`): korak po tipki isti na
svakom timelineu i zoomu (`frames`: strelice i kotacic source timelinea i Wrap redova uz smjer kotacica racunala;
`ctrl_frames`: Ctrl+kotacic pomice uzetu oznaku; `level`: Ctrl+strelice i Ctrl+kotacic, gore glasnije svugdje;
`arrow`, `level_action`, `is_wheel`). Postavka smjera kotacica `data/mouse-wheel.json` premjestena iz `qnc-wheel-jog`
(ponovno je izvozi). Koriste je `qnc-source-timeline`, `qnc-source-panel`, `qnc-panel-focus`, `qnc-script`,
`qnc-program-segments`, `qnc-segments-keys`, `qnc-sync-session`; njihove vlastite pretvorbe strelica i kotacica
uklonjene. Ponasanje isto: zlatne tablice tipki i misa nepromijenjene, testovi dotaknutih kocki, conformance. Ostali
koraci (kljucna slika i match frame za slozenu izjavu, jedan protokol uzetog, rez i undo slozene izjave, jedno znacenje
Alt+strelica i klika na sliku zooma) cekaju; 4-6 trebaju odluku korisnika.
Izvedeno 2026-11-05 (korisnik: "slozena izjava je isto sto i pokrivalice, samo pokrivalice koriste A2 a slozena izjava
A1"; "nigdje ne treba biti rupa slike, samo rupa tona"; "da, kreni s korakom A"; plan `docs/97` 5.3): korak A, rupa u
tonu u prici za sve. Negativni J ili L tona je rupa: ton prije zavrsi prije reza ili ton poslije pocne poslije njega, u
rupi razina 0, slika nedirnuta; J/L slike i dalje nikad ispod 0. `qnc-program-db`: stupci `a1_lead_frames`,
`a1_tail_frames`, `a2_lead_frames`, `a2_tail_frames` bez provjere >= 0 (prica napravljena s njom gradi se iznova s istim
retcima, bez pretvaranja), `SetJoinSound`, `SetCutSoundEdges`, `SetCoverOverlap` primaju i negativne (najvise 360000 u
svakom smjeru), strana bez pokrivalice ne prima ni rupu; pomak samo slike reza zadrzava rupu gdje je bila; A2 Tona u Off
nema rupu. `qnc-program-playlist`: rupa A1 na spoju dvaju klipova (`hole_a1`, unutar susjednih klipova) i rupa A2
pokrivalice (`holes_a2`: njezin ton ceka ili zavrsi ranije unutar njezine slike). `qnc-program-input`: slozena izjava
salje rupe istim putem (vlastita lista rupa uklonjena). `qnc-program-segments`: ton reza (A1 i A2) kroz `qnc-edit-cut` s
rupama, kao slozena izjava; promjena: pomak oba ruba tona sada pomice sam rez tona (ton prije zavrsi ranije, ton poslije
pocne ranije), ne pravi kratko preklapanje kao prije; povlacenje strane preko reza je rupa. Testovi `qnc-program-db`
(novi: rupa i prica sa starom provjerom zadrzava retke), `qnc-program-playlist` (novi: rupa A1 i A2), `qnc-program-segments`
(dva testa promijenjena na novo pravilo), ostale dotaknute kocke, conformance, release build s libav. Uzivo nije provjereno.
Izmjena 2026-11-05 (korisnik, MA Script prozor medija: "brisu se Izjave Dodaj IN-OUT i redak Jos nema izjava, tekst izjava
ostaje"; "naslov Prijepis ostaje"; "umjesto Dodaj oznaceni IN-OUT u pricu kao: Potrazi: i polje za upis pojma za
pretragu"): panel prozora medija grupe g je nova vrsta `transcript_search_panel` (`qnc-side-pieces`, `qnc-right-panel`,
`contracts/ui/editorial.layout.json`): isti prijepis i naslov, spremljene izjave ostaju, bez retka Izjave / Dodaj IN-OUT /
Kopiraj i bez poruke kad izjava nema (`qnc-transcript-panel` `show_searched`, `quotes::show(.., header)`). AI Media Assist
(grupa e) nije mijenjan. Traka iznad source timelinea umjesto natpisa ima "Potrazi:" i polje (`qnc-transcript-panel`
`search_field`, modul `search.rs`; gumbi + IZJAVA / + STAND-UP / + NETON ostaju): pojam (jedna ili vise uzastopnih rijeci,
bez obzira na velika slova i interpunkciju) svijetli u prijepisu, trenutni nalaz u boji fokusa i doveden u pogled, uz polje
"n od m" ili "nema"; Enter (nova akcija kataloga `transcript_search_next`, Enter u opsegu `transcript_search`, QNC preset)
ide na sljedeci nalaz i stavlja playhead na njega. Pojam je izbor prikaza ovog procesa (egui memorija), nigdje se ne pise.
Sifre `writer.clip_bar*` uklonjene, nove `transcript.search*`. Test `a_term_of_one_or_more_words_is_found_whatever_the_case_and_punctuation`,
conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-05 (korisnik: "u tekstu dozvoliti brisanje rijeci i iz timelinea automatski isjeci taj dio videa"; "na taj
nacin bi izbacili postapalice"; "ne bi koristio pretapanje"): izjava u scenariju (MA Audio, MA Script, tekst u prozoru
medija) uredjuje se brisanjem rijeci. Klik na rijec (ili slovo, kad su slova poravnata) stavlja playhead na nju i odabire je,
Shift+klik na rijec istog dijela prosiri odabir od prve odabrane do nje (nova radnja kataloga `script_word_pick`, bez
tipke), odabrane rijeci su crvene i precrtane; Delete ih izreze iz videa: dio postaje ono prije i ono poslije njih (isti klip,
slika i ton zajedno), spojeni cistim rezom bez pauze i bez pretapanja; rijeci na pocetku ili kraju dijela samo ga skrate,
svih rijeci dijela - dio izlazi (zadnji dio ostaje). Esc pusta odabir. Nova kocka unutar `qnc-script` `word_pick.rs`
(`WordPick`, `left_of`), operacija `qnc-script-db` `CutOut` (prvi ostatak zadrzava spoj dijela, drugi dobiva rez; izjava od
jednog raspona postaje svoj prvi dio), `qnc-script-panel` (Shift+klik, oznaka). Rjecnik `script.words_cut_out`,
`script.word_hint`. Nije izvedeno: Undo (baza scenarija nema korake undoa), prijelaz izracunat dodanim slicicama na spoju
(planirana `qnc-cut-smooth`), odabir i brisanje u panelu prijepisa AI Media Assista. Testovi
`words_cut_out_of_a_part_leave_what_was_before_and_after_them_joined_by_a_cut`,
`a_pick_grows_from_its_first_word_whichever_way_and_cuts_out_its_frames`, conformance, release build s libav. Uzivo nije
provjereno.
Dopuna 2026-11-05 (korisnik: "klik lijevim misem na traku koja pokazuje trajanje klipa selektira slot i u timelineu i u
zumu; klik na filmstrip dio koji nije pokriven trakom, desni klik se odnosi na marker"): na timelineu slozene izjave to je
vec pravilo (traka: lijevi klik bira slot, desni uzima dio cijeli; filmstrip izvan trake: desni klik najblizi rub ili rez).
Sada isto u zoomu reza: ispod svake slike je traka trajanja njezina slota u boji duljine (`cover_lengths`) s duljinom;
lijevi klik na traku bira slot (`subsegment_select`), desni ga uzima cijelog (`subsegment_take`), izabrani slot ima bijeli
okvir; desni klik na sliku izvan trake uzima marker tog reza (`subsegment_take_edge`, spoj bez pauze je cijeli rez, inace
rub dijela). `qnc-program-view` `EditStill.strip` (`StillStrip`) i `right_click`, `qnc-script` (`subsegment_zoom`),
`qnc-subsegment-panel` (boja trake), `qnc-trim-overlay` (`paint_strip`). Zoom Storyja nije mijenjan. Test zooma prosiren,
conformance, release build s libav. Uzivo nije provjereno.
Izmjena 2026-11-05 (korisnik: "lijevi klik na traku trajanja selektira slot, desni klik na selektirani slot u istoj traci
trajanja otvara zoom u slot modu i rezovi se odnose na slot (produljivanje ili skracivanje)"): zamjenjuje desni klik na traku
od 4. 11. (uzimao dio cijeli za pomak, Ctrl+S ostaje za to). Desni klik (i Ctrl+klik) na traku trajanja dijela slozene
izjave bira taj slot i uzima njegov IN ili OUT blizi kliku (`subsegment_take_edge` s ciljem `n:in|out:slot`), uvijek vlastiti
rub, nikad roll: otvara se zoom reza, strelice, kotacic i povlacenje produljuju ili skracuju slot, ostali zadrzavaju duljinu,
Enter sprema. Isto na traci ispod slike u zoomu (lijeva slika OUT svog slota, desna IN). Lijevi klik dok je rub uzet i dalje
sprema (zakljucano pravilo misa). `qnc-script` (`pointer.rs`, `TAKE_EDGE`, `subsegment_zoom`). Testovi, conformance, release
build s libav. Uzivo nije provjereno.
Popravak 2026-11-05 (korisnik: "nema iznimaka, jedna radnja u svim situacijama radi isto; desni klik = Ctrl+; desni klik na
traci trajanja, bez obzira je li timeline ili zoom (zoom je uvecani dio timelinea), mora raditi isto"; log: u zoomu je lijevi
klik na traku birao slot i dok je rub bio uzet, a na timelineu ga je spremao kao Enter; slika zooma je imala vlastite radnje):
zoom slozene izjave vise nema vlastitih radnji. Svaka slika stoji na frameu osi slozene izjave (lijeva zadnji frame dijela
prije reza, desna prvi poslije) i svaki klik salje radnju koju mapa misa timelinea daje istoj gesti na tom frameu, s ciljem i
frameom: lijevi klik na sliku `timeline_cue_frame` (playhead, dok je nesto uzeto sprema kao Enter), desni klik na sliku
`select_marker` s rubom (marker), lijevi klik na traku trajanja `timeline_select_cover` (slot; dok je uzeto, sprema), desni
klik na traku `timeline_take_cover` (slot i njegov blizi rub). Odgovor daje isti vlasnik osi (`qnc-script::own_action`) kao za
timeline. `qnc-program-view` `StillAction` (radnja, cilj, frame) za `click`, `right_click` i traku; `qnc-trim-overlay`
(`event_of`); `qnc-subsegment-panel` (radnje reda iz zooma na panelu reda). Test: zoom daje iste vlastite radnje kao timeline
na istom frameu. Conformance, release build s libav. Uzivo nije provjereno. Zoom Storyja nije mijenjan.
Izmjena 2026-11-05 (korisnik: "primijeni isto pravilo i na zoom u Storyju i S-Storyju, svugdje gdje koristimo timeline i zoom;
lijevi klik na sliku u zoomu zatvara zoom, to je greska; treba oznaciti ono sto je selektirano, npr. A1 prethodnog ili
narednog klipa, da korisnik vidi na sto ce se primijeniti radnja"): u svakom zoomu reza (Story, S-Story, slozena izjava)
lijevi klik na sliku ili na traku ispod nje bira tu stranu reza i nikad ne sprema niti zatvara zoom (u Storyju i S-Storyju
`zoom_pick_side` uzetog reza, u slozenoj izjavi `subsegment_select` slota te strane). Strana koja je izabrana je jasno
oznacena: crveni okvir njezinih slika, crveni okvir njezine trake i crveno uokvirena, blago obojena polovica A1/A2 te
strane. Desni klik je Ctrl+ kao na timelineu: na traci uzima slot (`select_current_marker_slot` u Storyju, u slozenoj
izjavi slot i njegov rub), na slici izvan trake marker reza (`select_marker`). Story i S-Story zoom dobili su traku slota
ispod svake slike (`qnc-program-view` `story_zoom_clicks`, poziva je `qnc-program-segments` refresh). Boja trake po duljini
na jednom mjestu za sve zoomove: `qnc-segment-panel::trim_zoom` (poziv iz `qnc-editorial-blocks` je isti redak, i iz
`qnc-subsegment-panel`). Klikovi zooma nose panel u kojem je zoom (`qnc-trim-overlay` `event_of`). Testovi
`a_story_zoom_picture_has_its_slots_strip_and_the_markers_right_click` i test zooma slozene izjave, conformance, release
build s libav. Debug cache obrisan (`cargo clean --profile dev`, 22,2 GiB; disk je imao 93 MB slobodno). Uzivo nije provjereno.
Izmjena 2026-11-05 (korisnik: "desni klik treba biti dio svih kontrolnih radnji: za rezanje videa preko teksta selektirana
rijec desni klik i Delete brisu tu rijec, isto vrijedi i za slova; 100% standardizirati, desni klik misem = Ctrl+"):
zamjenjuje odabir rijeci klikom i Shift+klikom (iz istog dana). U tekstu izjave (scenarij MA Audio i MA Script, tekst
otvorene izjave u prozoru medija) lijevi klik na rijec ili slovo samo stavlja playhead; desni klik (Ctrl+) uzima rijec ili
slovo ispod misa (nova radnja kataloga `script_word_take`, bez tipke); Shift+desni klik uzima od prvog uzetog do nje
(`script_word_pick`); Delete izreze uzeto iz videa, Esc ga pusta. Uzeta slova crtaju se slovo po slovo, crveno i precrtano.
`qnc-script` (`WORD_TAKE`, `pick_key`), `qnc-script-panel`, katalog, rjecnik. Panel prijepisa klipa (AI Media Assist, prozor
medija bez otvorene izjave) nije mijenjan: nije odredeno sto Delete brise u sirovom prijepisu klipa. Testovi, conformance,
release build s libav. Uzivo nije provjereno.
Popravak 2026-11-06 (korisnik: "ne radi, ili ja ne znam koristiti"; log: u MA Scriptu nijedan `script_word_take` ni Delete,
samo klikovi i dvoklikovi na rijeci): rijeci i slova u tekstu izjave vise nisu tekst koji se oznacava misem
(`selectable(false)`), pa desni klik ide samo riječi; uzimanje, prosirenje, Delete i Esc nad rijecima pisu se u player log
(`script-words`). `qnc-script-panel`, `qnc-script`. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "jednu uspio brisati, vise ne dozvoljava selektiranje; ne radi ni undo preko Backspace"; log
i kopija baze projekta, samo citanje: brisanje rijeci je radilo, ali su raniji dvoklikovi na istu rijec suzili dijelove
izjave na jednu rijec (dio 4521..4522 je 1 slicica, 4294..4298 bez rijeci), pa ostale rijeci tih dijelova nisu imale vrijeme
i nisu se mogle uzeti; baza scenarija nije imala undo): undo i redo scenarija. `qnc-script-db`: tablica `script_undo`
(modul je vlasnik; korak je cijela tablica blokova kakva je bila prije promjene, kao JSON), svaka promjena blokova (dodaj,
tekst, pomak, brisanje, polja, dijelovi, spoj, rubovi, J/L, izrezivanje rijeci) zapisuje korak u istoj transakciji kao
promjena; operacije `Undo` / `Redo` (`undo`, `redo`), nova promjena brise redo, cuva se 200 koraka; snimka glasa bloka se
nikad ne ponistava (ostaje kakva je snimljena); odbijena promjena nije korak. `qnc-script`: Backspace / Ctrl+Z / Undo
(`undo_object`) ponistava zadnju promjenu scenarija, Shift+Backspace (`redo_object`) je ponavlja; uzeto se pusta; poruke u
rjecniku (`script.undone`, `script.nothing_to_undo`...). Promjene napravljene prije ove verzije nemaju korake. Test
`every_change_of_the_script_is_one_undo_step_and_redo_takes_it_again`, conformance, release build s libav. Primjer
`qnc-script-db/examples/pieces_raw.rs` (samo citanje kopije baze). Uzivo nije provjereno.
Izmjena 2026-11-06 (korisnik: "nakon brisanja jedne rijeci selekcija i brisanje vise ne rade; mozda je najbolja opcija
obicni klasicni edit teksta: sto se obrise u tekstu, brise se i u tonu"; log: brisanje i undo rade, ali dvoklik na istu
rijec (`script_word_cut`) opet je suzio dio izjave na tu rijec, pa ostatak teksta nije imao vremena): zamjenjuje uzimanje
rijeci desnim klikom i dvoklik IN/OUT u tekstu izjave (5. i 6. 11.). Tekst svakog dijela izjave (scenarij MA Audio i MA
Script, tekst otvorene izjave u prozoru medija) je obicno polje za uredivanje teksta: brisu se rijeci ili slova kao u svakom
editoru (tipkovnica, mis, oznacavanje i undo samog polja); Enter (`script_text_cut`, opseg kataloga `script_text`) izreze iz
slike i tona dijela ono sto tekst vise ne kaze, Esc (`script_text_revert`) vraca tekst; dok je tekst kakav je izgovoren,
klik stavlja playhead na kliknutu rijec, a rijec pod playheadom je osvijetljena. Upisan tekst nije izgovoren i ne ostaje.
Nova kocka unutar `qnc-script` `text_edit.rs` (`text_of`, `kept_ranges`: poravnanje rijeci teksta s izgovorenima,
skracena rijec zadrzava preostala slova kad su poravnata, inace ostaje cijela; izrezano na pocetku ili kraju dio samo
skracuje), `qnc-script-db` operacija `KeepOf` (dio postaje rasponi koji ostaju: prvi zadrzava spoj, ostali rez; `CutOut` je
njezin poseban slucaj; jedan korak undoa), `qnc-script-panel` `text_editor.rs`. Testovi `text_edit` (2), conformance,
release build s libav. Uzivo nije provjereno.
Popravak 2026-11-06 (korisnik: "zasto se ne vidi transcribirani text u MA Script kad se klikne na transcribirani klip";
"jedina mjerodavna istina je baza, druga veza medu formama ne smije postojati"): log: Mironik 3608 (clip-b07ff91a) prepisan
je i upisan u bazu (Done), ali MA Script je prijepis tog klipa procitao dok je jos bio u radu i taj prazan rezultat drzao u
memoriji kao istinu; ponovno citanje bilo je samo na promjenu klipa ili na kraj prijepisa u istoj formi. Sada `qnc-transcript`
prijepis prikazanog klipa cita iz baze ponovno svake sekunde (`kept_at`, `QUEUE_EVERY`), tko god ga je napisao; prikaz se
slaze iznova samo kad baza kaze nesto drugo. Nema veze medu formama. Testovi, conformance, release build s libav. Uzivo nije
provjereno.
Izvedeno 2026-11-06 (korisnik: plan drugog agenta "izvucite stvarnu strukturu alatima... spremi dijagrame u docs/arhitektura.md"):
novi `docs/arhitektura.md`: velicina (312 paketa, ~157 000 redaka), Mermaid dijagram 35 sredisnjih kocki iz `cargo metadata`,
put klipa od kartice do exporta s promjenama tipova i mjestima dvostrukog posla, sekvenca jednog ciklusa forme i playera,
revizija s prioritetom (`qnc-script` 35 ovisnosti, `qnc-panel-focus` 32, dvostruko citanje prijepisa, 40 paketa s
neiskoristenim ovisnostima po `cargo machete`, traitovi s jednom implementacijom, clippy pedantic 4446 upozorenja) i
prijedlog redoslijeda pojednostavljenja. Instalirani alati `cargo-machete` 0.9.2 i `cargo-modules` 0.26.0 (0.28 trazi
rustc 1.98). Kod nije mijenjan.
Izvedeno 2026-11-06 (korisnik: "da, kreni s prvom tockom" iz `docs/arhitektura.md`, P1-1): jedan citac prijepisa. Nova
javna kocka `crates/qnc-clip-transcripts` (+ ugovor `contracts/modules/clip-transcripts.module.json`): prijepise klipova
koje kocke pokazuju cita iz baze projekta u pozadinskoj niti, ponovno svake sekunde (tko god ih je napisao), i daje ih
svima; drzi samo zadnji odgovor baze, promjenu vidi usporedbom (`rev`), neuspjelo citanje zadrzava zadnji odgovor i kaze
razlog; `poll_from` razrjesava bazu u pozadinskoj niti, `read_since` kaze je li citanje novije od trenutka; nista ne
pise. Prije su prijepis citala tri mjesta, svako sa svojom memorijom: `qnc-transcript` (panel prijepisa), `qnc-script`
(rijeci izjava, `read.rs`) i `qnc-zoom-words` (rijeci u zoomu reza; nije ga nasao pregled iz `docs/arhitektura.md`). Sada:
`qnc-side-pieces` drzi jedan primjerak za panel prijepisa i scenarij (klip na previewu i klipovi izjava,
`Script::transcript_clips`) i daje ga u `Transcript::poll` i `Script::poll`; `qnc-zoom-words` koristi kocku za prijepise,
a vremena slova (`letter-db`) cita i dalje sam; nakon vlastitog prijepisa ne prepisuje ponovno dok kocka ne procita bazu
nakon njega. Testovi dotaknutih kocki, conformance, release build s libav. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, obrisi debug cache i kreni s drugom tockom" iz `docs/arhitektura.md`, P1-2): slozena izjava
izdvojena iz `qnc-script` u novu javnu kocku `crates/qnc-subsegment-edit` (+ ugovor
`contracts/modules/subsegment-edit.module.json`). Kod je premjesten, ne prepisan: stanje uzetog (rub, roll reza, J/L zvuka
i slike spoja, dio cijeli kao slot, slip, A1 dijela), tipke i mis slozene izjave (`pointer.rs`), crtanje osi i zoom reza
(`view.rs`), radnje panela (playhead, zatvaranje, A1 dijela, mjesto i izlazak dijela, pauza i spoj) i testovi. Kocka nista ne
pise: svaki upis predaje vlasniku (`take_writes`), a scenarij ga pise u svoju bazu (`qnc-script-db`) jednim upisom, redom;
poruku i prikaz slozene izjave scenarij preuzima (`take_edit`). Scenarij joj daje svoje blokove, nazive klipova, projekt i
je li citanje/upis u tijeku (`sync`). `trimmed_piece` i `seconds_of` su slobodne funkcije kocke (koristi ih i scenarij za rez
teksta). `qnc-script` ponovno izvozi sve njezine nazive, pa paneli nisu mijenjani. `qnc-script`: `lib.rs` 1946 -> 904 retka,
nasih ovisnosti 35 -> 28 (uklonjeno 9 koje vise ne koristi). Testovi `qnc-subsegment-edit` (4), `qnc-script`, `qnc-side-pieces`,
`qnc-panel-focus` (zlatna tablica tipki), `qnc-subsegment-panel`, `qnc-script-panel`, `qnc-editorial-application`, cijeli
workspace se kompajlira. Uzivo nije provjereno. Debug cache obrisan (`cargo clean --profile dev`, 19,4 GiB).
Izvedeno 2026-11-06 (korisnik: "da, kreni s trecom tockom" iz `docs/arhitektura.md`, P1-3): `qnc-panel-focus` sveden na
usmjeravanje. Kod je premjesten, ne prepisan: shuttle (J/K/L) i dinamicni trim (Shift+K, K pomakne uzeti rub) su nova javna
kocka `crates/qnc-playhead-moves` (+ ugovor `contracts/modules/playhead-moves.module.json`; vozi samo preview, na K s uzetim
rezom kaze koliko slika rub ide, a fokus ga mice koracima strelica kao prije); gore/dolje na uzetom rezu (skok na kljucnu sliku
i primjena odgovora) nova javna kocka `crates/qnc-keyframe-jump` (+ ugovor `contracts/modules/keyframe-jump.module.json`);
match frame (F), tipke Export prozora i rijec u zoomu Wrap redova (koraci do granice rijeci) u `qnc-segments-keys`
(`match_frame`, `export`, `word_steps`); rijec na uzetu source oznaku u `qnc-source-timeline` (`mark_to_word`); kopija
`mark_cue` uklonjena (koristi se `qnc-source-panel::mark_cue`). Fokus drzi samo redoslijed kocki i krug panela: 27 ovisnosti
(prije 32, uklonjene i neiskoristene `qnc-wheel-jog` i `eframe`), 1042 retka (prije 1207). Ponasanje isto: zlatna tablica
tipki nepromijenjena, testovi `qnc-panel-focus`, `qnc-segments-keys`, `qnc-source-timeline`, `cargo check --workspace`,
conformance, `cargo machete` bez neiskoristenih ovisnosti. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da,,," na P1-4 iz `docs/arhitektura.md`; pravilo korisnika: "jedina mjerodavna istina je
baza, druga veza medu formama ne smije postojati"): predmemorije prikazanog klipa koje su bazu citale jednom po klipu sada je
citaju u ritmu. Nova javna kocka `crates/qnc-clip-follow` (+ ugovor `contracts/modules/clip-follow.module.json`): jedna
vrijednost klipa koji forma pokazuje, procitana u pozadinskoj niti i procitana ponovno u ritmu dok je klip prikazan; neuspjelo
citanje zadrzava zadnji odgovor i kaze razlog (`error`); vrijednost koju je pozivatelj sam zapisao (`set`) ne ponistava
starije citanje; dok pozivateljev upis putuje, citanje ceka (`may_read`). Sama nista ne cita: citanje daje pozivatelj.
Koriste je (cetiri kopije istog "procitaj jednom" uklonjene): `qnc-clip-sound` `ClipSounds` (svake sekunde: zvuk A1/A2
zadrzan u drugoj formi ili na drugom racunalu se cuje), `qnc-source-channels` (zapis klipa dovrsen poslije, u pozadini, daje
broj kanala), `qnc-source-timecode` `SourceTimecodes` i `qnc-clip-shots` (svake 3 s i nakon sto analiza postoji: ponovno
napravljena analiza se pokaze). Source timeline (`qnc-source-timeline`) zvuk klipa ne primjenjuje vise samo jednom: kad baza
odgovori drukcije (druga forma ga je zadrzala) i nijedna traka nije uzeta, primijeni ga. Val, filmstrip i posteri i dalje se citaju dok nedostaju (pisu se jednom po verziji). Testovi
`qnc-clip-follow` (2: promjena drugog pisca se vidi, neuspjelo citanje cuva zadnji odgovor; zapisana vrijednost se ne
ponistava starijim citanjem), testovi dotaknutih kocki, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, kreni s ciscenjem ovisnosti"; P2-5 i P2-6 iz `docs/arhitektura.md`): uklonjeno 59
neiskoristenih ovisnosti iz 33 paketa (nalaz `cargo machete`, svaki provjeren gradnjom `cargo check --workspace --all-targets`
i gradnjom s `libav`) te kocke koje nitko ne koristi, `qnc-editorial-shell` i `qnc-source-input` s ugovorima
`contracts/modules/editorial-shell.module.json` i `source-input.module.json` i kljucevima C5 u
`tools/qnc-conformance/lego-baseline.json` (kod ostaje u git povijesti). Ostavljeno: `xml-rs` u `qnc-camera-profiles` i
`qnc-sony-metadata` (uvozi se kao `xml`, lazni nalaz), `qnc-decoder-catalog` u `qnc-app` i `qnc-story` (treba ga znacajka
`libav`), te ovisnosti zamrznutog Projecta (`qnc-project-blocks`, `qnc-project-session`; manifest Projecta trazi
dir-browser). Svaka biblioteka workspacea sada ima korisnika. Conformance prolazi. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, kreni s P2-7 i P2-8" iz `docs/arhitektura.md`): traitovi s jednom implementacijom i
prosljedivaci. Uklonjeno: `MonotonicScheduler` u `qnc-broadcast-player` (nitko ga nije koristio osim vlastitog testa s laznim
rasporedivacem; test uklonjen, 79 testova playera), `ClipInputs` u `qnc-program-input` (jedna implementacija u istoj kocki:
`PlayerClipInputs` s vlastitim metodama), traitovi `FilmstripContentWriteFactory` i `WaveContentWriteFactory` (sada tip
funkcije koja pokrece pisca za nit radnika; tvornice u `qnc-content-artifacts` su zatvaranja). Ostaju jer su granica:
`DecoderJobs` (ugovor dodatka dekodera), `PictureSource` (libav je zasebna kocka koju `qnc-media-decode` ne smije znati),
`FrameBlocks` (ploca ne zna za desktop), citaci i pisci filmstripa i vala (radnici ne znaju za bazu). Prosljedivaci:
`qnc-ingest-store::content` uklonjen, Ingest kocke, `qnc-player-input` i dijagnostika koriste `qnc-content-store` izravno
(pravilo conformancea "public DB write boundary" trazi isti uzorak pod novim imenom); `qnc-wheel-jog` vise ne izvozi postavku
`qnc-edit-step`, `qnc-right-panel` vise ne izvozi konstante `qnc-side-pieces`. Ponasanje isto. Testovi dotaknutih kocki,
`cargo check --workspace --all-targets`, conformance (baseline spusten: dijagnostika vise ne ovisi o
`qnc-ingest-store`). Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, kreni s P3" iz `docs/arhitektura.md`), korak 1: suvisne kopije. `cargo clippy --fix` za
`redundant_clone` i `implicit_clone` u 52 datoteke (105 redaka; bez Projecta, koji je zamrznut); jedan prijedlog clippyja
(`qnc-pixel-convert` `preview.rs`) bio je pogresan jer se vrijednost kasnije koristi i nije primijenjen. U
`tools/qnc-ingest-worker` uklonjena mrtva dodjela (`stuck` se pocetnom vrijednoscu nikad nije citao). Ponasanje isto. Testovi
42 dotaknuta paketa, gradnja cijelog workspacea.
Izvedeno 2026-11-06 (P3, korak 2: pretvorbe brojeva). Pregled `clippy` (oko 860 upozorenja pretvorbi) pokazao je da pretvorbe
iz decimalnog broja u cijeli broj u Rustu nisu opasne (definirano zasicuju, NaN je 0), a `u64 -> i64` puca tek iznad 2^63
slika. Stvarna opasnost je negativan `i64` pretvoren u neoznaceni broj (tiho postane golem); od 117 takvih mjesta vecina je
zasticena (`.max(0)`, `clamp`, provjera prije, omjer price 1:1/2:1/1:2). Nezasticena mjesta s vremenom (slike, rasponi,
brojevi iz baze) sada idu kroz provjerenu pretvorbu (`try_from`): `qnc-program-db` (rub i pomak slota u S-Storyju kroz
postojeci `source_frame`, pomak klipa unutar izvora vraca gresku kraja klipa umjesto prelijevanja, broj koraka undoa),
`qnc-content-read` (potpis kataloga, revizija zapisa, redni broj slicice filmstripa, duljina klipa), `qnc-artifact-db`
(redni broj slicice), `qnc-transcript-db` (raspon reda prijepisa i prijepisa), `qnc-program-view` (slika pokrivalice u zoomu
reza). Ponasanje za ispravne podatke isto; negativna vrijednost iz baze daje 0 ili gresku umjesto golemog broja. Testovi
dotaknutih kocki.
Izvedeno 2026-11-06 (korisnik: "pushaj kao novu verziju, pa nastavi"; verzija `v-2026-11-06`; sljedece po `docs/98`
redoslijed 4: 2.1 krivulja razine): korak 1, krivulja razine u zvuku. Segment (A1) i pokrivalica (A2) imaju tocke razine:
`qnc-program-db` stupac `story_items.level_keys` (`[[slika klipa, desetinke dB], ...]`, prazno bez krivulje; prici
napravljenoj prije dodaje se, redovi ostaju; citac starije price trazi vlasnika da ga doda), `LevelKey` (slika izvornog
klipa, pa tocka ostaje na svom zvuku kad se komad skrati ili pomakne; od -60 do +12 dB, najvise 256, jedna po slici),
operacija `SetLevelKeys` (cijela krivulja odjednom, jedan korak Undo; lead je nema); podjela pokrivalice markerom kopira
krivulju. Izmedju dvije tocke razina ide pravcem u dB, prije prve i poslije zadnje ostaje njihova; krivulja se dodaje na
razinu trake. `qnc-program-input` daje tocke playlisti na osi izvora u stopi price; `qnc-program-playlist`
(`level_curve.rs`, `LevelPoint`, `LevelRamp`): svaka stavka se dijeli na tockama, pa je preko jedne stavke razina rute
jedan pravac (`ProgramAudioRoute.level_ramp`), i u J/L produzecima; `qnc-broadcast-engine` (sabirnica mnozi uzorke
pravcem; dio prerendera nosi krivulju na krajevima), `qnc-render-unit` (`AudioPart.level_ramp`, u otisku samo kad postoji,
pa dijelovi bez krivulje zadrzavaju otisak; `fade_at` ukljucuje krivulju, pa je primjenjuju prerender i export). Ton A2 u
Off i slozena izjava zasad bez krivulje. Testovi `qnc-program-db` (58), `qnc-program-playlist` (26), `qnc-render-unit`,
`qnc-broadcast-engine`, `qnc-program-input`, `qnc-program-render`, `qnc-player-input`, `qnc-fcp7-xml`. Nije izvedeno
(korak 2, ceka odluku o gestama jer je mapa misa zakljucana): crtanje krivulje na Wrap retku i u zoomu, dodavanje,
pomicanje i brisanje tocaka; val programa jos ne prati krivulju. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "drzimo se ustaljenih tipova komandi: desni klik na liniju nivoa tona selektira liniju
nivoa tona, klik postavlja playholder, linija playholdera je tocka koja se kontrolira; desni klik = Ctrl+"): korak 2
krivulje razine (docs/98 2.1), uredivanje na Wrap retku. Nova kocka `crates/qnc-level-curve` (+ ugovor
`contracts/modules/level-curve.module.json`): uzeta krivulja jednog segmenta (A1) ili pokrivalice (A2), preslikavanje
slike price i slike klipa (`Span`, i 2:1), tocka na liniji playheada (`nudge` za 1 dB, nastaje ako je nema; `remove_at`),
nacrt i krivulja za upis (`keep`), krivulja na osi price za crtanje (`place`, `Placed::within`). `qnc-program-view`:
`SegmentRow.level_keys`, `CoverSpan.level_keys`, `SegmentsView.level_line`, `level_curves`, `level_item_at`.
`qnc-timeline` (`level_lines.rs`, `TimelineLevelCurve`, `TimelineInput.level_curves`, `TimelineTheme.level_curve`):
krivulja se crta u pojasu A1/A2 Wrap retka (0 dB na sedam desetina visine, -60 dB dno, +12 dB vrh), tocke kao
kvadratici, uzeta krivulja zuta s kruzicem na tocki playheada; desni klik do 4 piksela od linije je
`TakeLevelLine` (prije zvuka segmenta ili pokrivalice). Mapa misa (zakljucana, prosirena uz odluku korisnika):
`right_click_level_a1` / `right_click_level_a2` -> `select_level_a1` / `select_level_a2` (`qnc-mouse-map`
`RightClickLevelA1/A2`). Katalog: `select_level_a1` Ctrl+Shift+1, `select_level_a2` Ctrl+Shift+2 (QNC preset,
storyboard; Ctrl+Alt bi na hrvatskoj tipkovnici bio AltGr). `qnc-program-commands` `TakeLevelLine`;
`qnc-program-segments` (`level_line.rs`, `Take::LevelLine`: dok je uzeta nista drugo se ne uzima; Enter je pise
`SetLevelKeys`, jedan korak Undo); `qnc-segments-keys` `level` (gore/dolje, Ctrl+strelice i Ctrl+kotacic 1 dB,
Delete brise tocku na playheadu, Esc odustaje; strelice lijevo/desno i kotacic pomicu playhead) i
`keep_level_on_click` (lijevi klik na Wrap retku sprema krivulju i ostavlja je uzetom, playhead ide na klik: sljedeca
tocka); svaka promjena se odmah cuje (`SoundDraft.levels`, program se otvara s nacrtom). `qnc-panel-focus` (Ctrl+Shift+1/2
s bilo kojeg panela prebacuje na Segmente). Nije izvedeno: val Wrap reda jos ne prati krivulju; krivulja u zoomu reza;
Ton A2 u Off i slozena izjava bez krivulje. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "provjeri konzistentnost komandi i sredi sortkat key-map help"): novi alat
`crates/qnc-keyboard-shortcut/examples/audit.rs` (samo citanje kataloga: ista tipka na dvije radnje u scopeu, radnje s
tipkom koje pomoc F1 ne pokazuje, radnje u pomoci bez tipke). Nalaz u QNC presetu: 13 radnji s tipkom bez mjesta u pomoci,
3 u pomoci bez tipke, zastarjelo pravilo "Rez M markera" (Ctrl+Shift+J uklonjen 29. 10., Ctrl+2 uzima obje strane tona
od 29. 10.). Pomoc (`help` u `contracts/qnc-keyboard-shortcuts.json`): pravilo "Rez M markera" prema danasnjem
postupku, nova pravila "Krivulja razine" i "J / K / L", "Strelice" s kljucnom slikom dok je rez uzet, dijakritike;
grupe: Reprodukcija (J, K, L, Shift+K, F), Audio (krivulja razine, N), Segmenti (Ctrl+Shift+S slip, Ctrl+E zoom,
Ctrl+Delete), Opcenito (S snapping); uklonjeni `delete_part` (nema tipku od 21. 10.), tabovi SubSegment i Voice (bez
tipke). Ostaje: Enter i Esc su u pravilima; Shift+W (`toggle_source_wrap`) je u katalogu iz v4, ali ga nijedna kocka ne
obraduje (mrtva tipka); u scopeu `project` Enter je i `form_next_field` i `project_open_selected` (namjerno: polje
obrasca), u `timeline_modal` Esc i `clear_focus` i `close_player`. Oznake radnji u katalogu ostaju kao u v4 (conformance).
Testovi `qnc-keyboard-shortcut`, `qnc-key-cheatsheet`, `qnc-panel-focus`, conformance, `cargo check --workspace
--all-targets`.
Izvedeno 2026-11-06 (korisnik: "makni Shift+W i pushaj"): Shift+W (`toggle_source_wrap`) uklonjen iz QNC preseta
(scopeovi `storyboard` i `timeline_modal`): nijedna kocka ga nije obradivala, tipka nije radila nista. Radnja ostaje u
katalogu i u ostalim presetima (preslika qnc_v4); conformance iznimke od qnc_v4 dopunjene. Testovi
`qnc-keyboard-shortcut`, `qnc-panel-focus`, `qnc-conformance`, conformance.
Izvedeno 2026-11-06 (korisnik: "nastavi s valom i zoomom za krivulju razine"): korak 3 krivulje razine (docs/98 2.1).
Pravilo krivulje je jednom, u `qnc-gain` (`curve_at`, `db10_to_gain`); koriste ga `qnc-program-db` (`LevelKey::level_at`,
`LevelKey::pairs`), `qnc-program-playlist` (`LevelPoint`, `LevelRamp`), `qnc-level-curve`, `qnc-timeline` i
`qnc-render-unit` (cetiri kopije uklonjene). Val: `qnc-program-waveform` `Placement.curve` i `gain_at` (razina i krivulja
na slici klipa); prate je val Wrap redova (`qnc-program-view` `wave_placements`), mjerenje glasnoce za NORM
(`qnc-program-loudness`, i pravilo pokrivalica u `qnc-program-input` `normalize`) i fini val zooma (`qnc-fine-wave`);
nacrt uzete krivulje vidi se u valu prije Entera (`qnc-program-segments` `view_with_waves`). Zoom: dok je krivulja uzeta,
zoom reza je oko tocke na liniji playheada (`qnc-program-view` `point_stills`: zadnja slika prije i slika na tocki,
rjecnik `story.still.before_point`, `story.still.at_point`), prati playhead (`set_playhead`); preko A1/A2 zooma crtaju se
krivulje istom funkcijom kao na timelineu (`qnc-timeline` `paint_level_curves`, `level_curve_hit`, pravilo 0.12), desni
klik na liniju u zoomu je `select_level_a1/a2` kao na timelineu. Pretvorba krivulja price u krivulje timelinea je jednom,
`qnc-trim-overlay` `timeline_level_curves` (koristi je i panel Segmenti). Zoom source IN/OUT (os klipa) nema krivulje.
Testovi `qnc-gain`, `qnc-program-waveform`, `qnc-program-segments` (73), `qnc-level-curve`, `qnc-program-loudness`,
`qnc-timeline`, `qnc-trim-overlay`, `qnc-program-db`, `qnc-program-playlist`, `qnc-fine-wave`, `qnc-program-view`,
conformance, `cargo check --workspace --all-targets`. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "mozda bi za finalno sredivanje tona u fino mogao postojati AI agent"; "da, pushaj i kreni s
pomocnikom tona, da ga korisnik moze ukljuciti i iskljuciti"): pomocnik tona, prvi dio. Nova cista kocka
`crates/qnc-sound-assist` (+ ugovor `contracts/modules/sound-assist.module.json`): iz glasnoce izmjerene s valom svakog
klipa (odsjecci od 100 ms) predlaze krivulje razine: pokrivalica (A2) se pod govorom A1 (blok glasniji od -40 LUFS,
praznine do 0,5 s spojene, najmanje 0,3 s) stisava rampom od 200 ms koliko treba da ambijent bude oko 15 dB ispod govora
(najvise 20 dB, manje od 2 dB se ne radi); govor svakog segmenta ide na medijan govora segmenata (najvise 10 dB, manje od
1,5 dB se ne radi). Stavka s vlastitom krivuljom korisnika (i nacrt uzete krivulje) ostaje kakva jest. Ukljucen ili
iskljucen je zapis u bazi price: `qnc-program-db` stupac `story_state.sound_assist` (prici napravljenoj prije dodaje se,
`ADDED_COLUMNS` sada za vise tablica), `ProgramSnapshot.sound_assist`, operacija `SetSoundAssist` (jedan korak Undo).
`qnc-program-input`: mjerenja klipova izdvojena u `Measures` (koriste ih normalizacija pokrivalica i pomocnik), pomocnik
radi poslije normalizacije (`assist.rs`), pa ga jednako cuju player, prerender i export. `qnc-program-segments`
(`assist.rs`, predmemorija prijedloga): krivulje pomocnika crtaju se na Wrap redovima i u valu i ulaze u mjerenje
NORM-a; `ToggleSoundAssist` (`qnc-program-commands`), getteri `normalizing`, `sound_assist_on` (popravak: NORM u traci
razine citao je `view().normalize`, koji osnovni prikaz nije postavljao). Prekidac: dugme AI u traci razine
(`qnc-level-bar`, uz NORM, vraca kliknutu radnju), `qnc-source-timeline` `offer_assist`; radnja kataloga
`toggle_sound_assist` (Shift+N, QNC preset, s bilo kojeg panela), pomoc F1 grupa Audio, rjecnik `story.ui.sound_assist*`,
`story.msg.sound_assist_on/off`. Testovi `qnc-sound-assist`, `qnc-program-db` (59), `qnc-program-segments` (74),
`qnc-program-input`, `qnc-level-bar`, `qnc-source-timeline`, `qnc-panel-focus`, `qnc-keyboard-shortcut`, `qnc-ui-text`,
conformance, `cargo check --workspace --all-targets`. Nije izvedeno: Ton A2 u Off i slozena izjava bez pomocnika;
prijedlog jos nije moguce "zadrzati" kao korisnikove krivulje. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "pushaj i dodaj zadrzavanje prijedloga kao korisnikove krivulje"): prijedlog pomocnika
tona zadrzava se kao korisnikove krivulje razine. `qnc-program-db` operacija `SetLevelKeysMany` (sve krivulje odjednom u
jednoj transakciji: stavka koja ne prima krivulju ne ostavlja nista upisano; jedan korak Undo). `qnc-program-segments`
pamti krivulje pomocnika kakve se prikazuju (`assist_curves`, s id-em segmenta ili pokrivalice, na slikama klipa) i
`keep_sound_assist` ih upisuje; od tada su obicne krivulje (Ctrl+Shift+1/2, desni klik na liniju), a pomocnik te dijelove
ne dira. Poruke: pomocnik iskljucen, nema prijedloga, zadrzano n dijelova (rjecnik `story.msg.sound_assist_kept`,
`_keep_off`, `_keep_none`). Radnja kataloga `sound_assist_keep` (Ctrl+Shift+N, QNC preset, s bilo kojeg panela), pomoc
F1 grupa Audio, `qnc-program-commands` `KeepSoundAssist`. Testovi `qnc-program-db` (60), `qnc-program-segments` (75),
`qnc-program-commands`, `qnc-panel-focus`, `qnc-keyboard-shortcut`, `qnc-ui-text`, conformance, `cargo check --workspace
--all-targets`. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "dodaj dugme za zadrzavanje pokraj AI"): u traci razine (`qnc-level-bar`) desno od AI je
dugme ZADRZI / KEEP (radnja `sound_assist_keep`, isto sto Ctrl+Shift+N); moze se pritisnuti samo dok je pomocnik ukljucen
(inace sivo). Sirina trake ostaje 240 px, mjerac je uzi. Rjecnik `story.ui.sound_assist_keep`, `_keep_hint`. Testovi
`qnc-level-bar`, `qnc-ui-text`, `qnc-source-timeline`, conformance, `cargo check --workspace --all-targets`. Uzivo nije
provjereno.
Analiza 2026-11-06 (korisnik: "pronadi svako mjesto gdje je procesna logika pomijesana s egui kodom ... predlozi podjelu
na qnc-core i qnc-gui"): `docs/99-core-gui-split.md`. 83 paketa izravno ovise o egui-ju, 25 logickih paketa vuce egui
preko UI paketa (najvise zbog tipova `qnc-timeline`: `TimelineProjection`, `LaneKey`; te `Alert`, `Choice`,
`MonitorState`, `Follow`, `Wheel`); mjesovite kocke `qnc-source-timeline`, `qnc-timeline`, `qnc-focus-walk`,
`qnc-keyboard-shortcut`, `qnc-newsroom-sheet`, `qnc-monitor`; stanje rada u egui memoriji (nacrt teksta izjave, pretraga
prijepisa, kanal povlacenja markera kroz `ctx`); poll aplikacije tece iz petlje crtanja (`qnc-form-host`). Prijedlog:
ne dva monolitna paketa (AGENTS 0.11, 12), nego pravilo slojeva core/gui s conformance provjerom i dijeljenje
mjesovitih kocki na `qnc-x` + `qnc-x-view`; pet koraka od najmanje rizicnog. Kod nije mijenjan.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s koracima 1 i 2", docs/99): korak 2, cisti tipovi iz UI kocki u core
kocke (samo premjestanje, UI kocka ih ponovno izvozi, ponasanje isto). Nove kocke (s ugovorima modula):
`crates/qnc-timeline-model` (`TimelineProjection`, `TimelineSourceMarkFocus`, `AudioLane`, `TimelineIntent`),
`crates/qnc-notice` (`Alert`, `Choice`, `ChoiceOption`; crtanje ostaje u `qnc-alert` i `qnc-choice-box`),
`crates/qnc-monitor-state` (`MonitorFrame`, `MonitorState`, Auto/On), `crates/qnc-desktop-entry`
(`DesktopApplicationRef`). `Wheel` i `WheelActions` u `qnc-edit-step`; `LaneKey`, `lane_key`, `lane_message` u
`qnc-lane-choice`; `Follow` u `qnc-source-preview`; `qnc-panel-focus` vise ne izvozi `qnc_ui_kit::paint_focus`.
Logicke kocke (prica, scenarij, preview, Ingest) koriste core kocke umjesto UI kocki. C14 baseline 36 -> 11 veza.
Testovi 25 dotaknutih kocki (i zlatna tablica tipki), conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s gui-kit pravilom"; prije "mozda uvesti i gui-kit"): gui-kit su
postojeci `qnc-ui-kit`, `qnc-desktop-theme` i `qnc-theme-styles`, bez novog paketa. Conformance C15
(`tools/qnc-conformance/src/lego.rs` `gui_kit_rules`): kocka koja crta, osim forme (C2) i gui-kita, ne pravi vlastiti
gumb ili kvacicu ni boju mimo teme; gui-kit ne ovisi ni o jednoj logickoj kocki osim `qnc-dev-diagnostics` i
`qnc-board`. Danasnjih 25 prekrsaja upisano u `lego-baseline.json` (smiju samo padati). Provjereno: conformance,
testovi conformancea (26), mutacijska provjera (smanjena brojka obara conformance). Kod kocki nije mijenjan.
docs/99 dopunjen.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s tekstualnim poljem u gui-kitu"): `qnc-ui-kit` (`text_field.rs`)
daje tekstualna polja: `text_field` (jedan redak), `row_field` (uz oznaku, visine reda, tekst u sredini; prije dvije
kopije u `qnc-newsroom-sheet` i `qnc-settings-form`), `text_area` (vise redaka), `search_field`. Prebaceni bez
promjene izgleda: `qnc-newsroom-sheet`, `qnc-settings-form` (nova ovisnost `qnc-ui-kit`), `qnc-settings-rows`,
`qnc-script-panel` (polja bloka i tekst izjave), `qnc-text-source-editor`, `qnc-transcript-panel` (pretraga). C15
broji i `TextEdit::` u kockama koje crtaju; danas nijedan, pa baseline nema novog kljuca. Project ploca (zamrznuta
forma) zadrzava svoja polja. Testovi dotaknutih kocki, `cargo check --workspace --all-targets`, conformance. Uzivo
nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni sa segment panelom"; gui-kit C15): `qnc-segment-panel` crta gumbe
i boje samo kroz gui-kit. Gumbi trake (akcija, bljesak, ikona) kroz `qnc_ui_kit::look_button` s istim izgledom
(`bar_look`), gumbi redaka taba Segment kroz novi `qnc_ui_kit::small_button` (mali tekst, oznaka izabranog za Edit);
boje traka pokrivalica iz ugovora kroz `qnc_desktop_theme::rgb` (sada javna, jedina pretvorba), boja ukupnog trajanja
u zaglavlju je `qnc_desktop_theme::total_text` (ista vrijednost). Cetiri C15 kljuca segment panela uklonjena iz
baselinea (25 -> 21). Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni sa subsegment panelom"; gui-kit C15): gumbi retka dijela slozene
izjave (ranije, kasnije, ukloni) u `qnc-subsegment-panel` crtaju se kroz `qnc_ui_kit::small_button` (nova ovisnost),
koji dobiva parametar `enabled` (sivo kad se ne moze; segment panel predaje `true`). Izgled isti. C15 kljuc
subsegment panela uklonjen iz baselinea (21 -> 20). Testovi, `cargo check --workspace --all-targets`, conformance.
Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s trim overlayem"; gui-kit C15): trim zoom je uvecani timeline
(pravilo 0.12), pa ima boje teme timelinea. `TimelineTheme` je dobio boje reza (`cut`, `cut_side`, `unheard`); sve boje
teme timelinea gradi gui-kit (`qnc_theme_styles::timeline`, tijelo premjesteno iz `TimelineTheme::from_qnc_theme`, koji
je uklonjen; `qnc-theme-styles` dobio ovisnost `eframe`). `qnc-trim-overlay::show` prima `&TimelineTheme` (predaju ga
`qnc-segment-panel::trim_zoom`, Story ploca u istom retku i subsegment panel); vlastite konstante boja uklonjene, boja
trake iz ugovora kroz `qnc_desktop_theme::rgb`, Ponisti/Spremi/Zatvori kroz novi `qnc_ui_kit::sized_button`. Izgled
isti (blagi crveni sloj strane u fokusu sada je `cut` s istom prozirnoscu). C15: 20 -> 16 (`qnc-timeline` i
`qnc-trim-overlay` bez prekrsaja). Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s media cardom"; gui-kit C15): `qnc-media-card` vise nema vlastitih
boja. `CardStyle` je dobio boje tockica stanja (`dot_ready`, `dot_original_ready`, `dot_pending`, `dot_error`) i
kvacice izbora (`check_on`, `check_mark`, `check_off`, `check_off_rim`), s istim vrijednostima; puni ih gui-kit
(`qnc_theme_styles::cards`). `paint_clip_status` prima `&CardStyle`. Izgled isti. C15: 16 -> 15. Testovi,
`cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni sa shell footerom"; gui-kit C15): `qnc-shell-footer` i `qnc-desktop`
nemaju vlastitih boja ni gumba. Palete tema Soft i High contrast (AGENTS 16: dok ne budu u UI ugovoru) premjestene iz
`ThemeId::palette` u gui-kit `qnc_theme_styles::shell_palette` (Dark je i dalje paleta ugovora ljuske); boje statusa su
`qnc_desktop_theme::danger` (ista vrijednost) i nova `done`; Close project kroz `qnc_ui_kit::sized_button` (sada prima
bilo kakav tekst); `qnc-desktop` boje ugovora pretvara kroz `qnc_desktop_theme::rgb` (vlastiti `rgb` uklonjen). Izgled
isti. C15: 15 -> 12. Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s level meterom"; gui-kit C15): `qnc-level-meter` nema vlastitih
boja. Zone mjeraca (ispod -12, od -12, od -3 dBFS) su `MeterStyle.zones`; daje ih traka razine iz teme timelinea
(`TimelineTheme.meter`), koju puni gui-kit (`qnc_theme_styles::timeline`), s istim vrijednostima. Izgled isti.
C15: 12 -> 11. Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-06 (korisnik: "da, pushaj i nastavi"; gui-kit C15): gui-kit (`qnc-ui-kit`) dobio je `framed_button`
(tekst pozivatelja, ispuna, obrub od 1 px ili bez, ravni ili zaobljeni kutovi, najmanja velicina) i `check_box`
(moze biti iskljucen). Kroz njih idu gumbi potvrdnog dijaloga (`qnc-confirm-dialog`), gumbi transporta glave poola i
naredbe reda (`qnc-media-pool-head`), kvacice trake uvoza (`qnc-ingest-import-bar`), kvacica Locked obrasca priloga
(`qnc-newsroom-sheet`) i prekidaci postavki (`qnc-settings-switches`). Izgled isti. C15: 11 -> 5.
Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije
provjereno.
Izvedeno 2026-11-06 (isti nastavak): gumbi zaglavlja source docka (`qnc-source-dock`, nova ovisnost `qnc-ui-kit`) kroz
`qnc_ui_kit::framed_button`; `qnc-ingest-layout::rgb` je samo `qnc_desktop_theme::rgb` (jedna pretvorba boje ugovora).
C15: 5 -> 3; ostaju urednik profila scenarija (3 gumba) i dijagnosticki alat (7 gumba, 5 boja).
Izvedeno 2026-11-06 (korisnik: "da, pushaj i kreni s urednikom profila"; gui-kit C15): kvacice urednika profila
scenarija (`qnc-script-profile-editor`: Locked, snima se (samo izgovorena vrsta), in/out cue) kroz
`qnc_ui_kit::check_box`. Izgled isti. C15: 3 -> 2; ostaje samo dijagnosticki alat (`tools/qnc-dev-diagnostics-app`).
Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-07 (korisnik: "da, pushaj i prebaci dijagnosticki alat na gui-kit"; gui-kit C15): dijagnosticki alat
(`tools/qnc-dev-diagnostics-app`) gumbe crta kroz `qnc_ui_kit::sized_button`, kvacicu kroz `check_box`, a boje loga
(zaglavlje, redak, greska) su `qnc_desktop_theme::log_colours`, iste vrijednosti. C15: 2 -> 0, nijedna kocka koja crta
nema vlastitog gumba, kvacice, tekstualnog polja ni boje; svaki novi je greska conformancea. docs/99 dopunjen.
Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-07 (korisnik: "da, pushaj i kreni s korakom 3, odmrzni i rijesi o projekt"; docs/99 korak 3, prvi
dio): mjesovite kocke podijeljene. `qnc-source-timeline` je core (stanje, tipke, zvuk klipa, A1/A2); crtanje (`show`,
`show_with`, `Instead`, `HeaderAction`, `header_actions`) je nova kocka `crates/qnc-source-timeline-view` (+ ugovor),
koju koriste Story/MA ploca, Ingest ploca i traka uvoza. `qnc-focus-walk` je core; citanje egui unosa popisa
(`list_focus`, `list_keys`) je nova kocka `crates/qnc-list-focus` (+ ugovor), koristi je Ingest ploca. Iz `qnc-timeline`
u core: `event` (radnja mape misa) u `qnc-timeline-model`, `ChannelChoice`, `lane_choice`, `drafted_choice` u
`qnc-lane-choice` (`qnc-channel-picker` i `qnc-timeline` ih ponovno izvoze); ime opsega tipki source timelinea je
`qnc_timeline_model::SOURCE_TIMELINE_SCOPE` (`qnc-key-intents` ga ponovno izvozi). Ponasanje isto. C14 11 -> 4 (ostaje
`qnc-project-session`). Testovi (96, ukljucujuci zlatnu tablicu tipki), `cargo check --workspace --all-targets`,
conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-11-07 (isti zahtjev, "odmrzni i rijesi o projekt"; docs/99 korak 3, drugi dio): Project
otkljucan samo za odvajanje logike od crtanja. `qnc-project-session` vise ne ovisi ni o jednoj kocki koja crta:
obrazac priloga kao podatak je nova core kocka `crates/qnc-newsroom-sheet-model` (+ ugovor; listovi racunala i
pripremljeni, `SheetOverlay`, `Answer`, `today`; testovi listova premjesteni), `qnc-newsroom-sheet` ga crta i
ponovno izvozi; `qnc-location-field` je core, crtanje preglednika polja je nova `crates/qnc-location-field-view`
(+ ugovor), koju koristi Project ploca; `BrowserAction` i `BrowserAnswer` su u `qnc-dir-browser`
(`qnc-dir-browser-panel` ih ponovno izvozi); iskljucivo otvaranje jednog od dva polja (`toggle_exclusive_panel`, jedini
korisnik) premjesteno iz gui-kita u `qnc-location-field`, s testovima. Conformance "shared UI patterns" to trazi u
`qnc-location-field`, a pravilo preglednika Project ploce trazi `qnc_location_field_view::show_browser`. Ponasanje isto.
C14 4 -> 0. Testovi, `cargo check --workspace --all-targets`, conformance. Uzivo nije provjereno. Project je ponovno
zamrznut.
Izvedeno 2026-11-07 (korisnik: "da, pushaj i kreni s korakom 4"; docs/99 korak 4): stanje rada iz egui memorije
vlasnicima. (1) Povlacenje M markera i strane J/L tona: `qnc-timeline` ga vraca kao svoju namjeru
(`qnc-timeline-model` `TimelineIntent::DragMarker`, `DragCutEdge`, radnje mape misa `drag_marker`, `drag_cut_edge`;
zlatna tablica pravila 0.12 dobila je povlacenje J/L linije); skriveni kanal kroz egui kontekst
(`take_marker_drag`, `take_cut_edge_drag`, `take_marker_drag_event`) uklonjen, `qnc-segment-panel` i
`qnc-subsegment-panel` uzimaju namjeru reda. (2) Nacrt teksta izjave prije Entera drzi `qnc-script`
(`ScriptView.text_drafts`, `text_draft`; radnje kataloga bez tipke `script_text_draft`, Esc `script_text_revert`
sada ide scenariju); `qnc-script-panel` pokazuje nacrt i salje sto je natipkano; nacrt rijeci izgovorenih drukcije
nestaje. (3) Pojam pretrage prijepisa i trenutni nalaz drzi `qnc-transcript` (`search.rs`, `SearchView`,
`TranscriptView.search`; nalazi se racunaju iznova kad se redci promijene; radnja `transcript_search_term`, a
`transcript_search_next` (Enter) sama stavlja playhead na sljedeci nalaz); polje pretrage (`qnc-transcript-panel`
`search_field`) i traka "Dodaj u pricu" (`qnc-script-writer-panel` `show_clip_bar`) dobivaju `SearchView`;
`qnc-side-pieces` osvjezava prikaz prijepisa odmah nakon njegove radnje. U egui memoriji ostaje izbor prikaza
(tabovi, otvoren prozor, skrol jednom do nalaza ili oznake) i stanje geste. Testovi
`the_edited_text_of_a_statement_is_the_scripts_until_enter_or_esc`,
`the_term_is_the_transcripts_and_enter_goes_round_its_finds`, testovi dotaknutih kocki (i zlatna tablica tipki),
`cargo check --workspace --all-targets`, conformance. Debug cache obrisan (`cargo clean --profile dev`, 31,4 GiB;
disk je bio pun). Uzivo nije provjereno.
Izvedeno 2026-11-07 (korisnik: "da, pushaj i kreni s korakom 5"; docs/99 korak 5, prvo mjerenje): iz postojeceg
`player.log` (34 689 sekundi s dijagnostikom, zadnji dani): dok forma crta 20+ puta u sekundi `form-poll` je 161 ms/s
(3,3 ms po crtanju, p99 9,7 ms), `form-paint` 130 ms/s, cijeli okvir desktopa 346 ms/s; u mirovanju poll 17 ms/s.
10 995 sporih okvira (p50 33 ms, p99 191 ms) bez podatka o bloku. Poll u vlastitu nit zasad se ne radi (player je
zaseban proces s visim prioritetom). Dodano samo mjerenje: `qnc-dev-diagnostics::take_frame_blocks` (blokovi jednog
okvira), `ui-frame-slow` ih pise (`qnc-desktop`), a nemjereni dio polla Story/MA dobio je imena `poll-catalog`,
`poll-posters`, `poll-sync` (`qnc-editorial-application`) i `poll-side` (`qnc-panel-focus`); ponasanje isto, C10
isti. Testovi `qnc-dev-diagnostics` (novi `a_frame_gives_its_blocks_once`), `qnc-desktop` (jedan test jednom pao na
brisanju privremene mape, 4 ponavljanja prolaze), conformance, release build `qnc-app` s libav. Odluka o niti nakon
zive sesije s novim zapisom.
Popravak 2026-11-07 (korisnik: "kliknuo sam A1, nista se ne dogada, na slozenoj izjavi"; "ne vidim liniju za
podesavanje nivoa"; "bitno je da se selektira slot u timeline, tako da se promjena kanala odnosi na njega"): log je
pokazao da klik na A1 u slozenoj izjavi uzima A1 dijela (`subsegment-key select_audio_a1`), ali timeline slozene
izjave nikad nije dobio stupac kanala ni razinu uz oznaku A1 (izbor je bio samo u retku dijela ispod timelinea, koji
u prozoru medija MA Scripta nije vidljiv). Usporedba sa stanjem prije odvajanja logike od crtanja (`872b83cb`):
source timeline, mapa misa, `lane_choice`/`ChannelChoice` i boje teme samo su premjesteni, bez promjene. Sada dok je
A1 dijela uzet timeline slozene izjave pokazuje stupac kanala i razinu kao source timeline (`qnc-subsegment-edit`
`SubsegmentView.sound_choice` kroz `qnc_lane_choice::drafted_choice`, `qnc-subsegment-panel` ga predaje kao
`a1_channel`), a klik na kanal u stupcu ide dijelu (`source_choose_a1_channel` u `ROW_ACTIONS`). Kad je slot izabran
(klik na traku trajanja), A1 (klik na oznaku, desni klik na A1) uzima zvuk tog slota, ne dijela pod playheadom
(`pointer.rs`, test `a1_is_taken_of_the_chosen_slot`). Nije izvedeno: izbor vise slotova odjednom (trazi novu gestu
u zakljucanoj mapi misa), krivulja razine i pomocnik tona u slozenoj izjavi (nikad nisu bili izvedeni). Testovi
`qnc-subsegment-edit`, `qnc-subsegment-panel`, `qnc-script`, `qnc-panel-focus`, conformance. Uzivo nije provjereno.
Dopuna 2026-11-07 (korisnik: "treba se moci selektirati vise slotova"; gesta "povlacenje preko traka"; "ako se
selektira vise slotova, odnosi se na sve slotove, ako jedan, onda samo na taj odabrani"; mapa misa prosirena uz odluku
korisnika): na timelineu slozene izjave povlacenje lijevom tipkom koje pocne na traci trajanja bira sve slotove od te
trake do one pod misem (`qnc-timeline` `track_strip_drag`, samo red s uvijek vidljivim trakama; namjera
`TimelineIntent::DragStrip`, pokazivac `drag_strip` -> `timeline_select_slots` u `contracts/input/mouse.json` i
katalogu; `qnc-subsegment-edit` `SUBSEGMENT_SELECT_SLOTS`, `chosen_to`, `chosen_slots`). Izabrani slotovi su istaknuti.
A1 (klik na oznaku, Ctrl+1) uzet na jednom od izabranih vrijedi za sve: kanal (svakom unutar kanala njegova klipa),
razina i ukljucenost, svi se odmah cuju, Enter upisuje svaki (`sound_also`; svaki upis je svoj korak undoa); jedan
izabran slot: samo on. Klik na traku bira opet jedan slot, klik izvan traka i Esc puste izbor. Test
`a_drag_over_the_strips_chooses_the_slots_between`, `cargo check --workspace --all-targets`, conformance. Uzivo nije
provjereno.
Izvedeno 2026-11-07 (korisnik: "linija razine i AI pomocnik tona trebaju biti univerzalni bez obzira na formu";
odluke: slozena izjava i source timeline, a AI u slozenoj izjavi izjednacuje govor dijelova; korak 1): krivulja razine
A1 u slozenoj izjavi, isti alat kao na Wrap retku (`qnc-level-curve`). Dio pamti krivulju u scenariju
(`qnc-script-db` `Piece.level_keys`, parovi slika klipa i desetinke dB, zapis kroz `set_piece`), program je pusta
(`qnc-program-input` `PieceInput.level_keys` -> `a1_points`, `qnc-script-draft`), val je prati
(`subsegment_placements`). Uredivanje (`qnc-subsegment-edit` `level.rs`): Ctrl+Shift+1 ili desni klik na liniju
(`select_level_a1` u `ROW_ACTIONS`) uzima krivulju dijela (izabrani slot, inace dio u ruci), gore/dolje,
Ctrl+strelice i Ctrl+kotacic tocka na liniji playheada za 1 dB, Delete brise tocku, svaki korak se odmah cuje,
Enter upisuje, klik upisuje i stavlja playhead (krivulja ostaje uzeta, `LEVEL_KEEP_AT`), Esc odustaje. Crta je
`qnc-subsegment-panel` istom funkcijom kao Wrap retke (`qnc_trim_overlay::timeline_level_curves`). Test
`a_parts_level_curve_is_taken_and_raised_at_the_playhead_line`, testovi dotaknutih kocki, conformance. Uzivo nije
provjereno.
Izvedeno 2026-11-07 (isti zahtjev, korak 2): AI pomocnik tona u slozenoj izjavi (`qnc-subsegment-edit` `assist.rs`):
isti `qnc-sound-assist` kao Story nad dijelovima na A1 (nema pokrivalica: izjednacuje govor dijelova), glasnoca iz
odsjecaka spremljenih s valom (`ProgramWaves::clip_loudness`). Dok je slozena izjava otvorena, AI (Shift+N, dugme AI u
traci razine) ukljucuje i iskljucuje pomocnika za nju (izbor ovog procesa, kao NORM u previewu), a dugme pokazuje
njezino stanje (`qnc-side-pieces` `subsegment_assist`, `qnc-panel-focus`); dio s vlastitom krivuljom (ili uzetom)
ostaje svoj, ostali sviraju, crtaju i u valu imaju krivulju pomocnika (kroz nacrt dijelova, `drafted`). KEEP
(Ctrl+Shift+N) upisuje prijedlog kao krivulje dijelova (`set_piece`). Test
`ai_is_the_open_subsegments_and_fills_only_parts_without_a_curve`, conformance. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-11-07 (isti zahtjev, korak 3; korisnik: "da, otkljucaj player"): krivulja razine A1/A2 na
source timelineu svake forme (Story, MA, Ingest), isti alat (`qnc-level-curve`): Ctrl+Shift+1/2 ili desni klik na
liniju (mapa misa source_timeline: `right_click_level_a1/a2` -> `select_level_a1/a2`, uz odluku korisnika), tocka na
liniji playheada gore/dolje, Ctrl+strelice i Ctrl+kotacic za 1 dB, Delete brise tocku, svaki korak se odmah cuje, Enter
je pamti za klip, Esc odustaje (`qnc-source-timeline` `level.rs`; `qnc-panel-focus` salje tipke dok je uzeta).
Pamti se po klipu uz kanal i razinu A1/A2 (`qnc-clip-sound`: stupci `a1_keys`, `a2_keys`, tablici prvog oblika dodaju
se pri prvom upisu, citac starijeg pogleda cita bez krivulja), a novi segment (A1) i pokrivalica (A2, i Sync) tog
klipa je preuzimaju u istom koraku (`qnc-program-db` `CreateSegment`/`CreateCover` `level_keys`, `take_clip_curve`;
`qnc-program-segments` `SourcePick::with_curves`). Player (otkljucan): nova naredba ugovora `SetLaneCurve { output,
points }` (`qnc-player-contract`), klijent (`Action::SetLaneCurve`; `Action` vise nije `Copy`), runner, engine
`Runtime::set_lane_curve` (slike klipa u uzorke, `clip_clock`), zvucni uredaj `qnc-audio-output` `curve.rs` (krivulja
izlaza u pozivu uredaja, bez alokacije i brave, pravac u dB izmedju tocaka; test
`a_level_curve_is_a_line_in_db_on_its_output_only`); `qnc-source-preview` `hear_curves` salje krivulje i ponovno ih
salje svakoj novoj sesiji. Crta je `qnc-timeline` u audio traci source timelinea (istom funkcijom kao Wrap retke).
Nije izvedeno: val source timelinea jos ne prati krivulju. Testovi `the_clips_level_line_is_taken_raised_and_kept`,
`qnc-program-db` (pokrivalica preuzima krivulju klipa), testovi dotaknutih kocki, conformance. Uzivo nije provjereno.
Popravak 2026-11-07 (korisnik: "ne vidim liniju razine tona"; log: nijedno uzimanje linije): krivulja se crtala samo kad
ima tocke ili je uzeta, pa bez tocaka nije bilo linije koju bi desni klik uhvatio. Sada linija razine (ravna 0 dB bez
tocaka) stoji uvijek: na source timelineu za A1 i A2 prikazanog klipa (`qnc-source-timeline` `drawn_curves`), na Wrap
redovima za svaki segment i pokrivalicu (`qnc-program-view` `level_curves`) i za svaki dio slozene izjave
(`qnc-subsegment-edit`). Test prilagoden, conformance. Uzivo nije provjereno.
Izmjena 2026-11-07 (korisnik: "vodenje tona na uskoj audio traci nema nikakvog smisla, niti se vidi koliko je podignuto ili
spusteno; ima smisla jedino u expand audio u timeline ili u zumu"): krivulja razine se vise ne crta ni ne uzima na uskoj
traci A1/A2 (source timeline, pojasevi Wrap reda, slozena izjava). Crta se i uzima samo u prosirenoj traci (zoom trake preko
videa: uzeta traka Ctrl+1/Ctrl+2 ili klik na A1/A2, uvecani val) i u zoomu reza. Uzeta krivulja (Ctrl+Shift+1/2) sama
otvara prosirenu traku. U prosirenoj traci je ljestvica u dB (+12, +6, 0, -6, -12, -24, -40) i uz tocku na liniji
playheada razina u dB (i u zoomu reza); desni klik na liniju je uzima. `qnc-timeline` (`level_lane`, `zoomed_lane`,
`level_lines::paint_scale`). Testovi, conformance. Uzivo nije provjereno.
Izmjena 2026-11-07 (korisnik: "ako smo ukljucili expand audio kanala, klik lijevim misem vraca prikaz filmstripa; to
otezava upravljanje nivoima tona; izlaz treba biti ponovni klik na audio dugme ili ESC"): dok je prosirena traka (zoom trake
preko videa) otvorena, lijevi klik na timelineu samo postavlja playhead i nista ne sprema ni ne zatvara (`qnc-timeline`
`holding`); vrijedi za source timeline, Wrap redove i slozenu izjavu. Zamjenjuje pravilo da lijevi klik sprema uzetu traku
kao Enter (17. 10.) dok je zoom otvoren. Izlaz: klik na A1/A2 (ili Ctrl+1/Ctrl+2) uzete trake sprema i zatvara kao Enter
(`qnc-lane-choice` `lane_key`; uzeta krivulja razine `qnc-source-timeline` `level_key`; na Wrap retku klik na oznaku bez
slike `confirm`, `qnc-panel-focus`), Esc odustaje i zatvara. Na Wrap retku uzeta krivulja i dalje sprema klikom i ostaje
uzeta, a klik u zoomu slozene izjave isto. Test `the_taken_lane_taken_again_keeps_its_draft`, test slozene izjave
prilagoden (svaki dio ima liniju), conformance. Uzivo nije provjereno.
Popravak i dopuna 2026-11-07 (korisnik: "opet duplikati koda; timeline je svugdje isti, zoom koristi isti kod, a zoom nema
nivo tona"; "ne vidim sta radi AI tona"; log: AI ukljucen na source timelineu AI Media Assista, bez ikakvog ucinka):
(1) Zoom reza vise ne uzima krivulje sam iz price (source zoom ih uopce nije imao): `qnc-program-view` `ZoomSound.level_curves`
puni vlasnik osi istim krivuljama koje crta njegov timeline (prica `SegmentsView::level_curves`, source IN/OUT krivulje klipa
iz source timelinea i val ih prati, slozena izjava krivulje dijelova); `qnc-trim-overlay` crta samo njih. (2) AI tona na source
timelineu: za prikazani klip `qnc-sound-assist` `level_speech` (govor A1 unutar klipa na medijan svojih dijelova, rampe 200 ms,
+-10 dB, ispod 1,5 dB nista) i ducking A2 pod govorom A1 kad je A2 drugi kanal (`propose`); iz glasnoce izmjerene s valom
(`qnc-program-waveform`). Dok je ukljucen, traka bez vlastite krivulje svira i crta prijedlog (u prosirenoj traci i zoomu), a
status kaze sto je napravljeno (koliko mjesta A1, koliko puta A2) ili da nema promjene; KEEP (Ctrl+Shift+N) ga sprema kao
krivulje klipa. Ukljucenost je izbor ovog procesa. Kome AI pripada: otvorena slozena izjava njoj, panel Segmenti u fokusu
prici, inace klipu na source timelineu (`qnc-panel-focus` `source_assist`; u Storyju AI price sada trazi fokus na Segmentima).
`qnc-source-timeline` `assist.rs`, rjecnik `source.msg.assist_*`. Testovi (`the_speech_of_one_clip_is_evened_out_within_it`),
conformance. Uzivo nije provjereno. Usput (prekrsaj pravila): jedna zamjena teksta u `assist.rs` napravljena je PowerShell
`Replace`/`Set-Content` umjesto Edit alata.
Popravak 2026-11-07 (korisnik: "brisanje tocaka nivoa tona je nesigurno"; log: Delete na uzetoj krivulji 15 puta, playhead
klikom nikad tocno na slici tocke, pa se nista nije brisalo): Delete brise tocku na liniji playheada, a kad je tamo nema,
najblizu tocku krivulje (`qnc-level-curve` `nearest_point`, `remove_at`); vrijedi za source timeline, Wrap redove i slozenu
izjavu. Tocka koju bi Delete obrisao oznacena je kvadratom oko nje (`qnc-timeline` `paint_level_curves`, i u zoomu). Testovi
prilagodeni, conformance. Uzivo nije provjereno.
Popravak 2026-11-07 (korisnik: "pravilo slotova nije univerzalno; selektirani slot mora se moci obrisati i vratiti s Undo ili
Backspace"; log: S-Story, slot uzet desnim klikom ili Ctrl+S, Delete: "Ovaj slot nema pokrivalicu za uklanjanje"): u S-Storyju
je svaki klip slot, pa Delete na izabranom ili uzetom slotu (i uzetom rubu slota) bez pokrivalice izbacuje taj klip iz price
(`DeleteSegment`, jedan korak Undo; Backspace ga vraca). Slot s pokrivalicom i dalje gubi pokrivalicu. `qnc-program-segments`
`delete.rs` (`clip_of_slot`). U Storyju prazan slot (izmedu dva M markera) i dalje nema sto obrisati. Test
`in_an_s_story_a_chosen_slot_is_deleted_and_undo_brings_it_back`, conformance. Uzivo nije provjereno.
Izvedeno 2026-11-07 (korisnik: "idemo isprobati spajanje slozene izjave pomocu AI generiranja meduslika"; odluke: medjuslike
zamijene slike oko reza (kao Morph Cut, trajanje i ton ostaju), duljina je postavka po spoju, zadano 8 slika; motor
rife-ncnn-vulkan (MIT, nihui) jer gotovi RIFE ONNX dolazi iz projekta pod CC BY-NC-SA 4.0): korak 1, proba. Preuzet
rife-ncnn-vulkan 20221029 (Windows, GitHub nihui) u `C:\Users\miron\tools\rife-ncnn-vulkan` (program, vcomp140.dll,
LICENSE, modeli rife-v4 i rife-v4.6; ostalo obrisano; izvan gita). Nova javna kocka `crates/qnc-frame-interp` (+ ugovor
`contracts/modules/frame-interp.module.json`): `span` (koje slike oko reza zamijene medjuslike: polovica prije, polovica
poslije), `Interpolator::between` (n slika izmedju dviju PNG slika u zadanu mapu; program ovog racunala na decimalni broj
trazi zarez, pa se vremena zadaju brojem slika u nacinu mape, model se ucitava jednom). Primjer `examples/smooth_probe.rs`
(original samo citan, sve u zadanu mapu): na spoju slozene izjave aktivnog projekta (Mironik 3608, 5269 -> 5374, skok 2 s)
8 medjuslika 1080p za 9 s na Iris Xe, cut.mp4 i smooth.mp4 iste duljine i list prijelaza; slike ciste, malo zamucenja oko
ruke s brosurom. `qnc-script-db/examples/pieces.rs` (samo citanje dijelova izjava). Conformance PADA na C5 (kocka jos nema
korisnika) dok korak 2 ne spoji smooth spoj u slozenu izjavu, player i export. Nije izvedeno: nacin spoja Smooth i njegova
duljina u bazi, pozadinski render prijelaza, player i export.
Izvedeno 2026-11-07 (korisnik: prijepis "Kad je jedna tisuca ... sezdeset devet" gdje covjek kaze "tisucu devetsto sezdeset
devete"; "pricekat cemo noviju verziju Parakeeta, jedino sto bi mogli je ponovni prolaz s wav2vec2 odabranog dijela"):
izmjereno na tom dijelu (Mironik 3608 CH1): hrvatski wav2vec2 (ParlaSpeech-HR) cuje "kad je 1969. u zagrebu bila
promocija". Novi program-dodatak `tools/qnc-transcribe-wav2vec2` (ugovor `qnc.transcribe` 1, cisti Rust, procesor, ispod
normalnog prioriteta; prozori do 30 s na tihom mjestu, segment do stanke dulje od sekunde, prva rijec velikim slovom, bez
interpunkcije); `qnc-wav2vec2` `greedy_words` (rijeci s prvom i zadnjom slikom i vjerojatnoscu). AI katalog racunala:
motor `wav2vec2.parlaspeech-hr.cpu`, `transcribe.selected` (Best) pokazuje na njega (whisper ostaje u popisu). Nova radnja
`transcribe_repass` (`qnc-transcript` `repass`): IN/OUT prikazanog klipa u red prijepisa s motorom Best; samo za projekt s
hrvatskim jezikom govora; spremljeni prijepis raspona zamjenjuje stari na tom mjestu (postojece pravilo
`qnc-transcript-db`). Gumb "Ponovo (HR)" uz Prijepis ispod poola AI Media Assista (`pool_actions`), katalog, rjecnik.
Izmjereno kroz ugovor: 7 s govora za 5,9 s s ucitavanjem modela. Testovi, conformance (osim otvorenog C5 za
`qnc-frame-interp`). Uzivo nije provjereno.
Dopuna 2026-11-08 (korisnik: "s automatskim pokretanjem wav2vec2 za odabrani tekst"): kad dvoklik na dvije rijeci prijepisa
oznaci IN/OUT (odabrani tekst), taj dio odmah ide u ponovni prijepis hrvatskim modelom, kao gumb "Ponovo (HR)", u formi koja
ima prijepis (`qnc-source-timeline` `take_marked_by_text`, `qnc-panel-focus`). Rez skarama i I/O ga ne pokrecu. Uzivo nije
provjereno.
Izvedeno 2026-11-08 (pregled cijele aplikacije `docs/100-audit-2026-11-08.md`; korisnik: "da, kreni s prvim korakom"):
korak 1, gubitak rada i rusenje. (#3) Red prijepisa sprema prepisani dio u bazu projekta nakon svakog komada (prijepis
`[od, do kuda je cuo)` s oznakom motora "(part, still being transcribed)", `qnc-transcribe-run` `Progress::Heard.until`,
`qnc-transcribe-queue` `keep_part`); prekid ili pad vise ne baca rad: sljedeci rad istog zahtjeva nastavlja od kraja
spremljenog dijela i na kraju sprema jedan prijepis cijelog raspona (dio unutar njega se brise postojecim pravilom).
(#4) Stanje reda vrijedi samo za raspon i motor koji je radio (`qnc-transcript-db` `SetQueued.asked`, `set_queued` prima
zapis reda): klip zatrazen ponovno dok stari radi ostaje cekati, kraj starog rada ga vise ne oznaci gotovim. (#14) Slozena
izjava pusta uzeto (rub, slot, ton, J/L, slip, krivulja) kad drugi pisac skrati izjavu ispod uzetog dijela
(`qnc-subsegment-edit` `forget_parts_gone`), indeksi dijelova citaju se bez pada. (#15) Roll, pomak slota, ton vise
slotova i KEEP AI-ja su jedan upis i jedan korak undoa: nova operacija baze scenarija `Together` (samo promjene dijelova;
jedna odbijena ne upise nijednu; `qnc-script-db` `together`). (#16) Upisi scenarija i prijepisa (red, izjave) dok jedan
putuje idu redom, svaki sa svojim odgovorom (`qnc-script` `queued`/`write_next`, `qnc-transcript` `ask`/`ask_next`).
(#17) Undo scenarija vraca i korak snimljen prije novih stupaca (upisuje samo stupce koje korak ima, ostali dobivaju
zadanu vrijednost; `qnc-script-db` `undo.rs`). (#18) Nova verzija S-Storyja dobiva broj veci i od svake datoteke
`story-draft-<n>.db` uz projekt (`qnc-db-broker` `highest_draft`). Testovi `qnc-script-db` (2 nova), `qnc-transcript-db`
(novi), `qnc-skeleton-db` (novi), dotaknute kocke, `cargo check --workspace --all-targets`, conformance (osim otvorenog
C5 `qnc-frame-interp`). Uzivo nije provjereno.
Dopuna 2026-11-08 (korisnik: "omoguciti prikaz prijepisa bez cekanja kraja prepisa"; log: prijepis Mironik 3608 trajao je
165 s, a klip je otvoren tek nakon njega): klip koji se prepisuje je u redu panela prijepisa poveznica kao i gotov
(`qnc-transcript-panel`); klik ga otvara i pokazuje retke kako nastaju. Spremljeni dijelovi (korak 1, #3) stavljaju klip u
tab Prijepis vec nakon prvog komada, a druge forme ga citaju iz baze. Uzivo nije provjereno.
Popravak i izmjena 2026-11-08 (korisnik: "kada u tekstu koristim IN/OUT, na OUT postavlja rez prije selektirane rijeci, a
trebao bi na kraju"; "kad kreiram novi projekt fokus po defaultu na prvo polje, svaki Enter skace na novi red, na kraju
Enter vodi na dugme Kreiraj projekt, strelicama Odustani - Kreiraj"; "ponovni prijepis pocinje tek kad korisnik klikne
Dodaj izjavu ili Dodaj u slozenu izjavu"; Project otkljucan samo za to): (1) Klik na rijec prijepisa pamti rijec; dok je
playhead jos na njezinoj prvoj slici, O stavlja OUT iza rijeci (`qnc-source-panel` `chose_word`, `word_at_playhead`;
`qnc-timeline-model` `with_source_out_at`); isto u tekstu izjave scenarija za dio slozene izjave
(`qnc-subsegment-edit` `word`). (2) Obrazac novog projekta (`qnc-newsroom-sheet`): kad se otvori, naziv projekta ima
tipkovnicu; Enter ide na sljedece polje za upis (izbori se preskacu); Enter u zadnjem polju (`qnc-settings-form`
`ColumnAnswer.past_last`) stavlja tipkovnicu na Kreiraj (okvir fokusa), strelice lijevo/desno izmedju Odustani i
Kreiraj (nove radnje kataloga `form_button_prev`, `form_button_next`, QNC preset, opseg `project`), Enter pritisne gumb;
klik u polje vraca tipkovnicu polju. `show` dobiva katalog tipki (Project ploca i program obrasca vise ne citaju Enter
sami; C10 Project ploce 1142 -> 1141). (3) Dvoklik na rijeci vise ne pokrece ponovni prijepis odmah: tekst oznacen
rijecima ide hrvatskom modelu tek na Dodaj izjavu (`save_virtual_shot`), Slozenu izjavu (`script_subsegment`) ili
+ IZJAVA (`script_add_clip`) (`qnc-panel-focus`). Testovi dotaknutih kocki, conformance (osim otvorenog C5
`qnc-frame-interp`). Uzivo nije provjereno. Project je ponovno zamrznut.
Izmjena 2026-11-08 (korisnik: "odabrana izjava bi se trebala ponovo transcriptirati u pozadini i ne bi se smjela
osvjezavati na trenutnoj formi, samo zapisana u bazu; tek kad korisnik prijede na sljedecu formu ili ponovo otvori
trenutnu (istina je u bazi)"): klip poslan na ponovni prijepis forma zadrzava kakav ga pokazuje (`qnc-clip-transcripts`
`hold`, `let_go`, `is_held`; postavlja ga `qnc-side-pieces` kad prode `transcribe_repass`): novi prijepis ide samo u bazu,
panel prijepisa ne pokazuje retke ponovnog prijepisa dok traje, a tekst izjava u scenariju se ne mijenja. Kad forma
nema klip na previewu (napustena ili ponovno otvorena) cita bazu i pokazuje novi prijepis; druge forme ga citaju odmah.
Klip koji vise nije trazen (drugi klip u AI Media Assistu) takoder se pusti. Ponovni prijepis vise ne otvara tab
Prijepis kad zavrsi (`qnc-transcript`: samo kad je zavrsio prvi prijepis, Fast). Test
`a_held_clip_keeps_what_it_shows_until_it_is_let_go`, conformance (osim otvorenog C5 `qnc-frame-interp`). Uzivo nije
provjereno.
Popravak 2026-11-08 (korisnik: "i dalje osvjezava formu prilikom zapisa u bazu"; log: AI Media Assist, Dodaj izjavu
3879..4163, ponovni prijepis gotov za 21 s): red ponovnog prijepisa pojavljivao se i nestajao u popisu reda iznad teksta
prijepisa (i poruka "queued"), pa se tekst pomicao. Ponovni prijepisi (Best) nisu u popisu reda panela i ne pisu poruku
(`qnc-transcript`); u bazi i u logu su kao i prije. Uzivo nije provjereno.
Popravak 2026-11-08 (korisnik: "ponovljeni prijepis nije ispravio greske, a ono sto je ispravio je duplicirao"; baza
aktivnog projekta, samo citanje: Mironik 3609 stari redak 992..1168 "Ovo je ovo je prije brkova, a onda..." ostao je cijeli
uz novi "O je ovo je prije brkova" 997..1048, jer je stari redak brisan samo kad mu je sredina u novom rasponu): (1)
noviji prijepis uzima starom prijepisu rijeci po rijec: rijec pripada gdje joj je sredina, redak koji prelazi novi raspon
zadrzava samo svoje rijeci izvan njega (jedan redak prije i jedan poslije), tekst su te rijeci (`qnc-transcript-db`
`lines_cut`, `save`). (2) Ponovni prijepis u pozadini slusa sekundu prije i poslije raspona (unutar spremljenih slika
klipa) i zadrzava samo rijeci cija je sredina u rasponu, istim pravilom (`qnc-transcribe-run` `padded`), pa prva i zadnja
rijec nisu odrezane ("O je" umjesto "Ovo je"). Greske koje hrvatski model sam cuje krivo ("eksttih") ovo ne ispravlja.
Postojeci dvostruki redci ostaju dok se ta izjava ponovno ne prepise. Test
`a_newer_transcript_takes_the_words_of_an_old_line_that_runs_past_it`. Uzivo nije provjereno.
Popravak 2026-11-08 (korisnik: "jos uvijek isti problem, ponavljanje... ovaj problem smo vec bili rijesili, ali ne
postujes pravila da koristis postojece kocke"; log: scenarij, izjava blok 2 dio 992..1072 klipa Mironik 3609): redci
upisani prije pravila po rijecima ostali su u bazi i tekst izjave (`qnc-script` `timed_words`) ih je citao dvaput.
Pravilo "noviji prijepis vrijedi preko svog raspona" sada vrijedi i pri citanju, na jednom mjestu: jedini citac
`qnc-transcript-db` `read_clip` reze starije prijepise po rijecima rasponom svakog novijeg (redoslijed `made_at_utc`,
ista `lines_cut` kao pri upisu), pa ga dobivaju panel prijepisa, scenarij i rijeci zooma bez ijedne izmjene kod njih.
Provjereno citanjem aktivnog projekta: Mironik 3609 vise nema "ovo je" dvaput. Test (stari redak upisan cijeli cita se
bez ponavljanja). Uzivo nije provjereno.
Izmjena 2026-11-08 (korisnik: "opet si napustio kocku za obrazac za kreiranje novog projekta"; "kad kliknem na novi
projekt obrazac mora biti s fokusom na prvi zapis Obrazac; korisnik ne treba klikati misem, sve ide kroz tipkovnicu, a
prelaz iz reda u red obrasca ide s Enter"): zamjenjuje prethodni zapis o obrascu (fokus na nazivu, vlastita logika
gumba). Obrazac je cijeli na tipkovnici: kad se otvori, tipkovnica je na prvom redu (Obrazac); Enter ide red po red
(Obrazac, naziv projekta, svako polje lista, pa gumbi); na redu s izborom (Obrazac, polje izbora) strelice biraju
izbor, na redu za upis tipkovnica je u polju; zadnji Enter stavlja tipkovnicu na Kreiraj, strelice Odustani | Kreiraj,
Enter pritisne. Izbor na redu i gumbi idu kroz postojecu kocku reda tipki `qnc-key-row` (kao red dugmadi u Ingestu i
Storyju), strelice su iste radnje kataloga kao na svakom redu (`step_back_frame`, `step_forward_frame` dodane u opseg
`project`; radnje `form_button_prev/next` uklonjene). Hod po poljima je u kocki obrasca (`qnc-settings-form`
`ColumnKeys`, `is_chosen`, `field_column`), obrazac priloga (`qnc-newsroom-sheet`) vodi samo na kojem je redu. Klik u
polje i dalje stavlja tipkovnicu na taj red. Testovi, conformance (osim otvorenog C5 `qnc-frame-interp`). Uzivo nije
provjereno.
Popravak 2026-11-08 (korisnik: "gubi se prikaz fokusa i kursora"): Project ploca nije znala da je obrazac otvoren, pa je
Enter na redu s izborom (kad se ne pise u polje) otvarao i projekt odabran u popisu ispod obrasca; sada dok je obrazac
otvoren Enter ne otvara projekt (`qnc-project-blocks`, isti redak). Okvir reda s izborom je boje fokusa teme, ne
prigusene boje oznake (`qnc-settings-form` `ColumnKeys.focus`). Uzivo nije provjereno.
Popravak 2026-11-08 (korisnik: "okvir fokusa se vidi samo na poljima s izbornikom, ne i na edit poljima"; "ni kursor se
nije vidio"): okvir reda na kojem je tipkovnica crta se i na redovima za upis (naziv projekta, polja bez izbora). Naziv
projekta dobivao je tipkovnicu prije nego je nacrtan, pa je isti Enter koji ga je doveo obradilo i samo polje i pustilo
tipkovnicu (kursor nestao); sada je dobiva nakon crtanja, kao i polja lista (`qnc-newsroom-sheet`). Uzivo nije
provjereno.
Ispravak 2026-11-08 (korisnik, snimka obrasca: "nepotrebno crtas dva fokus okvira, nepotreban je okvir na nazivu kolone;
nepotrebna je zelena boja dugmeta Kreiraj projekt, moze ostati zeleno jedino ako je trenutno u fokusu, inace bude kao
dugme Odustani"): red za upis vise nema okvir oko cijelog reda (svoj okvir i kursor pokazuje polje); red s izborom ima
okvir samo oko izbornika, ne oko naziva (`qnc-settings-form`, `qnc-newsroom-sheet`). Gumb na kojem je tipkovnica je
zelen, drugi je obican (`sheet_button`); Kreiraj projekt je zelen samo dok je u fokusu. Uzivo nije provjereno.
Popravak 2026-11-08 (korisnik: "enter vodi na novi red, ali ne prikazuje fokus okvir nigdje osim polja koja su
dropdown"): vlastiti okvir polja za upis u temi se ne vidi, pa i red za upis dobiva okvir boje fokusa, samo oko polja
(ne oko naziva), isti kao izbornik: naziv projekta i polja lista (`qnc-newsroom-sheet`, `qnc-settings-form`). Jedan
okvir po redu. Uzivo nije provjereno.
Otkljucano i izvedeno 2026-11-08 (korisnik: "da, otkljucaj i kreni sa smooth spojem"; odluke od 7. 11.: medjuslike zamijene
slike oko reza, trajanje i ton ostaju, duljina po spoju, zadano 8): Smooth spoj slozene izjave, korak 2. Spoj dobiva
`qnc-script-db` `Piece.smooth_frames` (operacija `SetSmooth`, korak undoa; pripada spoju kao pauza) i `smooth_clip`
(`SetSmoothClip`, bez koraka undoa: to je napravila pozadina). `qnc-script-draft` `smooth_of`: id klipa medjuslika iz dva
klipa, slika reza i broja (`qnc-frame-interp` `clip_id`); bez smootha uz pauzu, J/L slike ili prekratak dio; smooth svira samo
dok je `smooth_clip` klip spoja kakav je sada (pomaknut rub: obican rez dok se ne napravi novi). Nova javna kocka
`crates/qnc-smooth-joins` (+ ugovor): pozadina kroz `qnc-script` poll, posao krece samo dok player ne svira; dvije slike iz
originala kroz katalog dekodera (bez probea, kartica samo citana), medjuslike `qnc-frame-interp` (AI katalog `smooth`:
`qnc-ai-catalog` `installed_frame_interpolator`), kratki MXF u mapi glasa projekta (`qnc-voice-clip` `sequence`, jedan tihi
kanal) i klip kataloga kroz `qnc-voice-catalog` (pool ga ne pokazuje). `qnc-program-input`: medjuslike prepisuju sliku oko reza
kao pokrivalica bez tona, pola prije i pola poslije reza (`PieceInput.smooth`), pa ih player i Draft XML (V2) dobivaju bez
vlastitog koda; samo dijelovi u stopi izjave. Panel (`qnc-subsegment-panel`): uz Rez/Pretapanje dugme Smooth, - i + (po 2, 2-24
slike) i stanje (izraduje se, spremno, ne na ovom spoju); radnja kataloga `subsegment_join_smooth` bez tipke. Program i model
izvan gita: `target/release/rife-ncnn-vulkan.exe` + `vcomp140.dll`, `data/models/rife-v4.6` (MIT, uci u beta paket). Nije
izvedeno: posao se ne prekida kad player krene (10-25 s, dijeli graficku), Send to S-Story ne prenosi smooth u pricu, izvor
isprepletene slike se odbija, ulazne slike su JPEG (q 3). Provjereno: testovi `qnc-smooth-joins`, `qnc-script-db` i
dotaknutih kocki, conformance (C5 `qnc-frame-interp` rijesen), model cita JPEG ulaz, klip iz niza slika ima tocan broj slika.
Uzivo nije provjereno.
Popravak 2026-11-08 (korisnik: "ne vidim nikakve nove gumbe"): redci dijelova slozene izjave crtali su se ispod timelinea, a
dock ima mjesta samo za timeline, pa ih nikad nije bilo vidjeti (ni pauzu, spoj i A1 dijela). Sada jedan redak pluta odmah iznad
timelinea (`qnc-subsegment-panel`): za izabrani ili uzeti dio, inace za dio iza spoja najblizeg playheadu; prvi dio klikom na
njegovu traku. Layout nije diran. Uzivo nije provjereno.
Izmjena 2026-11-08 (korisnik: redak "pokrio je prikaz razine tona"; izabrao rjesenje A): zamjenjuje plutajuci redak. Dok je
slozena izjava otvorena, njezine kontrole su u njezinu tekstu (`qnc-script-panel` `part_controls.rs`; isti blok u MA Audiju i MA
Scriptu): u redu dijela A1 (kanal, razina, uklj.), mjesto i Obrisi, a red izmedju dva dijela je spoj (pauza, Rez/Pretapanje,
Smooth). Iste radnje kao prije; `qnc-subsegment-panel` crta samo timeline i zoom. Uzivo nije provjereno.
Izmjena 2026-11-08 (korisnik: "suma podataka, nepregledno, zbunjujuce... treba odvojiti tonove, oni su vec odabrani"): u tekstu
izjave nema tona (A1 dijela mijenja se na timelineu, Ctrl+1 ili klik na A1). Izmedju dva dijela stoje samo Rez/Pretapanje i Smooth
(stanje samo dok nije spremno); pauza, brojevi -/+, mjesto i Obrisi samo uz dio izabran klikom na traku timelinea. Dio s A1 radnje
ostaju u katalogu (`subsegment_part_channel` i dr.) bez gumba. Uzivo nije provjereno.
Izmjena isti dan (korisnik: "ostaviti samo opciju dodavanja razmaka"): u tekstu otvorene izjave jedina kontrola je pauza izmedju
dva dijela (-/+). Rez/Pretapanje, Smooth, mjesto i Obrisi dijela nemaju gumb (radnje ostaju u katalogu, spremljeni smooth i dalje
svira); gdje idu ceka odluku korisnika. Uzivo nije provjereno.
Dopuna isti dan (korisnik: "naziv klipa je nepotreban u prikazu... baza neka sadrzi sve podatke, ali za Smut treba samo
neophodno"): dio slozene izjave u tekstu pokazuje samo TC i duljinu (naziv ostaje u bazi); uz pauzu je jedan gumb Smooth
(ukljuceno ili ne, zadano 8 slika), stanje samo dok nije spremno. Uzivo nije provjereno.
Izmjena isti dan (korisnik: tabelarni prikaz, "ljeva kolona timecode, srednja tekst izjave, krajna desna dugme smooth i
podesavanje razmaka"; IN/OUT se pise samo u dokumentu za provjeru): dijelovi slozene izjave u scenariju su redovi tablice
(`qnc-script-panel`): TC pocetak-kraj i duljina, tekst, a desno (dok je izjava otvorena) Smooth i pauza spoja prije tog dijela.
Red "IN: ... OUT: ..." bloka vise se ne prikazuje u formi; u izvozu ostaje. Uzivo nije provjereno.
Dopuna isti dan (korisnik: "zasto se ne cuje ton"; A1 sva tri dijela bio je iskljucen; "ukljuci A1 u bazi i dodaj crvenu oznaku"):
dio slozene izjave s iskljucenim A1 ima crvenu oznaku A1 na pocetku svoje trake na timelineu (`qnc-timeline`
`TimelineCoverSpan.a1_off`, boja `lane_silent`). A1 ukljucen u aktivnom projektu primjerom `qnc-script-db/examples/a1_on.rs`
kroz pisca scenarija (na zahtjev korisnika). Uzivo nije provjereno.
Popravak isti dan (korisnik: "ne radi tekstualno editiranje izjave"; log: Delete i Backspace isli su scenariju, nijedan nacrt
teksta): ista izjava crta se u scenariju i preko njega u prozoru medija s istim egui id-em, pa polje sprijeda nije dobivalo klik ni
tipkovnicu. Svaki prikaz ima svoj id (`qnc-script-panel` `paint_block(.., place, ..)`). Uzivo nije provjereno.
Popravak isti dan (korisnik: "iz rijeci pa izbrisao slovo p, ostavio a; reproducira se uredno, ali u tekstu se ne prikazuje a"):
rijec se u tekstu dijela birala po svojoj sredini, pa je ostatak rijeci nestajao. Rijec s poravnatim slovima sada pokazuje slova
koja dio jos izgovara (`qnc-script` `timed_words`; pojedino slovo po svojoj sredini, znak bez vremena ide s prethodnim). Test.
Izmjena isti dan (korisnik: "u ljevoj koloni sve timecode stavi u jedan red"): lijeva kolona tablice izjave u jednom je redu
(pocetak - kraj, duljina), kolona siroka 310.
Dopuna isti dan (korisnik: "cini se da ne radi snimanje offa"; snimke postoje, ali ulaz racunala daje tisinu, -91 dB i izvan QNC-a;
"da, dodaj upozorenje"): snimka ciji je najglasniji uzorak ispod -60 dBFS ostaje zapisana, a poruka scenarija kaze da je tiha
(`qnc-voice-record` `Recording.silent`, `qnc-voice-take`, `qnc-script`, rjecnik `script.take_silent`). Uzivo nije provjereno.

Izvedeno 2026-11-08 (korisnik: "dok korisnik cita off wav2vec2 transkodira u tekst, tako se ujedno kontrolira je li ispravno procitano"; plan u 4 koraka, "da, kreni s korakom 1"): korak 1, provjera procitanog nakon Stop. Nova cista kocka `crates/qnc-read-check` (+ ugovor): napisani tekst prema cutim rijecima (procitano, krivo, preskoceno, izgovoreno izvan teksta, postotak; sitna razlika motora je procitano, broj u znamenkama pokriva rijeci broja i obrnuto). Nova kocka `crates/qnc-take-heard` (+ ugovor): take bez cutih rijeci u pozadini, jedan po jedan, samo dok player miruje i nista se ne snima, kroz Best motor racunala; rijeci sa sekundama u takeu zapisuju se uz take (`qnc-script-db` `script_voice_takes.heard_json`, operacija `SetTakeHeard`, bez koraka undoa; starijoj tablici stupac se dodaje, takeovi ostaju). Postuje postavke projekta (prijepis ukljucen, jezik hr), inace panel kaze zasto. `qnc-voice-record` `read_take`, `qnc-transcribe-run` `hear_samples` (zajednicki `run_engine`). Panel scenarija (`qnc-script-panel` `read_marks.rs`; MA Audio i MA Script): tekst OFF-a prema izabranom takeu zeleno, krivo crveno, preskoceno crveno precrtano, pod misem sto je cuto, ispod izgovoreno izvan teksta; na T1..Tn postotak, ".." dok se slusa. Usporedba je s tekstom kakav sada jest. Nije izvedeno: rad uzivo tijekom citanja, ponovljena citanja i slozeni OFF, M markeri u tekstu. Testovi `qnc-read-check` (7), `qnc-take-heard`, `qnc-voice-record`, `qnc-script-db`, `qnc-script`, conformance. Prag slicnosti je prva procjena. Uzivo i s pravim motorom nije provjereno.

Popravak 2026-11-08 (korisnik: "klik na snimku offa trebao bi plejati off"): gumb T1..Tn pusta snimku i kad je to vec izabrana snimka (prije je klik na izabranu snimku bio bez ucinka; `qnc-script-panel`). Uzivo nije provjereno.

Popravak 2026-11-08 (log: klik na take istog klipa: `Play` odbijen "NotReady" jer je poslan iza cuea; `SetOutputRoutes [4,4]` odbijen "outside the decoded channels" jer je klip otvoren prije novog takea): take vec otvorenog klipa pusta se od pocetka nakon cuea (`play_from`), a zapis s novim takeom priprema se iznova (`qnc-source-preview` `open_again`, `qnc-script` `heard_takes`, `qnc-panel-focus`). Svaki take slusa jedna forma (`qnc-take-heard`, u procesu; prije su MA Audio i MA Script isti take prepisivali istodobno). Uzivo nije provjereno.

Izvedeno 2026-11-08 (korisnik: "postavljanje M Markera za montazu pokrivalica u tekstualnom dijelu offa"; "da, kreni s markerima"): korak 4 plana, prije rada uzivo i ponovljenih citanja. Desni klik (Ctrl+) izmedu dvije rijeci teksta snimljene vrste stavlja M marker, na postojecem ga uzima, Delete brise uzeti, Esc pusta (`qnc-script` `text_marks.rs`, akcija kataloga `script_mark_at` bez tipke; `qnc-script-panel` `read_marks.rs`: oznaka `|` u tekstu, ispod trajanja kadrova). Marker je mjesto u tekstu: broj rijeci ispred koje stoji (`qnc-script-db` `script_blocks.marks_json`, operacija `SetMarks`, korak undoa; pri izmjeni teksta ostaje uz svoju rijec, s obrisanom rijeci nestaje). Vrijeme: rijec spojena s cutom rijeci izabranog takea, inace sljedeca, inace udio u rijecima (`qnc-read-check` `CheckedWord.heard`, `word_second`; `qnc-script-draft` `mark_seconds`, `SkeletonPart.marks`). Put do price: S-Story ih cuva uz segment (`story_items.text_marks`, slike klipa, ne dira pravilo "klip je slot"), "Send to Story" ih pretvara u M markere i prazne slotove (`qnc-program-db` `AppendSegment.marks`, `append_segments`). Nije izvedeno: Ctrl+M u tekstu (tekst nema kursor), boja trajanja po duljini, povlacenje markera, S-Story ih ne pokazuje. Testovi `qnc-read-check`, `qnc-script-db`, `qnc-script-draft`, `qnc-program-db`, `qnc-script-panel`, conformance. Uzivo nije provjereno.

Popravak 2026-11-08 (korisnik: "zasto u Story nema teksta u zoomu"; baza: zoom je klip glasa prepisao kanalom govora projekta (CH1 = T1, tihi), a svira T5, pa je prijepis bez redaka): rijeci u zoomu reza nad klipom glasa su rijeci cute u takeu koji A1 pusta (`script_voice_takes.heard_json`), citane iz baze (`qnc-zoom-words` `voice_words`); klip glasa se vise ne prepisuje za zoom. Izjave kao prije. Uzivo nije provjereno.

Izmjena 2026-11-08 (audit zooma; korisnik: "vise puta sam naglasio da je zoom samo uvecani dio timelinea"; "da, kreni s 1, pa s 2"): zoom je jedan put. Zvuk zooma slagao se na 4 mjesta (source IN/OUT, rub klipa u S-Storyju, rez price, slozena izjava); sada svaki vlasnik daje samo svoju os (`qnc-program-view` `ZoomAxis`: rez, sto prozor drzi, duljina, pozicija playera na toj osi, trake), a prozor, playhead i zvuk u dosegu racuna `ZoomAxis::sound`. Pozicija je obvezno polje, pa zoom koji ne prati player vise ne moze nastati previdom. Slike oko reza imaju jedan oblik (`CutPictures`) u `edit_stills`, `trim_stills`, `point_stills`, `edge_stills`, `source_stills`, `subsegment_zoom`. Popravak iz loga: zoom ruba klipa u S-Storyju nije dobivao poziciju playera (`edit.rs` `playhead(cut, None)`, od 1. 11.), pa zvuk nije tekao; sada prati player dok svira taj klip, na osi klipa. Ponasanje ostalih zoomova isto. Nije izvedeno (korak 3 audita): rub u Storyju na `qnc-edit-edge`, `sound_key` slozene izjave na zajednicke tipke. Uzivo nije provjereno.
Izvedeno 2026-11-09 (korisnik: "Prijepis, izjave, slozene izjave, na svojim posterima trebaju kad su selektirane imati u gornjem desnom kutu overlay poluprozirno dugme X za brisanje"): izabrana kartica tabova Izjave / SubClip (`short_remove`), Prijepis (`transcript_forget_clip`) i Slozene izjave (`script_remove`) ima poluprozirno X gore desno na posteru, crveno pod misem; radnju po tabu daje `qnc-pool-tabs` `remove_action`, crta i hvata `qnc-media-card` (`MediaCardFeatures.remove_chosen`, `CardGridAction::Remove`, `qnc-media-pool-panel` `show_removable`). Izjava: `qnc-virtual-shots` `PoolShotsReader::remove` (brise i cita u pozadini, nikad B-roll kadar). Prijepis: `qnc-transcript-db` operacija `ForgetClip` (svi prijepisi klipa i njegov red; odbijeno dok se klip prepisuje; klip i izjave ostaju). Slozena izjava: postojece uklanjanje bloka scenarija (ima Undo); izjava i prijepis nemaju Undo. Slaganje specifikacije poola premjesteno je iz ploce u `qnc-editorial-layout` `pool_spec`, a provjera otvaranja izjave iz aplikacije u `qnc-virtual-short-cards` `to_open`. Ingest nema X. Uzivo nije provjereno.
Izvedeno 2026-11-09 (korisnik: "kad se klikne na izjavu u source timeline bi se trebao prikazati zum izjave na 70% sirine, a ispred i iza izjave po 15%, kao zum u S-Story i Story"): izabrana izjava (ne B-roll) dok je njezin klip na previewu pokazuje se na source timelineu kao prozor klipa oko svog spremljenog IN/OUT-a, 70 % i po 15 % sa strane; isti timeline, iste radnje, sve slike su slike klipa. Pravilo 70/15 je jedno: `qnc-timeline-model` `enlarged` (i zoom slota u `qnc-program-segments`). `qnc-source-timeline-view` `show_with` prima oznake i crta sa suzenom projekcijom; `qnc-timeline-assets` `SourceTimelineAssets::window` daje dio vala i za svako mjesto filmstripa najblizu spremljenu sliku (kratka izjava ima malo razlicitih slika, nista se ne dekodira); `qnc-virtual-short-cards` `shown_marks`. Klik na klip u poolu vraca cijeli klip. Ingest nema izjave. Uzivo nije provjereno.
