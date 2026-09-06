# QNC: dubinski audit radnog stabla i oba Project oblika

Datum: 2026-09-06. Root: `C:\Users\miron\Projects\QNC`.
Git HEAD: `82aef9ab6a7eefe3743455266b377af482fbc92d`.
Audit obuhvaca i postojece necommitane promjene, ne samo HEAD ili ranije izvjestaje.
Mjerilo je trenutni [AGENTS.md](C:/Users/miron/Projects/QNC/AGENTS.md), uz posljednju korisnicku uputu: navigacijski okidac se salje shellu, ne zapisuje se u bazu.

## 1. Nalazi

P1 oznacava gresku ili nedovrseni ugovor koji treba zatvoriti prije oslanjanja na taj workflow. P2 oznacava ogranicenje, rizik ili djelomicnu uskladenost. Uz svaki nalaz razlikuju se izvrsena reprodukcija i zakljucak iz koda. Nedovrseni, eksplicitno odgodeni media moduli nisu prikazani kao novootkrivene regresije.

### F01 / P1: neuspjelo kreiranje ostavlja projekt u registriju

[ProjectStore::create_project](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:315) prvo trajno upisuje `projects` i `project_storage_locations`, a tek potom priprema export, projektnu bazu i zastitu direktorija. Nema rollbacka objavljenih zapisa ako kasniji korak vrati gresku. Aktivni projekt moze biti upisan prije nego zavrsno zakljucavanje parent direktorija uspije.

Reprodukcija: `export.directory = ../invalid` daje gresku, ali `list_projects()` vraca jedan projekt. Nije potrebna simulacija pada diska ili procesa. Oba Project oblika koriste ovaj isti put.

Posljedica: UI javlja neuspjeh, a sljedece ucitavanje pronalazi djelomicno kreiran projekt. Potrebno je definirati trenutak uspjesne objave, validirati prije objave i pokriti neuspjehe ciljanom provjerom. To nije zahtjev za migraciju ili popravljanje postojecih projekata.

### F02 / P1: rucno upisana lokacija projekta nije lokacija stvarnog kreiranja

[Project path input](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:1200) mijenja `draft_settings.storage.projects_root`. [Store](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:345) ipak kreira direktorij iz svojeg `self.projects_root`. Browser potvrda poziva setter storea, dok rucni unos nema isti ucinak.

Reprodukcija kroz javni owner API, s istim settings podatkom koji salje forma: projektna baza nastaje na prethodnoj/default lokaciji, a trazeni direktorij ne nastaje. Oba oblika su zahvacena. Rucni unos i browser izbor moraju zavrsiti istom komponentnom naredbom, bez lokalnog UI zaobilaznog popravka.

### F03 / P1: postoje oba oblika Projectsa, ali nijedan nema navigacijski izlaz

[Kreiranje](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:1513) i [otvaranje](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:1830) zavrsavaju promjenom statusa i popisa. Ne emitiraju poruku hostu. [ShellDesktopApp](C:/Users/miron/Projects/QNC/crates/qnc-shell-desktop-api/src/lib.rs:5) ima samo `show_desktop(...) -> ()`; nema izlazni dogadjaj ni opcionalni navigacijski prikljucak.

[Shell aktivacija](C:/Users/miron/Projects/QNC/apps/qnc-app/src/main.rs:319) postoji za rucni izbor taba. Nema prijema okidaca ni povezivanja s iducom odabranom grupom. Ovo je nedovrseni trazeni korak, a ne dokaz da hosted Project ne postoji.

Ispravan smjer je kratkotrajni navigacijski signal nakon uspjeha, putem javnog adaptera. Shell njime aktivira isti precevni/navigacijski put koji koristi korisnikov klik. Ne uvoditi DB trigger, tablicu dogadjaja, polling red poruka ili poslovni servis. Samostalni Project mora nastaviti raditi bez shella.

### F04 / P1: shell koristi drugi popis dostupnosti i drugi redoslijed

