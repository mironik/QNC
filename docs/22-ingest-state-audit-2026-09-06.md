# Ingest audit: implementacija i nedostajuce veze

Datum: 2026-09-06. Auditirani commit: `0a46c1a`, `main` = `origin/main`.
Root: `C:/Users/miron/Projects/QNC`.
Referenca: `C:/Users/miron/Projects/qnc_v4`.

Ovo je nalaz i prijedlog, ne novo pravilo ili odobrenje implementacije.
Kod, postavke i poslovne baze nisu mijenjani auditom. Project ostaje zamrznut.

Naknadna korisnicka dopuna: prepoznavanje kartice treba koristiti kamerine
indekse i vec zapisane metapodatke, ne samo traziti datoteke po ekstenzijama.
Read-only Sony primjer i precizniji ulazni slijed dokumentirani su u
[Sony analizi](23-sony-card-index-analysis-2026-09-06.md).

## Odgovor: nedostaje li kod ili su pokidane veze?

Nisu svi dijelovi u istom stanju. Pregledani su svi izvorni direktoriji
novog roota, ukljucujuci ignorirane izvore izvan target/.git, Cargo runtime
ovisnosti i svih sest dostupnih commitova. Repozitorij nije shallow; dostupni
su main, origin/main i backup tag. Nije nadjena druga nova Ingest jezgra,
izvrsni scanner/probe niti njihovo uklanjanje iz te povijesti. To nije tvrdnja
o drugim repozitorijima ili izgubljenim/neobjavljenim granama.

| Dio | Stanje u novom QNC | Sto nedostaje |
| --- | --- | --- |
| Standalone i shell adapter | Implementirani, isti desktop/component sloj | Nije prepreka trenutnom radu |
| Citanje aktivnog projekta i postavki | Povezano kroz qnc-work-settings, read-only | Multi-station aktivacija i prenosivi binding nisu rijeseni |
| Naziv projekta u shell baru | Dolazi iz procitanog Ingest plana | Nije dokaz izvrsenog importa |
| Dir Browser i UI action bar | Povezani javni moduli; lokalne mape rade | Mrezni browser, potpuna identifikacija medija na ostalim OS-ovima |
| Kartica, izvor, sesija | record_source_selection stvarno zapisuje registry | Stabilniji identitet i izvrsenje sljedecih faza |
| Radni plan i odredisni URI-ji | Postoje u komponenti | Nema potrosaca koji izvrsava plan ili pise artefakte |
| Selekcija klipova | Toggle/select-all/clear postoje u memoriji komponente | Nema punjenja clips iz baze ni trajnog zapisa izbora |
| Baza klipova i probe | Tablice i javni prikazi postoje | Nema runtime metode koja puni clips/probe_records |
| Katalog struktura kamera | Stvarna SQLite baza i zaseban publisher/validator | Runtime citac/detektor koji primjenjuje obrasce nije spojen niti implementiran |
| Scanner, original/proxy grouping, Media Probe | Manifesti; nema izvrsnih novih modula | Implementirati/izdvojiti nakon provjere postojeceg v4 koda |
| Posteri | thumb_uri polje postoji | Renderer ga ne koristi; kartica uvijek crta tri tocke |
| Filmstrip, wave, player, timeline | Ugovori i/ili pasivni prikazi | Nema izvrsnih media modula u novom Ingest runtimeu |
| Uvezi | Intent postoji, komponenta vraca gresku 'nije implementiran' | Nema importa ni completion lifecyclea |

Konkretan stari kod postoji:

- [v4 scanner](C:/Users/miron/Projects/qnc_v4/qnc-host/src/ingest/scanner.rs:19): scan_inventory.
- [v4 probe](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/proxy.rs:351): probe_media i probe_media_fast.
- [v4 plan importa](C:/Users/miron/Projects/qnc_v4/qnc-host/src/media/resolve.rs:616): resolve_import_plan.
- [v4 import pipeline](C:/Users/miron/Projects/qnc_v4/qnc-host/src/ingest/import_pipeline.rs:30): advance_selected_import_pipeline.
- [v4 filmstrip](C:/Users/miron/Projects/qnc_v4/qnc-media-ffmpeg/src/filmstrip.rs:44): postojece strategije izdvajanja.

