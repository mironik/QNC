# QNC contracts

Status: pocetni contract dokument  
Datum: 2026-09-04

## Application/form contract

Svaka QNC aplikacija/forma mora imati manifest. Shell / QNC.app smije imati
host manifest, ali nije poslovna aplikacija/forma i ne ulazi u ownership
matricu poslovnih aplikacija.

```text
application_id
application_name
application_version
application_kind
lifecycle
owned_database_contracts
read_database_contracts
owned_artifacts
module_dependencies
capabilities
supported_os
supported_cpu
transport_protocol_version
keyboard_shortcut_contract
ui_layout_reference_contract
ui_layout_contracts
workflow_forbidden_operations
workflow_forbidden_capabilities
status_contract
error_contract
```

`lifecycle` moze biti:

- `interactive_form`
- `batch_exit_after_completion`
- `runtime_while_form_active`
- `command_service`
- `interactive_desktop_host`

`application_kind` za Shell / QNC.app mora biti `desktop_host`. Poslovne
aplikacije koriste vlastiti application kind, npr. `interactive_form`,
`interactive_batch_form` ili `interactive_form_variant`.

## Module contract

Svaki modul mora imati manifest:

```text
module_id
module_name
module_version
module_kind
capabilities
input_contract_version
output_contract_version
supported_os
supported_cpu
transport_protocol_version
forbidden_calls
forbidden_dependencies
state_policy
database_write_policy
```

Modul manifest ne smije imati `allowed_applications`, `allowed_modules` ili
slican hardkodirani popis aplikacija koje ga smiju koristiti. Modul je javni
QNC resurs. Njegov manifest smije definirati samo sto modul nudi i sto sam ne
smije pozvati.

`module_kind` moze biti:

- `library`
- `in_process_plugin`
- `out_of_process_helper`
- `network_service`
- `ui_widget`

`state_policy` mora biti jedno od:

- `stateless`
- `session_local`
- `owned_by_calling_application`

`database_write_policy` mora biti jedno od:

- `no_db_writes`
- `public_db_owner_write_adapter_only`
- `public_db_write_transports_only`
- `public_ingest_content_write_transport_only`
- `narrow_project_close_write_adapter_only`
- `returns_artifact_to_public_write_transport`

Stare module politike `owner_application_only` i
`returns_result_to_owner_application` nisu dozvoljene za module. Modul koji
treba trajni zapis vraca rezultat ili write naredbu javnom DB owner/write
adapteru ili javnom DB/transport writeru; ne pise bazu direktno i ne predaje
odgovornost aplikacijskoj formi.

## DB contract

Svaka baza ili javna shema mora definirati:

```text
database_id
owner_application
schema_version
qnc_uri
tables
public_read_views
write_owner
public_read_policy
migration_policy
created_at
updated_at
```

Pravila:

- Samo owner aplikacija pise u svoju bazu ili shemu.
- Druge aplikacije citaju samo public read views ili dogovorene tablice.
- Raw OS path nije javni identitet baze.
- DB contract mora raditi lokalno, na LAN-u i u intranetu.
- Project aplikacija kreira globalnu registry bazu i posebnu projektnu bazu za
  svaki projekt. Postavke po kojima rade Ingest, Media Assist, Story i druge
  aplikacije zapisuju se u projektnu bazu tog projekta.
- Aplikacije ne dobivaju workspace, runtime context za poslovno stanje,
  nasljedivanje ili alat za suradnju, i ne pozivaju Project aplikaciju. One
  citaju oznaku aktivnog projekta i postavke za rad iz baze read-only, kroz
  javni ili dogovoreni DB contract.
- Ingest nema rucno postavljanje projektnih radnih postavki. Postavke po kojima
  Ingest radi dolaze iz baze aktivnog projekta.
- QNC baza je prenosivi poslovni artefakt. Ingest mora moci raditi na racunalu
  na kojem ne postoje Project aplikacija ni QNC.app shell, ako je dostupna
  valjana baza s oznakom aktivnog projekta i postavkama za rad.
- Ako aplikacija ne moze procitati aktivni projekt ili njegove postavke za rad,
  mora stati u kontrolirano stanje greske. Ne smije kreirati projekt,
  popravljati Project bazu niti izmisliti default postavke.

Project registry smije imati privatne runtime tablice za lokalnu storage
konfiguraciju i mapiranje projekta na fizicku mapu. Te vrijednosti nisu javni
identitet projekta i ne smiju zamijeniti QNC URI contract.

Project system template seed je obvezan:

```text
seed/system_seed.json
```

Seed je lokalna kopija iz `qnc_v4` i mora sadrzavati system project/source
templatee. Project aplikacija ga seeda u svoju registry bazu pri otvaranju.

## Transport URI contract

