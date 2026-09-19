# 95 — Jedinstveni obrazac naredbi za označavanje i uređivanje (pravilo)

Pravilo korisnika: naredbe za **sve** objekte moraju biti ujednačene, i za sadašnje i za buduće objekte (IN, OUT, M marker, dalje…). Izvor obrasca: v5 (`Story`: `mark_in`, `mark_out`, `select_mark_in`, `select_mark_out`, `nudge_in/out`, `clear_focus`); v5 ga je dovršio za IN/OUT, a za M marker je ostao nespojen (prečac `select_marker` = `Ctrl+M` postoji u katalogu bez radnje).

## 1. Obrazac (za svaki objekt X)

| Korak | Naredba | Učinak |
|---|---|---|
| Postavi | tipka objekta (`I` = IN, `O` = OUT, `M` = M marker) | stvori/postavi X na playhead |
| Otvori kontrolu | `Ctrl` + ista tipka (`Ctrl+I`, `Ctrl+O`, `Ctrl+M`) | fokus na postojeći X; playhead skače na X; ako X ne postoji, poruka „Prvo stavi X (tipka)” |
| Uredi (u fokusu) | ←/→ = ±1 frame; **povlačenje** pina/oznake; ili **novi položaj playheada + ista tipka** | premješta fokusirani X |
| Izlaz | `Esc` (`clear_focus`) | fokus se vraća na playhead |
| Brisanje | `Delete` / `Backspace` | briše fokusirani/odabrani X (osim zaključanih) |

Ista tipka ima dva značenja ovisno o stanju: **bez fokusa postavlja/stvara, u fokusu premješta.** Tako korisnik ne uči novu naredbu za novi objekt.

Sve tipke dolaze **isključivo iz kataloga prečaca** (`contracts/qnc-keyboard-shortcuts.json`), nikad ugrađene u formu; za svaki objekt katalog ima par radnji `mark_<x>` i `select_<x>`.

## 2. Pravila po objektu (granice pomaka)

| Objekt | Ograničenje pri uređivanju |
|---|---|
| IN | ne smije prijeći OUT (`IN ne smije prijeći OUT`); ≥ 0 |
| OUT | mora ostati ≥ IN + 1; ≤ kraj klipa |
| M marker | u `[0, trajanje]`; ne prelazi susjedne markere; nema dva na istom frameu; početni (frame 0) i završni zaključani |
| budući objekt | isti obrazac, granice definira vlasnik objekta |

Uređivanje je **jedan zapis** (povlačenje: pretpregled tijekom povlačenja, zapis pri otpuštanju), a rezultat se vraća iz baze; forma ne drži logiku.

## 3. Kako to gradimo (univerzalna komponenta)

- Javni modul `qnc-edit-focus`: čisto stanje i pravila obrasca (koji je objekt u fokusu, što tipka znači u kojem stanju, što ←/→/Esc rade). Bez UI-ja i baze.
- Svaki objekt se registrira svojim „adapterom” (kako se postavlja, čita položaj, provjerava granica, zapisuje). Novi objekt = novi adapter, ista komponenta.
- Timeline komponenta samo crta fokus i emitira namjere (klik, povlačenje, ±frame); ne odlučuje.
- Rezultat je namjera prema aplikacijskom sloju (`SetObject`, `MoveObject`, `ClearFocus`), a zapis ide kroz write transport vlasnika objekta.
- Vrijedi u samostalnoj aplikaciji i u shellu; primjenjuje se i na Ingest/Media Assist forme kad dobiju iste objekte (IN/OUT).

## 4. Zaključak za razvoj

1. Uvodi se `Ctrl+M` → fokus na marker (najbliži playheadu; ako je playhead na markeru, taj), ←/→ ±1 frame, povlačenje, `M` na novom položaju.
2. IN/OUT već rade tako u v5; ponašanje se preuzima bez izmjene.
3. Povlačenje na timelineu dobiva zajedničku komponentu (v5 ima samo klik i pomak playheada).