[AppRegistry::load](C:/Users/miron/Projects/QNC/apps/qnc-app/src/main.rs:200) izravno cita `apps/*/qnc-app.json`. [AppManifest](C:/Users/miron/Projects/QNC/apps/qnc-app/src/main.rs:123) nema `priority_group`; sortiranje koristi numericki `order`. Provjera executablea trazi samo neprazan tekst, ne postojanje datoteke.

Project vec cita proizvedeni katalog dostupnih aplikacija i zapisuje odabir po grupama a-z. Shell ne cita taj odabrani slijed. Zato postoje dva tumacenja dostupnosti/redoslijeda. Novo pravilo templatea jos nije provedeno u automatizaciji shella. Stari numericki footer je trenutno dopusteno odstupanje, ali ne smije postati algoritam za sljedecu grupu.

Dodatno, [activate_tab](C:/Users/miron/Projects/QNC/apps/qnc-app/src/main.rs:325) postavlja aktivni tab prije uspjesnog stvaranja adaptera. Manifest bez dostupnog adaptera ili s `external_component` nacinom hostanja moze zamijeniti radni prikaz placeholderom.

### F05 / P1: alternativa u grupi a dobiva pogresan pocetni korak

[write_project_workflow](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:1494) bira prvi `tab_id != "project"`, a status dovrsene faze takodjer prepoznaje po literalnom nazivu `project`. Pravilo grupe a nije primijenjeno.

Reprodukcija: valjani slijed `alternative/a, ingest/b` zapisuje `entry_step_id = step_alternative`, umjesto `step_ingest`. Sam sortirani popis aplikacija jest ispravan. Greska je u izvedenom entry/status podatku koji bi shell mogao upotrijebiti. Ne popravljati imenovanjem svake poznate varijante u novom switchu.

### F06 / P1: radne postavke postoje u bazi, ali javni ugovor ih ne isporucuje

[public_project_settings](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:1231) iznosi samo `project_id`, `template_id`, `created_at`, `updated_at`. Ne iznosi `settings_json` ili tipizirane radne postavke. Javni snapshot takodjer ne iznosi snapshot sadrzaj. Tablica `project_settings_kv` postoji, ali u novokreiranom projektu nema zapisa.

Reprodukcija potvrduje navedena cetiri stupca i `kv_rows = 0`. Stvarni settings JSON i template snapshot zapisuju se u privatne tablice: nije tocno da se projekti ili njihove postavke uopce ne spremaju. [DB contract](C:/Users/miron/Projects/QNC/contracts/databases/project-workspace.database.json) dozvoljava javno citanje kroz `public_read_views`, ne proizvoljni pristup privatnim tablicama.

Oznaka `active_project_id` je u globalnom registriju, a ne u prenosivoj projektnoj bazi. [Ingest manifest](C:/Users/miron/Projects/QNC/contracts/applications/ingest.application.json) ima prazan `read_database_contracts`; runtime ne ucitava radne postavke aktivne projektne baze.

Posljedica: kopija same projektne baze jos nije dovoljan ugovor za trazeni neovisni Ingest. Potrebni su javni DB podaci za aktivnost/postavke i read-only potrosac ugovora. Nije potreban Ingest -> Project API, adapter ili poslovna poruka.

### F07 / P1: local/LAN/Intranet nije zatvoren end-to-end put

[Project resolver endpoint](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:783) prihvaca lokalnu putanju i odbija mrezni endpoint. [Ingest DB endpoint](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/lib.rs:358) takodjer zahtijeva lokalnu SQLite datoteku. Konstrukcija resolvera ne daje dovrsenu mrezu owner servisa i klijenta.

[Ingest LAN/Internet akcije](C:/Users/miron/Projects/QNC/crates/qnc-ingest-components/src/lib.rs:334) eksplicitno javljaju da izvor nije povezan. Project browser potvrda je lokalna. Dir Browser javni session daje URI, ali listing i private binding su lokalni filesystem.

