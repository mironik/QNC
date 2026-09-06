# Ingest: citanje radnih postavki

Datum: 2026-09-06. Odobreni zahvat, bez promjene Project UI-ja i aktivacije.

## Ugovor

Project vec zapisuje postavke u `project_settings.settings_json`. Javni
`public_project_settings` sada iznosi isti zapis, bez dodatnog poslovnog
stanja. Reader ne migrira ni popravlja postojece baze. Baza sa starim javnim
prikazom bez postavki mora dati gresku, ne fallback na privatnu tablicu.

Ingest prvo cita `active_project_id` iz `public_app_settings` i provjerava
odgovarajuci `public_projects` red, zatim cita postavke iz odabrane projektne
baze. Ne poziva Project, ne trazi njegov executable i ne dobiva settings od
shella. Postojeci izbor je jedan po registry bazi; ovo ne uvodi izbor po
stanici, sesiji niti korisniku.

Javni modul `qnc-work-settings` odgovoran je samo za read-only snapshot radnih
postavki. Lokalni adapter koristi ownerov postojeci privatni zapis fizickog
bindinga `project_storage_locations` iskljucivo za URI resolver. Taj zapis
ne postaje poslovni API niti izlazi iz transport adaptera. Poslovni SELECT
upiti koriste javne DB prikaze. Ne postoji dependency na Project crateove.

Mrezni adapter koristi HTTP(S) preko konfigurirane authority adrese, ne SMB
otvaranje SQLite datoteke. Opcionalni zasebni read-only helper daje isti
snapshot iz baze na storage hostu; nije Ingest/Project servis ni app bridge.
Helper ne pokrece aplikacije, ne zapisuje baze i ne prima SQL naredbe.

Lokacije u izlazu su QNC URI. Fizicke putanje ostaju lokalnom adapteru.
Snapshot daje korijenski odredisni URI i postojece ingest/storage, playback,
input, video, audio, AI i keyboard postavke; ne uvodi export workflow.
Medijski upis, scanner i probe nisu dio ovog koraka. Ne tvrditi da read-only
mrezni adapter time implementira i mrezni media browser ili media upis.

## Ingest i UI

Citanje ide u workeru. Komponenta drzi snapshot i gresku; forma samo prikazuje
stanje i salje postojece intente. Nema rucnog editora projektnih postavki.
Nema defaults kada nedostaje aktivni projekt, baza ili obvezni podatak.
Postavke se ucitavaju na pocetku i ponovo uz postojece Osvjezi/Select akcije.
Za zapoceti odabir zadrzava se procitani snapshot, ne mijenja ga drugi UI.

Odobreno minimalno vidljivo odstupanje: kod nedostupnih postavki postojeci
prazni media panel prikazuje gresku komponente umjesto genericke prazne poruke.
Raspored panela, browser i gumbi ostaju isti. DB-controlled AI stanje se
prikazuje bez lokalnog overridea.

## Odobren prikaz u shell baru

Korisnik je 2026-09-06 zatrazio da desni kut shell footera umjesto
`Ingest aktivan` prikazuje naziv projekta koji je Ingest stvarno ucitao.
Referenca: v4 `qnc-app/src/app.rs` footer s tri kolone. Postojeca geometrija,
font i tabovi ostaju; mijenja se samo tekst desne kolone, uz skracivanje
dugog naziva i puni tekst na hoveru. Nema novog dugmeta ili shortcuta.

Naziv dolazi iz uspjesno ucitanog Ingest radnog snapshota. Shell ga samo
prikazuje preko generickog javnog desktop statusa, bez DB upita, projektnih
postavki ili prosljedivanja drugoj aplikaciji. Dok traje citanje ili nakon
greske nema starog naziva. Ponovna aktivacija desktop povrsine salje samo
lifecycle obavijest bez payloada; Ingest sam ponovo cita bazu. Standalone
koristi isti komponentni reader i ne ovisi o shellu.

Naknadno korisnik trazi isti prikaz i za Project: desna kolona daje naziv
selektiranog retka iz postojeceg Project prikaznog stanja, ne naziv novog
projekta iz tekstualnog inputa i ne kopiju statusa Ingesta. Bez selekcije
prikazuje `Projekt nije odabran.`. Isti desktop status ugovor, font,
skracivanje i hover; nema promjene postojecih akcija ili aktivacije.

## Verifikacija

- Reader korak: `cargo test --workspace --locked --quiet`, 179 testova prolazi.
  Read-only snapshot, nedostupni/stari prikazi, promjena aktivne baze i stvarni
  HTTP loopback zahtjevi za LAN/Intranet URI-je provjereni su testovima.
