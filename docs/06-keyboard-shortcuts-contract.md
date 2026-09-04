# QNC keyboard shortcuts contract

Status: pocetni shortcut ugovor  
Datum: 2026-09-04

## Pravilo

QNC keyboard shortcuts su obvezna vanjska datoteka:

`C:\Users\miron\Projects\QNC\contracts\qnc-keyboard-shortcuts.json`

Pocetni izvor te datoteke je postojeci QNC v4 katalog:

`C:\Users\miron\Projects\qnc_v4\seed\keyboard-shortcuts.json`

Ta datoteka je izvor istine za shortcut mapiranje. UI, aplikacije i moduli ne
smiju hardkodirati shortcut tipke direktno u kod.

## Model

Shortcut se veze na stabilni `action_id`, ne na privatnu funkciju, widget ili
lokalni event handler.

```text
user input -> keymap/shortcut modul -> action_id -> aplikacijski intent
```

Kod mora:

- ucitati vanjsku shortcut datoteku
- validirati `version`
- traziti shortcut preko `action_id`
- podrzati OS razlike samo kroz tu datoteku
- odbiti ili prijaviti shortcut koji nije definiran u contractu

Kod ne smije:

- hardkodirati `Ctrl+...`, `Cmd+...`, funkcijske tipke ili kombinacije u UI kod
- definirati razlicite shortcut izvore po aplikacijama
- vezati shortcut direktno na privatni kod druge aplikacije
- koristiti raw OS path kao javni identitet shortcut contracta

## Modul

Keyboard/Shortcut je modul, ne aplikacija.

Public capabilityji:

```text
keyboard.shortcuts.load
keyboard.shortcuts.validate
keyboard.shortcuts.resolve
keyboard.shortcuts.list
```

Forbidden calls / dependency boundary:

```text
Keyboard/Shortcut modul:
  ne poziva aplikacijski workflow
  ne pise u aplikacijske baze
  ne pokrece media obradu
  ne preuzima vlasnistvo nad UI stanjem
```

## Conformance

Testovi moraju provjeriti:

- postoji obvezna shortcut datoteka
- shortcut datoteka ima validan schema version
- nema shortcut kombinacija hardkodiranih u aplikacijskom ili modulnom kodu
- svaka UI akcija koja koristi shortcut ima `action_id` u shortcut contractu
- OS specificni overridei postoje samo u shortcut datoteci