Javni reader kataloga aplikacija zasebno ima lokalni i HTTP read put te loopback testove. To je funkcionalnost tog modula, ne dokaz mreznog rada Project/Ingest baza ili browsera. Prisutnost `qnc-transport-resolver` ovisnosti sama nije zatvaranje ovog pravila.

### F08 / P1: Dir Browser spaja razlicite putanje na case-sensitive filesystemu

[normalize_path_for_identity](C:/Users/miron/Projects/QNC/crates/qnc-dir-browser/src/lib.rs:515) bezuvjetno pretvara cijelu putanju u mala slova prije izrade URI identiteta. [bind_path](C:/Users/miron/Projects/QNC/crates/qnc-dir-browser/src/lib.rs:190) zadrzava prvi binding za isti kljuc.

Iz koda slijedi da `/Media/A` i `/Media/a` dobivaju isti URI, iako mogu biti razliciti direktoriji. Drugi izbor tada moze razrijesiti prvi direktorij. To je greska javnog modula koja se prenosi na sve forme; nije pitanje samog prikaza separatora. Nije izvrsen live Linux test.

### F09 / P2: dvije Project instance citaju istu bazu, ali zadrzavaju razlicite predmemorije

[ProjectStore::open](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:92) ucitava projects root jednom. [set_projects_root](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:117) mijenja DB i samo trenutnu instancu. Druga instanca moze nastaviti stvarati projekte na staroj lokaciji.

Reprodukcija s dvije owner instance nad istim izoliranim registrijem: druga promijeni root, prva potom kreira projekt u starom rootu. To provjerava owner ponasanje, ne predstavlja izvrseni UI test s dva prozora.

Project forma takodjer cuva popise projekata/templatea u memoriji i osvjezava ih kroz vlastite akcije. Shell cuva hosted instance i pri povratku samo ponovno prikazuje istu instancu. Nema dogovorenog reload/revision koraka za promjene druge instance.

[SQLite konfiguracija](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:1043) vec ima `busy_timeout = 5000 ms`, foreign keys i pokusaj ukljucivanja WAL-a. Stari nalaz "nema multiprocess SQLite postavki" vise nije tocan. To ipak ne rjesava zastarjele predmemorije ili istodobne promjene filesystem zastite. Rjesenje ostaje ponovno citanje DB ugovora, ne komunikacija dviju aplikacijskih instanci.

### F10 / P2: filesystem rad i DB cekanje blokiraju UI thread

[Project komponenta](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/project_component.rs:100) sinkrono poziva store. Create/open/delete, browser listing, rekurzivno zakljucavanje i moguce cekanje SQLite locka zavrsavaju prije povratka iz UI akcije. Shell takodjer sinkrono stvara komponentu u svojem render putu.

Hosted oblik zato moze zaustaviti cijeli shell UI, a standalone vlastiti prozor. Razdvajanje u crateove nije razdvajanje izvrsavanja. Postojeci async katalog nije rjesenje za ove ostale operacije. Ovo je nalaz toka poziva, bez izmjerenog trajanja nad korisnikovim velikim projektima.

[Brisanje](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:493) drzi DB transakciju tijekom rekurzivnog brisanja direktorija i zastite; commit slijedi tek nakon toga. Uz dugo zakljucavanje postoji i greskovni prozor: neuspjeli commit nakon fizickog brisanja ostavlja registrij koji vise ne odgovara disku. Taj commit failure nije induciran u testu.

### F11 / P2: zastita direktorija siri se i na zapisivanje sadrzaja

[lock_project_dir](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:824) ne postavlja samo delete lock: cijelo stablo postavlja read-only. [POSIX varijanta](C:/Users/miron/Projects/QNC/crates/qnc-project-store/src/lib.rs:880) uklanja sve write bitove, a otkljucavanje vraca samo owner-write, ne izvorne grupne dozvole.

Izolirana Windows provjera pokazuje `SQLITE_READONLY` pri pokusaju izravnog updatea zakljucane projektne baze. To samo po sebi NIJE zahtjev da Ingest smije pisati Project bazu: on je mora citati read-only. Nalaz je da je zastita sira od zabrane slucajnog brisanja i da putanje za vlastite izlazne baze/artefakte drugih modula jos nemaju potvrden write put. Na POSIX-u rekurzivni read-only zahvaca i direktorije.

