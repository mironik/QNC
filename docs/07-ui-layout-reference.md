# QNC UI/layout reference contract

Status: pocetni UI/layout ugovor  
Datum: 2026-09-04

## Pravilo

Postojeci UI i layout iz `C:\Users\miron\Projects\qnc_v4` obvezna su referenca
za novi QNC i moraju se doslovno preslikati.

Prije kodiranja bilo koje forme, panela, dialoga, browsera, timelinea, player
kontrole ili shell prikaza treba provjeriti relevantni stari UI/layout.

UI/layout se mora koristiti kao doslovni vizualni i strukturni baseline.
Raspored, redoslijed elemenata, nazivi, font, razmaci, poravnanja, fokus,
keyboard ponasanje i stanja prikaza moraju ostati isti.

Redizajn, reinterpretacija ili uljepsavanje layouta nisu dozvoljeni bez
izricitog odobrenja.

UI kod i layout kod smiju se koristiti za doslovno preslikavanje. Aktivna
poslovna logika se ne smije prenijeti bez razdvajanja na aplikaciju/modul i bez
jasnog odobrenja.

## Obvezni referentni izvori

Shell i aplikacijski okvir:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\main.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\app.rs
C:\Users\miron\Projects\qnc_v4\seed\tabs\*\plugin.json
```

Project:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\project\*
C:\Users\miron\Projects\qnc_v4\qnc-app\src\components\project_catalog.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\components\project_command.rs
```

Ingest:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\ingest\mod.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_location_browser.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\media_assets.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\components\source_import_command.rs
```

Media Assist:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\media_assist.rs
```

Story i editorial:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\story.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\story\*
C:\Users\miron\Projects\qnc_v4\qnc-app\src\editorial.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\editorial\*
```

Vidljivi media moduli:

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_timeline.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_segment_timeline.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_timeline_progress.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_broadcast_player.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_filmstrip_background.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\editorial\media_pool.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\editorial\program_waveform.rs
```

Keyboard/focus:

```text
C:\Users\miron\Projects\qnc_v4\seed\keyboard-shortcuts.json
C:\Users\miron\Projects\qnc_v4\qnc-app\src\shortcuts.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\components\shortcut_bindings.rs
```

## Sto se mora usporediti

- raspored elemenata
- poravnanja
- razmaci
- panel struktura
- tab/form navigacija
- font i vizualni stil
- keyboard fokus
- keyboard shortcut ponasanje
- prazna/loading/error stanja
- live test ponasanje
- pixel/visual podudarnost gdje je moguce

## Prije implementacije

Za svaku UI/layout promjenu treba zapisati:

```text
qnc_v4_reference
kept_behavior
changed_behavior
reason_for_change
live_test_scope
visual_match_expected
```

Ako relevantni stari UI postoji, novi UI se ne smije kodirati iz sjecanja.
Ako novi UI nije doslovna preslika, odstupanje mora biti unaprijed zapisano i
odobreno.

## Konkretni UI contracti

Trenutno zamrznuti UI contracti:

```text
contracts/ui/shell.layout.json
contracts/ui/project.layout.json
contracts/ui/ingest.layout.json
```

Project audit je zapisan u:

```text
docs/08-project-ui-reference-audit.md
```

Ingest audit i prijedlog prekodiranja:

```text
docs/11-ingest-ui-reference-audit.md
docs/12-ingest-recoding-proposal.md
```

Buduci UI kod mora koristiti ove contracte kao izvor geometrije, naziva i
redoslijeda elemenata. Ako se dodaje Media Assist, Story ili druga
forma, prvo se radi isti tip qnc_v4 layout audita i tek tada se pise UI kod.
