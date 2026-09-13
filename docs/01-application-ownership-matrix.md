# QNC application ownership matrix

Status: pocetni ownership ugovor  
Datum: 2026-09-04

## Aplikacije i moduli

Shell / QNC.app nije poslovna aplikacija/forma i nije red u ownership matrici.
Shell je desktop host: cita app registry, prikazuje aplikacijske povrsine i ne
posjeduje poslovne baze ni workflowe Project/Ingest/Media Assist/Story
aplikacija.

| Aplikacija/forma | Posjeduje | Pise | Cita | Tipicni moduli u workflowu | Workflow zabrane |
| --- | --- | --- | --- | --- | --- |
| Project | project registry, project settings, DB/location registry | Project DB | vlastitu bazu i javne status DB-ove po potrebi | manifest/capability, DB validation, transport/resolver, Dir Browser za izbor lokacija | scan/probe, filmstrip/wave, player decode, export/render, Story workflow, Media Assist workflow |
| Ingest | source/card identity, source scan, original/proxy grouping, jedini media probe rezultat | Ingest DB | oznaku aktivnog projekta i postavke za rad iz baze, vlastitu bazu | Dir Browser, Media Browser, Media Probe, Filmstrip, Wave, transport/resolver, scanner, camera detector | Project app/API/crate/workflow, pisanje u Project/Story/Media Assist DB, pozivanje Story/Media Assist workflowa, proxy kao zaseban clip, naknadni probe |
| Media Assist | analysis/assist rezultate, korisnicke assist odluke, vlastite reference | Media Assist DB | oznaku aktivnog projekta i postavke za rad iz baze, Ingest DB, vlastitu bazu, javne artefakt reference | Timeline, Media Browser, Broadcast Player, analysis moduli, transcript/AI adapteri, UI widgeti | scan/probe, ingest workflow, pisanje u Story DB, pisanje u Ingest DB, direktno pozivanje Story aplikacije |
| Story varijanta | story odluke, virtualne kadrove ako ih koristi, markere, frame rangove, EDL/playlist odluke | Story DB te varijante | oznaku aktivnog projekta i postavke za rad iz baze, Ingest DB, vlastitu bazu, javne artefakt reference | Timeline, Media Browser, Broadcast Player, Export, UI widgeti, frame/timebase, EDL modul | scan/probe, ingest workflow, pisanje u Media Assist DB, pisanje u Ingest DB, sve Story potrebe u jednoj aplikaciji |

## Modulna ownership pravila

Modularni DB proizvod Ingesta ukljucuje `qnc.db.source_index` za potvrdene
original/proxy/support odnose (docs/30). Javnom DB adapteru owner daje
privatni binding i write ovlast; drugi korisnici citaju public viewove.
To nije novi aplikacijski workflow niti baza projektnih postavki.
Modul jos nije povezan s Ingest `Odaberi` runtimeom.

`qnc.db.media_records` cuva media snapshote i izvorne XML/JSON dokaze (docs/31).
Phase i potpunost su odvojeni podaci; DB adapter ne izvrsava probe. Ovaj
modul takoder jos nije povezan s Ingest `Odaberi` runtimeom.

Moduli su javni QNC resursi. Modul ne smije imati hardkodirani popis aplikacija
koje ga smiju koristiti. Modul smije imati dependency boundary: sto on sam ne
smije pozvati, ucitati ili pokrenuti.

Aplikacije ne poznaju druge aplikacije kao runtime dependency. Kada aplikacija
treba raditi po postavkama projekta, iz baze cita oznaku aktivnog projekta i
postavke za rad read-only. Aplikacija ne poziva Project aplikaciju, Project
store ili Project workflow.

Ne smije postojati alat za suradnju, nasljedivanje, shared runtime context ili
aplikacijski bridge koji prenosi poslovno stanje izmedu aplikacija. Jedina
poslovna veza je zapis u bazi kroz DB contract. Ingest zato nema rucno
postavljanje projektnih radnih postavki; cita ih iz baze aktivnog projekta.
QNC baza mora biti prenosivi poslovni artefakt: ako se valjana baza prenese na
drugi racunar, Ingest mora moci raditi bez Project aplikacije, QNC.app shella i
drugih QNC aplikacijskih procesa.