To su kandidati za izdvajanje, ne odspojeni crateovi u novom rootu. Ne smije
se povezati cijeli stari host/ProjectDbBroker ili prenijeti sve stare probe
fallbackove. Pojedinacni v4 postupak treba uskladiti s novim ugovorom; ne
pretpostaviti da svaka stara rutina vec postuje danasnji AGENTS.

## Nalazi koji blokiraju puni ingest

### F1 / Visoko: Select jos ne izvrsava ingest

`crates/qnc-ingest-components/src/lib.rs:491` vodi potvrdu na ucitavanje
postavki, pa `:651` na record_source_selection. Ne slijede popis klipova,
grupiranje original/proxy, probe ili punjenje content baze. `:542` izricito
odbija import. `crates/qnc-ingest-store/src/lib.rs:260` samo kreira shemu.

To je poznat nedovrseni korak iz AGENTS 16, a ne dokaz da probe nepravilno
radi ili da je spor. Trenutno u novom Ingestu nema runtime media probea.
Nema ni osnove tvrditi da je zakon jednokratnog probea funkcionalno verificiran.

### F2 / Visoko: radni plan nije spojen na odredista i rezultate

`crates/qnc-ingest-components/src/work_plan.rs:31` pripremi uloge odredista
original/proxy/audio/incoming/ingest-thumbnails/filmstrip kao QNC URI.
Izvan testova nema potrosaca tih polja koji upisuje media rezultat.
`crates/qnc-ingest-store/src/lib.rs:71` uvijek postavlja dvije vlastite baze
pod root/data, ne prema ucitanom odredisnom planu. Ne postoji trajni izbor
klipova, zapis import operacije niti ponovno ucitavanje rezultata u view.

Ovdje stvarno postoji nedovrsena veza. Nije nalog za uvodenje jos jedne
Project baze ili novog editora postavki. Treba dogovoriti i spojiti postojeci
DB/URI ugovor za Ingest rezultat; globalni katalog klipova moze ostati globalan
ako ugovor tako odredi. Radni rezultat mora imati nedvosmislen izvor/odrediste.

### F3 / Visoko: local/LAN/Intranet nisu isti izvrsni put

`crates/qnc-ingest-components/src/lib.rs:453` i sljedeca grana daju samo
poruke za LAN/Internet. Dir Browser mapira samo lokalne putanje. Store
`sqlite_db_path` u `crates/qnc-ingest-store/src/lib.rs` odbija network endpoint;
runtime otvara lokalni default i nema konfiguriranu mrezu za upis Ingest baze.
U work-settings modulu mrezni read-only endpoint jest implementiran, ali to
nije media listing/read/write transport niti Ingest DB write endpoint.

AGENTS 1/9 zahtijeva stvarni zajednicki ugovor, ne samo URI stringove. Dodavanje
local-only scanner poziva sada bi zacementiralo ovu razliku.

### F4 / Visoko: jedan aktivni projekt za cijeli registry

`crates/qnc-work-settings/src/local.rs:52` cita jedan active_project_id.
`server.rs` prihvaca samo registry_uri, bez zasebnog konteksta radne stanice.
Kod odvojenih stanica koje koriste isti registry izbor drugog korisnika moze
promijeniti sto sljedeci Ingest refresh/Select procita. Metapodaci porijekla
projekta ne rjesavaju odabir trenutnog projekta po stanici.

To je ogranicenje postojeceg DB ugovora, ne poziv da Ingest upravlja projektima.
Promjena Project aktivacije zahtijeva posebno odobrenje. Zasebni konfigurirani
registryji izoliraju izbor, ali nisu isto sto i dogovoreni shared-server model.

### F5 / Visoko: kopirana projektna baza jos nije dovoljna za rad

Reader prvo mora imati globalni registry, zatim privatni apsolutni local_path
iz project_storage_locations (`crates/qnc-work-settings/src/local.rs:80`).
Na novom racunalu/OS-u ta putanja moze biti nepostojeca ili neapsolutna; reader
nema zaseban nacin konfiguriranja bindinga odabrane workspace baze.
Nema ovisnosti o Project executableu, sto je ispravno, ali jos nema potpunog
prenosivog DB artefakta s odgovarajucim lokalnim transport bindingom.

## Konkretni bugovi i rizici postojeceg koda

### F6 / Srednje: stale rezultat citanja pri povratku u Ingest