Ne uvoditi novi lease, recovery ili zastitni servis. Prije media upisa provjeriti delete-only namjeru i dozvoljen zapis u vlastite izlaze, zasebno na svakom OS-u.

### F12 / P2: identitet kartice ovisi o promjenjivom nazivu

[card_id](C:/Users/miron/Projects/QNC/crates/qnc-ingest-store/src/lib.rs:434) hashira source kind, serial i volume label. Reprodukcija: isti serial s drugim nazivom daje drugi `card_id`. Bez seriala, isti naziv moze objediniti razlicite kartice.

[Windows browser adapter](C:/Users/miron/Projects/QNC/crates/qnc-dir-browser/src/lib.rs:299) cita `GetVolumeInformationW`. Ne-Windows roots implementacija daje `/` bez serijskog broja/naziva volumena. To nije dovrsena MultiOS identifikacija kartice. Naziv treba ostati opisni podatak, a ne uvjet stabilnosti vec prepoznate kartice.

### F13 / P2: pasivna forma i keyboard put jos nisu potpuni

Glavne Project akcije imaju `action_id` i idu kroz komponentu; forma vise ne poziva izravno ProjectStore. Ingest isto salje intente. To je stvaran napredak.

Ipak [Project advanced](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/project_advanced.rs:670) u UI sloju stvara identitet i podatke korisnickog export preseta; Project forma koordinira i pretvorbe privatnih OS putanja. Te odgovornosti jos nisu sve u komponenti. Samo cuvanje teksta ili otvorenog panela u UI stanju nije poslovna logika.

[Odabir keyboard preseta](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:1455) sprema vrijednost u settings draft. [Dispatch](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:213) nastavlja koristiti ucitani shortcut katalog bez primjene tih postavki. Vanjska JSON datoteka i dispatch postoje; promijenjeni preset nije povezan s aktivnim dispatchom. Ingest takodjer jos nema citanje projektnog keyboard settings ulaza.

### F14 / P2: isti Project board nije jamstvo istog hosted izgleda

Standalone [primjenjuje svoj stil prije otvaranja prozora](C:/Users/miron/Projects/QNC/apps/qnc-project/src/main.rs:41). Hosted adapter to ne radi; dobiva shellov `egui::Context`. [Ingest show_desktop](C:/Users/miron/Projects/QNC/crates/qnc-ingest-desktop/src/app.rs:32) mijenja stil tog istog contexta pri prikazu, a Project ga pri povratku ne vraca.

Postoji put za prijenos UI stila izmedju hostanih povrsina. To nije dokaz dijeljenja poslovnog workflowa, ali je razlika izmedju dvaju modova i rizik za doslovni UI baseline. Soft/HighContrast hardkodirane boje su izricito privremeno dopustene; widget tema i painted pozadina ipak nisu potpuno ujednacene.

Pixel/fokus/resize usporedba standalone -> shell Project -> Ingest -> shell Project nije izvrsena u ovom auditu. Ne tvrdim da je prikaz 100% jednak v4.

## 2. Oba Project oblika: stvarni put izvrsavanja

```text
Samostalno:
  qnc-project.exe
    -> qnc_project_desktop::create_project_app(root)
    -> ProjectApp / ProjectComponent / ProjectStore
    -> eframe::App::update -> show_desktop
    -> vlastiti prozor i vlastiti egui Context

Unutar QNC desktopa:
  qnc-app.exe
    -> app registry -> desktop_entry -> factory
    -> qnc_project_desktop_adapter::create(root)
    -> isti qnc_project_desktop::create_project_app(root)
    -> nova instanca ProjectApp / ProjectComponent / ProjectStore
    -> ShellDesktopApp::show_desktop
    -> povrsina unutar shell prozora i shell egui Context
```