- Ingest footer: 27 ciljanih testova (`qnc-ingest-components`, `qnc-app`,
  `qnc-shell-desktop-api`) prolazi. Naziv se mijenja tek nakon uspjesnog
  citanja; tijekom ucitavanja/greske ne prikazuje se stari naziv.
- Project footer: 23 ciljana testa (`qnc-project-desktop`,
  `qnc-project-desktop-adapter`, `qnc-app`) prolaze. Test selekcije obuhvaca
  promjenu retka, selekciju razlicitu od active zastavice i uklonjen redak.
- `qnc-conformance` i `git diff --check` prolaze. Svjeze izgradeni `qnc-app`
  i `qnc-project`. Windows live: u Project desnom footeru `novi-novi--x5`;
  prethodni Ingest live prikaz pokazao je `projekt 6` iz njegovog snapshota.
  Nazivi nisu hardkodirani i ne preuzimaju se iz druge aplikacije.
- Nisu provjereni zasebni LAN/Intranet hostovi ni Linux/macOS native prikaz.
  Ovaj korak ne ukljucuje media import/probe/generiranje, niti potvrdu da
  buduci media moduli vec primjenjuju sve prenesene postavke.

## Provjera postojeceg koda i v4

Provjeren backup `f7fafad`: nijedan Ingest app/component/store/desktop crate
ne cita `active_project_id`, `public_project_settings`, `ingest_media` ili
`ingest_profile`. Ingest manifest ima prazan `read_database_contracts`.
Browser, source registry/session, clip selection i UI-kit vec postoje i ne
pisu se ponovo. Novi reader i plan nisu predstavljeni kao postojeca izvedba.

Referentni kod u `C:/Users/miron/Projects/qnc_v4`:
- `qnc-host/src/media/resolve.rs`: `ingest_media_choice_for_project`,
  `resolve_import_plan`, `ingest_profile`, proxy/original policy.
- `qnc-host/src/ingest/store.rs`: `snapshot_ingest_media`,
  `snapshot_playback_input`, `snapshot_import_path`, `load_state_from_connection`.
- `qnc-host/src/jobs.rs`: `payload_for_ingest_media_prepare_claim` raspored
  proxy/original/audio odredista.
- `qnc-host/src/ingest/db.rs`: ingest/thumbnails relativno korijenu.

Prijenos u ovom koraku: strogo citanje link/proxy/original i
proxy/original/proxy_if_available izbora; iste uloge odredisnih direktorija
kao QNC URI u Ingest komponenti. Nisu preneseni ProjectPaths/ProjectDbBroker,
pozivi Projectovih metoda, spajanje baza aplikacija, filesystem provjere medija,
proxy generator, ni prazni/default projekt kao fallback.

## Transport konfiguracija

Bez konfiguracijske datoteke reader koristi lokalni registry u
`data/project_store.db` relativno runtime rootu, kao postojece aplikacije.
To je lokacija baze, ne default projekt ili default radne postavke.

Opcionalni `data/work-settings-transport.json` ili putanja iz
`QNC_WORK_SETTINGS_CONFIG` sadrzi samo transport konfiguraciju:

```json
{
  "registry_uri": "qnc://lan/storage/db/project_registry",
  "registry_file": null,
  "endpoint": "https://storage.example/qnc",
  "token_env": "QNC_WORK_SETTINGS_TOKEN"
}
```

Za lokalnu konfiguraciju `registry_uri` je `qnc://local/db/project_registry`,
`registry_file` je privatna lokacija registryja, a endpoint/token_env su null.
Relativna registry_file putanja racuna se prema direktoriju konfiguracije.
Nema fallbacka sa neispravne eksplicitne konfiguracije na lokalne postavke.

Storage helper: `qnc-work-settings --root <storage-root> --serve 127.0.0.1:9011
--registry-uri qnc://lan/storage/db/project_registry`. Token je obvezan preko
`QNC_WORK_SETTINGS_TOKEN`. Ingest ne pokrece helper; storage administrator ga
moze postaviti iza HTTPS reverse proxyja s timeoutom i limitom zahtjeva.
Helper slusa samo na loopbacku; javno slusanje i upravljanje TLS-om nisu u njemu.
HTTP klijent dopusta nekriptirani promet samo prema loopbacku, ne LAN hostu.
To nije gotov deployment niti potvrda stvarnog udaljenog LAN/Intranet testa.

Izvrsna provjera bez forme: `qnc-work-settings --root <runtime-root>` ucita
trenutni DB izbor, ispise radni snapshot i ugasi se. Ne prima projektni ID.