`load_work_settings` (`crates/qnc-ingest-components/src/lib.rs:368`) odbija
novi zahtjev dok receiver postoji. Zamisliv kodni slijed: Ingest zapocne
citanje P1, korisnik ode u drugu povrsinu, baza aktivira P2, povratak pozove
on_activated, ali stari receiver jos nije pollan. Novi reload se odbije;
poll potom prihvati stari P1. Nema generation/revision provjere ni queued
refresh zahtjeva. Nije reproducirano klikanjem; nalaz proizlazi iz lifecyclea
i uvjeta u kodu. Posebno vazno prije sporijeg mrezenog rada.

### F7 / Srednje: granice promjene/otkaza izvora nisu potpune

`settings_failed` (:358) ukloni plan, ali ne clips/preview/source prikaz.
Uspjesna promjena projekta cisti source_uri, ali ne pripadni naziv/serial/volume.
`capture_selected_source_metadata` (:634) moze sacuvati prethodne metapodatke
kad novi root nema identitet. DIR_OPEN ponisti pending_source; DIR_UP,
DIR_ROOTS i ponovni Local to ne rade. Nakon Select pa navigacije Gore moze se
dovrsiti potvrda stare lokacije, iako browser vec pokazuje drugu.

Treba jedan dosljedan reset/otkaz u komponenti i nepovezivanje stare kartice
s novim URI-jem. UI ne smije biti vlasnik tog postupka.

### F8 / Srednje: greske akcija se gube prije prikaza

`crates/qnc-ingest-desktop/src/app.rs:55` koristi samo request_repaint,
ignorira accepted/message. `poll` ignorira rezultat confirm_source_selection.
view.message nije prikazan u trenutnim widgetsima. Primjer: odbijeni Uvezi
ili neuspjeli source DB upis ne daje odgovarajucu korisnicku poruku. Nepoznate
akcije jos vracaju accepted i poruku da komponenta nije spojena.

### F9 / Visoko za deduplikaciju: identitet kartice nije stabilan

`crates/qnc-dir-browser/src/lib.rs:190` gradi source URI iz hasha fizicke
putanje. Druga kartica na istom slovu diska dijeli URI; ista kartica na drugom
slovu dobiva novi. `crates/qnc-ingest-store/src/lib.rs:434` u card_id ukljucuje
volume_name: preimenovanje istog medija mijenja identitet. Bez seriala
jednako ime volumena moze spojiti razlicite medije. Hash nije zamjena za
model identiteta medija i odvojen identitet promjenjive lokacije.

`qnc-dir-browser` na Windowsu cita volume serial, a ne zajamceni hardware
serial kartice. Na ostalim OS-ovima (:260) vraca samo / bez identiteta
montiranih medija. Stari/novi klipovi i ponovni ingest kartice stoga jos nisu
spremni za multiOS deduplikaciju.

### F10 / Srednje: browser i source DB zapis blokiraju UI thread

Samo citanje radnih postavki ide u worker. Otvaranje browsera i navigacija
iz dispatcha sinkrono rade is_dir/canonicalize/read_dir/metadata/volume lookup.
Potvrda nakon poll-a sinkrono zapisuje SQLite uz busy_timeout od 5 sekundi.
Komponentna granica je ispravna, ali sama po sebi ne jamci responzivnost.
Spor disk, nedostupan mapped drive ili DB lock mogu zaustaviti prikaz.

### F11 / Srednje: ucitane postavke nisu sve primijenjene

Keyboard active_preset je u WorkSettings, no
`crates/qnc-ingest-desktop/src/app.rs:62` dispatch koristi originalni katalog
iz IngestContracts. Nema apply koraka iz ucitanog keyboard zapisa.
Video/audio su preneseni, ali jos nema playera ili import modula koji bi ih
primijenio. Ucitavanje nije isto sto i izvrsavanje postavki.

Zadnja promjena poistovjetila je ai.enabled s ai_mining i onemogucila checkbox.
V4 `qnc-app/src/project/ai.rs:24` opisuje ai.enabled kao analizu/virtualne
kadrove, dok `qnc-app/src/ingest/mod.rs:971` ima zaseban SetAiMining izbor.
Nema eksplicitnog ugovora da su to isti podatak. To mapiranje treba razjasniti,
a ne proglasiti vjernom primjenom postojeceg v4 postupka.