| Modul | Public capability | Input | Output | State/write policy | Dependency boundary |
| --- | --- | --- | --- | --- | --- |
| manifest/capability | manifest.validate, capability.list | manifest dokumenti aplikacija/modula | validation result, capability list | nema poslovnih writeova | nema workflow logike, nema media obrade |
| transport/resolver | qnc.uri.resolve, qnc.uri.validate | QNC URI, environment, access policy | resolved handle/endpoint za trenutni proces | nema poslovnih writeova | nije izvor istine, nije workflow router izmedu aplikacija |
| DB contract/validation | db.schema.validate, db.owner.check | DB URI, schema manifest, owner policy | schema validation, migration check, read/write check | samo schema/status ako owner dopusti | ne pise poslovne podatke umjesto aplikacije |
| frame/timebase | frame.convert, timecode.format | fps_num/fps_den, frame, seconds, timecode | frame/timecode/seconds konverzije | nema writeova | nema hardcoded FPS, nema probe |
| Dir Browser | dir.list, dir.select | root/location QNC URI, browse policy | directory listing, selected location URI | nema; caller pise izbor | nema media scan, nema probe, nema clip katalog |
| Media Browser | media.list, media.select | DB query/read view, filters, selection state | media rows/cards/select intent | nema; caller pise izbor | nema probe, nema mijenjanja tudeg kataloga |
| scanner | source.scan.roles | source location URI, camera/source rules | source file role map, original/proxy/support grouping | return result ili owner aplikacija pise | proxy nije clip, nema pisanja u tudje baze |
| camera detector | source.camera.detect | source tree facts, filenames, metadata sidecars | camera layout classification, role hints | nema writeova | nema probe, nema clip import |
| Media Probe | media.probe.full | original media URI i proxy URI ako postoji | puni probe record za original i proxy metadata | return result ili owner aplikacija pise | ne smije pozvati Filmstrip/Wave/Player/Export/Story workflow; proxy nije zaseban clip |
| Filmstrip | filmstrip.generate | clip id, source/proxy izbor iz DB, probe podaci iz DB | stvarni frameovi, artifact manifest/status | return result ili owner aplikacija pise artefakt | ne smije pozvati `ffprobe`, Media Probe, scanner ili Ingest workflow |
| Wave | wave.generate | clip id, audio stream/probe podaci iz DB | waveform/peaks, artifact manifest/status | return result ili owner aplikacija pise artefakt | ne smije pozvati `ffprobe`, Media Probe, scanner ili Ingest workflow |
| Broadcast Player | playback.open, playback.seek, playback.status | media refs, playlist/EDL, probe/timebase iz DB | playback status, current frame, errors | caller-owned status ili return status | radi samo playback; ne smije pozvati `ffprobe`, Media Probe, scanner, Export ili workflow druge aplikacije |
| Export | export.render, export.report | EDL/playlist/output request, DB refs | exported files, report, status | caller-owned export/status ili return result | ne smije pozvati `ffprobe`, Media Probe, scanner ili citati privatni UI state |
| Timeline | timeline.model, timeline.paint, timeline.hit_test | timeline model, frame ranges, markers, selection | painter model, navigation target, hit-test result | nema; caller pise state | nema playback clock ownership, nema probe, nema DB write |
| Monitor | monitor.preview.paint | caller-owned surface id i potvrdjeni preview frame | painted surface | nema | nije player, nije Ingest forma, nema sat/decode/DB |

## Pravilo za vise Story varijanti

Svaka Story varijanta mora imati svoj manifest i DB namespace ili svoju bazu.
Zajednicki Story moduli moraju ostati neutralni. Posebna logika jednog Story
workflowa ne smije se dodati u globalni Story monolit.

## Otvorene odluke

- Tocni nazivi baza jos nisu zakljucani.
- Treba odluciti ide li svaki Story tip u vlastiti DB file ili zajednicki Story
  DB s `story_type` i `story_instance` namespaceom.
- Pocetni audit `qnc_v4` je u
  `C:\Users\miron\Projects\QNC\docs\03-qnc-v4-app-module-audit.md`.