Ulazi su [standalone main](C:/Users/miron/Projects/QNC/apps/qnc-project/src/main.rs:19), [javni adapter](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop-adapter/src/lib.rs:13), [zajednicka konstrukcija](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/lib.rs:33) i [zajednicki prikaz](C:/Users/miron/Projects/QNC/crates/qnc-project-desktop/src/app.rs:192).

| Pitanje | Samostalni Project | Shell-hosted Project |
| --- | --- | --- |
| Postoji li? | Da, zaseban bin | Da, javni adapter i factory |
| Zasebna kopija poslovne implementacije? | Ne | Ne, isti desktop/component/store kod |
| Proces | qnc-project | qnc-app; ne pokrece qnc-project.exe |
| Treba shell? | Ne | Hostani prikaz koristi shell |
| Memorijsko stanje | Vlastita instanca | Vlastita instanca spremljena u shellu |
| Registry DB | Isti root daje isti registry | Isti root daje isti registry |
| Projektna DB | Owner otvara bazu konkretnog projekta | Isti owner kod |
| Navigacijski okidac | Nema | Nema, iako postoji prikazni adapter |
| Ingest/Project poslovni API | Nema | Nema pronadjene takve veze |

Zakljucak: ne treba stvarati dva nova Project oblika, kopirati poslovni kod ili uvoditi novi host. Oba oblika vec postoje. Nedostaju ponasanja na postojecim granicama.

`cargo tree` potvrduje shell -> javni adapter -> desktop/component/store iste aplikacije. Shell ne ovisi izravno o punom `apps/qnc-project` bin crateu. AGENTS odjeljak 16 eksplicitno privremeno dopusta ovaj tranzitivni in-process put. Zato ga ovaj audit ne prikazuje kao neodobreno povezivanje Ingesta s Project workflowom. Istodobno ovo nije out-of-process izolacija i pad/blokiranje hosted koda moze pogoditi host.

Oba root checka sada traze AGENTS i system seed, te odgovarajuci layout. Raniji nalaz "shell ne zahtijeva seed" vise ne vrijedi. Samostalnost ne znaci da je dovoljno prenijeti samo exe bez potrebnih instalacijskih artefakata. `--check-contracts` nije zamjena za stvarno pokretanje s ispravnim rootom.

## 3. Sto je stvarno implementirano

| Sloj | Trenutno stanje |
| --- | --- |
| Shell | Registry, genericka factory mapa, Project i Ingest adapter, footer i hostana povrsina; bez sljedece-grupe okidaca |
| Project | Standalone i hosted; projekti, templatei, advanced, browser, globalna i zasebne projektne baze |
| Project katalog | Samostalan generator + javni reader; stvarna prisutnost aplikacija, grupe a-z, najvise jedna varijanta po grupi, obvezna grupa a |
| Template zapis | Odabrani grupno sortirani slijed u projektnoj bazi; javni sequence view postoji |
| Ingest | Standalone i hosted; browser, potvrda izvora, card/location/session zapis |
| Dir Browser | Jedan javni lokalni session/list/select modul; URI i privatni binding; mreza i MultiOS volume identity nisu dovrseni |
| UI kit | Pasivni zajednicki obrasci, akcijska traka i ekskluzivni paneli; nije owner baze ili media workflowa |
| Neutralni moduli | Contract/manifest, DB validacija, resolver, keyboard, frame/timebase, browser, UI kit, katalog reader |
| Camera katalog | Samostalan alat i verzionirani DB artefakt s uredjivanjem/validacijom; nije scanner/detector koji je vec spojen na Ingest |
| Media moduli | Scanner, probe, filmstrip, wave, player i ostali planirani media runtime nisu zavrseni postojecom Ingest formom |
| Story / Media Assist | Ugovori/referentni plan, ne implementirane instalirane aplikacije u ovom stablu |

Inventar: 22 workspace Cargo paketa, 5 application manifesta (ukljucujuci shell host) i 20 module manifesta. JSON manifest nije sam po sebi izvrsi modul.