QNC javne reference na DB i media lokacije koriste QNC URI.

Pocetni oblik:

```text
qnc://local/{resource_kind}/{resource_id}
qnc://lan/{authority}/{resource_kind}/{resource_id}
qnc://intranet/{authority}/{resource_kind}/{resource_id}
```

Primjeri:

```text
qnc://local/db/project_registry
qnc://local/db/ingest_content/source_123
qnc://lan/storage-a/media/source_123/clip_456/original
qnc://intranet/mam-a/db/story/news_story_001
```

Pravila:

- QNC URI je javni identitet.
- Resolver pretvara QNC URI u stvarni pristup za OS i okruzenje.
- Aplikacije i moduli ne smiju spremati raw Windows/Linux/macOS putanje kao
  javni contract.
- Raw path moze postojati samo kao privremeni rezultat resolvera unutar procesa.

## Boundary contracts

Svaka aplikacija mora eksplicitno navesti workflow zabrane: sto ne smije
pokrenuti ili koristiti u svojem workflowu.

Svaki modul mora eksplicitno navesti dependency zabrane: sto on sam ne smije
pozvati, ucitati ili pokrenuti.

Ni aplikacijske ni modulne zabrane ne smiju biti hardkodirani popis korisnika
koji smiju koristiti modul.

Globalne zabrane:

- aplikacija ne pise u tudu bazu
- aplikacija ne poziva privatni kod druge aplikacije
- modul ne spaja dvije aplikacije mimo baze
- modul ne poziva dependency koji je izvan njegova dependency boundaryja
- UI ne radi scan/probe/media obradu
- probe se ne radi izvan Ingest procesa
- Filmstrip/Wave/Player/Export ne pozivaju `ffprobe` ni Media Probe
- proxy nije zaseban clip
- raw OS path nije javni identitet

Primjer modulne dependency zabrane:

```text
Filmstrip module:
  forbidden_calls:
    - ffprobe
    - Media Probe
    - scanner
    - Ingest workflow
```

## Keyboard shortcut contract

QNC keyboard shortcut ugovor je obvezna vanjska datoteka:

```text
C:\Users\miron\Projects\QNC\contracts\qnc-keyboard-shortcuts.json
```

Pocetni izvor tog contracta je postojeci QNC v4 katalog:

```text
C:\Users\miron\Projects\qnc_v4\seed\keyboard-shortcuts.json
```

Pravila:

- UI, aplikacije i moduli ne smiju hardkodirati shortcut chordove direktno u
  kod.
- Kod mora koristiti shortcut katalog preko action id-a i shortcut/keymap
  modula.
- QNC shortcut katalog je vanjski contract koji se provjerava pri kodiranju
  svakog UI-ja, menija, playera, timelinea, browsera ili input handlinga.
- SQLite korisnicke postavke smiju biti user override preko istog contracta.
- OS specificne razlike smiju biti samo u shortcut contractu ili user overrideu,
  ne razbacane po kodu.

## UI/layout reference contract

QNC UI/layout referentni ugovor je obvezan:

```text
C:\Users\miron\Projects\QNC\docs\07-ui-layout-reference.md
```

Pocetni izvor je postojeci QNC v4 UI:

```text
C:\Users\miron\Projects\qnc_v4
```

Pravila:

- Novi UI mora biti doslovna preslika relevantnog QNC v4 UI-ja.
- Raspored, redoslijed elemenata, nazivi, font, razmaci, poravnanja, fokus,
  keyboard ponasanje i stanja prikaza moraju ostati isti.
- UI/layout kod iz QNC v4 smije se koristiti za doslovno preslikavanje.
- Aktivna poslovna logika iz QNC v4 ne smije se prenijeti bez razdvajanja na
  aplikaciju/modul i bez jasnog odobrenja.
- Redizajn, reinterpretacija i uljepsavanje nisu dozvoljeni bez izricitog
  odobrenja.

Konkretni UI layout contracti nalaze se u:

```text
contracts/ui/*.layout.json
```

Project DB referenca iz qnc_v4 zapisana je u:

```text
docs/09-project-db-reference-audit.md
```

## Minimalni conformance testovi

Prije prvog ozbiljnog koda moraju postojati testovi ili provjere za:

- application manifest schema
- module manifest schema
- zabranu `allowed_applications` u module manifestu
- DB owner/write policy
- QNC URI parser/validator
- QNC keyboard shortcut catalog validation
- zabranu hardkodiranih shortcut chordova mimo catalog/user override modela
- UI/layout mirror validation prema relevantnom QNC v4 prikazu
- zabranjene direct app-to-app imports
- zabranjene `ffprobe` pozive iz Filmstrip/Wave/Player/Export modula
- zabrane workflowa aplikacija i dependency zabrane modula
- supported OS/CPU deklaracije