### F12 / Srednje: UI izgled nije dokaz funkcionalnosti medija

`crates/qnc-ingest-desktop/src/widgets.rs:591` ne koristi thumb_uri; uvijek
crta '...'. Preview crta naziv na crnoj povrsini, a timeline (:878) prazne
trake. All/Virtual klikovi se ignoriraju. Play i frame akcije mijenjaju bool/
brojac u komponenti, bez dekodiranja ili play clocka. Generiraj postere i
audio-lane akcije nemaju izvrsnu implementaciju. Nema completion/exit stanja
koje bi ostvarilo batch_exit_after_completion iz manifesta.

## Sto je ispravno i ne treba pisati ponovo

- Nema runtime ovisnosti Ingesta o Project app/store/adapteru. Cargo stablo to potvrduje.
- Forma prikazuje view i salje action_id; DB upis je u vlastitom storeu.
- Javni Dir Browser i qnc-ui-kit stvarno se koriste; potvrdna dugmad nisu browser workflow.
- Reader cita Project baze read-only, bez migracije, popravljanja i private-settings fallbacka.
- Radne postavke imaju stvaran rezultat i kontrolirane greske; UI nije izvor njihove istine.
- Ingest source registry ima stvarne transakcije, javne viewove, WAL i busy_timeout.
- Original/proxy/probe i artefact tablice postoje kao pocetna shema, ne kao gotov workflow.
- Katalog kamera stvarno postoji: 23 obrasca, 17 kandidata i 7 zabiljezenih rupa.
- Standalone executable i hosted adapter koriste istu aplikacijsku povrsinu.
- Postojeci shell status je prikazni ugovor, ne poslovna veza izmedu aplikacija.

## Verifikacija i granice

Ponovno izvrseno u ovom auditu:

- cargo test --workspace --locked --quiet: 182 prolaza, 0 padova.
- qnc-conformance: all checks passed.
- qnc-camera-catalog check: katalog valjan; sam alat izricito navodi da runtime detector nije implementiran.
- qnc-work-settings nad stvarnim registryjem: projekt 6, field/link,
  proxy_if_available, 1080p50, audio 48000 Hz, keyboard default.
- Cargo normal dependency tree, sva trenutna izvorna stabla i dostupna Git povijest.
- Relevantni v4 scanner/probe/import i AI/source-dock izvori.

Testovi pokrivaju implementirani temelj, ne puni ingest kojeg nema. Conformance
ne otkriva sve F2/F4/F6/F8/F11 praznine; prolaz manifesta nije end-to-end dokaz.
Nije izvodjen import, ffprobe/ffmpeg, upis na karticu, novi UI live postupak,
zaseban LAN/Intranet host niti Linux/macOS/ARM izvrsavanje. F6/F7 su kodni
nalazi, bez novog reprodukcijskog testa u ovom koraku.

## Prijedlog nastavka, prije implementacije na potvrdu

1. Ne pisati novi reader/browser/store od pocetka. Zatvoriti F6-F8 i
   precizirati postojece veze radnog plana s odredisnim transportom i trajnim
   Ingest rezultatom. Ne dodavati Project postavke u Ingest formu.
2. Zakljuciti identitet medija i source/list/read/write ugovor za local/LAN/
   intranet. Odvojiti medij od slova diska i lokacije. Shared active-project
   izbor je zaseban DB ugovorni problem koji ne smije preuzeti Ingest.
3. Za prvi funkcionalni Select izdvojiti odgovarajuci v4 scanner i pairing
   postupak u javne module koji koriste katalog. UI dobiva samo rezultate.
   Live: kartica read-only, jedan original = jedan clip, proxy samo veza.
4. Spojiti jedini probe prolaz i trajni probe zapis. Live mjeriti cjelokupni
   Select, prikazivati dovrsene zapise u batchu; kasniji moduli citaju bazu.
5. Tek tada posteri, odabir/import i DB-first filmstrip/wave/player prema
   zasebnim ugovorima. Ne prenositi cijeli stari host niti njegove fallbackove.

Svaki korak ima zasebnu provjeru prije iduceg. Ovaj audit nije odobrenje za
otkljucavanje Projecta ili promjenu vec potvrdenog UI layouta.