System template seed i zabrana njegova brisanja postoje. Korisnicki templatei imaju zaseban zapis i delete put. Default export direktorij i korisnicki export override imaju implementaciju; F01 je greskovni slucaj tog puta, a F02 govori o lokaciji projekta, ne o nepostojanju svih export funkcija.

Ingest sada ne radi puni scan/probe, niti filmstrip/wave obradu. To je oznaceno kao ogranicenje u AGENTS odjeljku 16 i ne smije se predstavljati kao dovrseni ingest. Nema pronadjenog naknadnog `ffprobe` fallbacka u Project/shell/Ingest runtimeu ovog reza. Nepovezane Ingest akcije imaju placeholder obradu; neke mijenjaju samo memorijski view i nisu dokaz DB workflowa.

## 4. Uskladjenost s AGENTS.md

| Pravilo | Ocjena prema kodu |
| --- | --- |
| Standalone + hosted za svaku implementiranu app | Project i Ingest imaju oba oblika |
| Shell nije poslovni owner | Uskladjeno za pronadjene pozive; host nema vlastitu poslovnu bazu |
| Shell koristi javni adapter | Da; tranzitivni desktop/store privremeno dopusten odjeljkom 16 |
| Poslovna veza samo DB | Nema pronadjenog Ingest -> Project API-ja; potrosnja radnih postavki jos nije implementirana |
| Globalna + projektna baza | Da; prenosivi javni settings/active ugovor nije dovrsen |
| Slijed aplikacija po grupama | Project odabir da; izvedeni entry i shell automatizacija ne |
| Pasivna forma | Glavne akcije preko komponenti; preostale advanced/path odgovornosti u UI-ju |
| Local/LAN/Intranet | Djelomicno u neutralnim ugovorima i katalogu, ne end-to-end u app runtimeu |
| MultiOS | Rust/OS adapteri postoje; konkretne path/volume/permission rupe ostaju |
| Jedan javni browser, dugmad u UI kitu | Da za modul i standardnu traku; lokalni paint kod jos se ponavlja |
| Probe jednom, rezultat u DB | Jos nije implementiran puni ingest; odsutnost probe koda nije potvrda tog workflowa |
| Vanjski shortcut katalog i dispatch | Postoje; primjena odabranog preseta i kompletan action tok nisu zatvoreni |
| Kratke transakcije i responzivan UI | Nisu osigurani za create/open/delete i sinkroni browser rad |
| Live identicnost v4 UI-ja | Nije provjerena ovim auditom |

Dokumentacija i conformance ne smiju zamijeniti ove razlike. Povijesni shell UI note opisuje raniju fazu; aktualne dozvoljene adaptere odredjuje odjeljak 16. Navigacijski okidac nije jos precizno naveden u javnom host API-ju. Korisnicku uputu da se signal ne sprema treba zadrzati pri sljedecem ugovornom koraku; ovaj audit nije mijenjao pravila.

## 5. Izvrsene provjere

- Procitani root AGENTS, arhitekturni/UI/DB dokumenti, app/module/DB manifesti i stvarni runtime putovi svih implementiranih slojeva. Posebno oba Project ulaza, adapter/factory, forma, komponenta, owner, katalog, browser i Ingest granice.
- `cargo metadata --no-deps --format-version 1`: inventar paketa.
- `cargo tree -p qnc-app --depth 3`: stvarna ovisnost hosta i adaptera.
- `cargo test --workspace --locked`: 134 testa prolaze.
- `cargo run --locked -p qnc-conformance`: sve provjere prolaze, katalog ima 85 shortcut akcija.
- `cargo fmt --all -- --check`: prolazi.
- Izolirani Rust harness koristi aktualne javne owner crateove i privremene baze. Nije mijenjao aplikacijski kod ni korisnikove projektne baze.

Sedam zabiljezenih opazanja harnessa, ne sedam novododanih workspace unit testova:

