# QNC module contract matrix

Status: pocetni module contract  
Datum: 2026-09-04

## Pravilo

Modul je alat koji aplikacija koristi. Modul nije aplikacija i nije veza izmedu
aplikacija.

Svaki modul mora imati:

```text
modul -> public capabilities -> input -> output -> state/write policy -> forbidden calls
```

Modul je javni QNC resurs. Modul ne smije imati hardkodirani popis aplikacija
koje ga smiju koristiti.

Sve sto se ponavlja u vise aplikacija i moze imati uski contract treba biti
javni modul ili javna komponenta. To smanjuje dupliciranje, ali ne smije
stvoriti novi monolit: javni modul ne smije znati Project/Ingest/Story/Media
Assist workflow niti smije postati centralni host poslovnog stanja.

Zabrana na modulu znaci samo ovo: sto modul sam ne smije pozvati, ucitati ili
pokrenuti. Primjer: Filmstrip je javan modul, ali Filmstrip ne smije pozvati
`ffprobe`, Media Probe, scanner ili Ingest workflow.

Ako neka aplikacija/forma ne smije koristiti odredeni modul ili capability, ta
zabrana se pise u manifest te aplikacije/forme. To nije lista dozvoljenih
korisnika modula.

## Modulna matrica

| Modul | Public capability | Input | Output | State/write policy | Forbidden calls / dependency boundary |
| --- | --- | --- | --- | --- | --- |
| manifest/capability | manifest.validate, capability.list | manifest dokumenti aplikacija/modula | validation result, capability list | nema poslovnih writeova | workflow logika, media obrada |
| transport/resolver | qnc.uri.resolve, qnc.uri.validate | QNC URI, environment, access policy | resolved handle/endpoint za trenutni proces | nema poslovnih writeova | workflow routing, ownership odluke, spremanje raw OS patha kao javnog ID-a |
| DB contract/validation | db.schema.validate, db.owner.check | DB URI, schema manifest, owner policy | schema validation, migration check, read/write check | samo schema/status ako owner dopusti | poslovni DB write umjesto aplikacije, preskakanje owner checka |
| frame/timebase | frame.convert, timecode.format | fps_num/fps_den, frame, seconds, timecode | frame/timecode/seconds konverzije | nema writeova | `ffprobe`, Media Probe, hardcoded FPS |
| Dir Browser | dir.list, dir.select | root/location QNC URI, owner-private start path kroz resolver/session | OS-neutral browser state, breadcrumb URI, selected location URI | session-local; caller pise izbor | egui dugmad, media scan, probe, clip katalog |
| Workstation Identity | workstation.identity.read | poziv na izvornoj radnoj stanici, bez poslovnog stanja | versioned JSON: naziv stanice, lokalni korisnik, device/CPU serial, razlog nedostupnosti | stateless; caller owner sprema snapshot | DB write, aplikacijski workflow, remote discovery, elevation; nije autentikacija |
| Media Browser | media.list, media.select | DB query/read view, filters, selection state | media rows/cards/select intent | nema; caller pise izbor | probe, clip kreiranje, mijenjanje tudeg kataloga |
| scanner | source.scan.roles | source location URI, camera/source rules | source file role map, original/proxy/support grouping | return result ili owner aplikacija pise | Media Probe, Filmstrip, Wave, Story workflow, proxy kao zaseban clip |
| camera detector | source.camera.detect | source tree facts, filenames, metadata sidecars | camera layout classification, role hints | nema writeova | probe, clip import, poslovne odluke aplikacije |
| Media Probe | media.probe.full | original media URI i proxy URI ako postoji | puni probe record za original i proxy metadata | return result ili owner aplikacija pise | Filmstrip, Wave, Player, Export, Story workflow, Media Assist workflow, proxy kao zaseban clip |
| Filmstrip | filmstrip.generate14 | clip id, source/proxy izbor iz DB, probe podaci iz DB | 14 stvarnih frameova, artifact manifest/status | return result ili owner aplikacija pise artefakt | `ffprobe`, Media Probe, scanner, Ingest workflow, poster repeat kao filmstrip |
| Wave | wave.generate | clip id, audio stream/probe podaci iz DB | waveform/peaks, artifact manifest/status | return result ili owner aplikacija pise artefakt | `ffprobe`, Media Probe, scanner, Ingest workflow |
| Timeline | timeline.model, timeline.paint, timeline.hit_test | timeline model, frame ranges, markers, selection | painter model, navigation target, hit-test result | nema; caller pise state | playback clock ownership, probe, DB write, poslovni workflow aplikacije |
| Broadcast Player | playback.open, playback.seek, playback.status | media refs, playlist/EDL, probe/timebase iz DB | playback status, current frame, errors | caller-owned status ili return status | `ffprobe`, Media Probe, scanner, Export, app-to-app state sharing |
| Export | export.render, export.report | EDL/playlist/output request, DB refs | exported files, report, status | caller-owned export/status ili return result | `ffprobe`, Media Probe, scanner, privatni UI state |
| Monitor | monitor.status | output target URI/config | output/health/status events | caller-owned status ili return status | centralni workflow host, workflow routing |
| UI widget / `qnc-ui-kit` | ui.render, ui.intent, ui.form_action_bar, ui.exclusive_panel | view model, theme/font policy, action labels, pasivni UI open/close state | rendered UI, UI intent, genericki UI-state rezultat | nema poslovnih writeova | scan, probe, media obrada, DB write, poslovni workflow, browser session ownership |
| test adapter | conformance.run | manifest, source tree, DB files, fake transport | conformance result | test output samo | produkcijski DB write, skrivanje pravila u test helperu |

Napomena za forme s vise browser polja: `qnc-dir-browser` daje session/listing
state po instanci, ali ne zna koliko ih aplikacija prikazuje. Aplikacijska
komponenta ili forma mora osigurati da otvaranje jednog browser panela zatvori
drugi panel u istoj formi.

## Workflow i dependency zabrane

Primjeri aplikacijskih workflow zabrana:

```text
Story aplikacija:
  ne pokrece media.probe.full
  ne pokrece source.scan.roles
  ne pokrece Ingest workflow

Media Assist aplikacija:
  ne pokrece media.probe.full
  ne pise Story DB
  ne poziva privatni Story workflow
```

Primjeri modulnih dependency zabrana:

```text
Filmstrip modul:
  ne poziva ffprobe
  ne poziva Media Probe
  ne poziva scanner
  ne poziva Ingest workflow

Broadcast Player modul:
  ne poziva ffprobe
  ne poziva Media Probe
  ne pokrece Export
```

## Kako dvije aplikacije koriste isti modul

Primjer s Timeline modulom:

```text
Story varijanta
  -> salje svoj Story timeline model u Timeline modul
  -> dobiva painter/navigation rezultat
  -> pise samo Story DB

Media Assist
  -> salje svoj Media Assist timeline/view model u isti Timeline modul
  -> dobiva painter/navigation rezultat
  -> pise samo Media Assist DB
```

Timeline modul ne zna za privatni workflow nijedne aplikacije i ne pise u
njihove baze.

## Redoslijed implementacije modula

1. manifest/capability
2. transport/resolver
3. DB contract/validation
4. frame/timebase
5. Dir Browser
6. scanner + camera detector
7. Media Probe
8. Media Browser
9. Filmstrip
10. Wave
11. Timeline
12. Broadcast Player
13. Export
14. Monitor

Ovaj redoslijed se smije promijeniti samo ako je ugovor modula vec definiran i
ako promjena ne otvara app-to-app vezu mimo baze.