```text
FAILED_CREATE: error za ../invalid; registry_rows=1
TYPED_ROOT: created_at_default=true; created_at_requested=false
PUBLIC_SETTINGS: [project_id, template_id, created_at, updated_at]; kv_rows=0
WORKSPACE_WRITE_WHILE_LOCKED: SQLITE_READONLY (8)
ALTERNATIVE_GROUP_A: entry=step_alternative; expected=step_ingest
DUAL_INSTANCE_ROOT: first_created_at_old_root=true
RENAMED_CARD_SAME_SERIAL: different_card_ids=true
```

Lokalni reprodukcijski izvor: [audit harness](C:/Users/miron/AppData/Local/Temp/qnc-code-audit-20260906-01/src/main.rs), s [Cargo manifestom](C:/Users/miron/AppData/Local/Temp/qnc-code-audit-20260906-01/Cargo.toml). To je privremeni dijagnosticki alat izvan repozitorija, ne nova QNC komponenta. Valjani fixture projekti obrisani su kroz owner API; privremeni sandbox uklonjen je na izlazu. Kartica nije koristena, skenirana ili mijenjana.

Conformance uglavnom provjerava deklaracije, granice importa i trazene stringove/strukture. Ne provjerava da create ne ostavlja zapis nakon greske, da rucni root radi, da oba moda osvjezavaju stanje, da entry prati grupu, da javni settings nose podatke ili da hosted callback salje signal. Prolazak svih postojecih testova zato ne opovrgava reproducirane nalaze.

## 6. Sto nije provjereno

- Nije izvrsen novi native live/pixel/fokus test samostalnog i hostanog Projecta uz v4. Postojece slike i stari live zapisi nisu proglaseni novim dokazom.
- Nisu pokretane dvije stvarne GUI instance; dual-instance reprodukcija koristi dvije neovisne owner instance nad istom testnom bazom.
- Nisu provedeni fizicki LAN/Intranet, Linux, macOS ili ARM testovi.
- Nije proveden stvarni import, scan, probe, original/proxy pairing ili media generiranje s kartice.
- Nisu inducirani pad procesa, puni disk ili SQLite commit failure nakon fizickog brisanja.
- Tvornicke specifikacije camera kataloga nisu ponovno provjeravane na internetu; ovo je audit QNC koda i ugovora, ne nova certifikacija svih camera obrazaca.

## 7. Predlozeni nastavak bez novog monolita

1. Zadrzati oba postojeca Project ulaza i jedan zajednicki poslovni kod. Ne stvarati novu Project aplikaciju ili kopiju forme.
2. Prije oslanjanja na signal uspjeha zatvoriti F01/F02: create objava bez lazno uspjesnih zapisa i jedinstveni komponentni put za zadanu lokaciju. Dodati negativne provjere iz ovog audita.
3. Precizirati uski javni host navigacijski izlaz. Uspjesni create/open daje prolazni okidac; bez hosta aplikacija normalno nastavlja samostalno. Signal ne prenosi projektne radne postavke i ne sprema se u bazu.
4. Shell cita spremljeni izbor/slijed kao navigacijski podatak, provjerava dostupnost i aktivira precac sljedece odabrane grupe. Nema grananja po nazivima Project/Ingest/Story, pozivanja njihova poslovnog koda ili spremanja napretka u tudju bazu. Ispraviti F04/F05 na postojecim javnim granicama.
5. Provjeriti zasebno: create i open u hosted obliku; iste radnje bez shella; pogreska bez okidaca; preskocena grupa; alternativa u a; nedostajuca aplikacija; kraj slijeda. Za vidljivo ponasanje napraviti native live usporedbu oba moda i povratka iz Ingesta.
6. Zatvoriti DB-first i transport prije punog Ingesta: javni read-only active/settings ugovor, neovisni reader, stvarni endpointi, identiteti putanja/kartica i izlazne write dozvole. Nikakav Project proces ne smije biti preduvjet za Ingest.

Ovaj audit ne implementira navedene korake, ne mijenja AGENTS, ne migrira baze i ne zatvara live UI verifikaciju. Najblizi rizik nije nedostatak drugog Project oblika: to su nedostajuci host signal i neuskladjeni navigacijski podaci, uz reproducirane greske zajednickog owner koda.
