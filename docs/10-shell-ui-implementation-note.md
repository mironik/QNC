# QNC shell UI implementation note

Status: prvi shell korak  
Datum: 2026-09-04

## qnc_v4_reference

```text
C:\Users\miron\Projects\qnc_v4\qnc-app\src\app.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\qnc_theme.rs
C:\Users\miron\Projects\qnc_v4\qnc-app\src\components\theme_picker.rs
C:\Users\miron\Projects\qnc_v4\seed\tabs\project\plugin.json
C:\Users\miron\Projects\qnc_v4\seed\tabs\ingest\plugin.json
C:\Users\miron\Projects\qnc_v4\seed\tabs\media_assist\plugin.json
C:\Users\miron\Projects\qnc_v4\seed\tabs\story\plugin.json
```

## kept_behavior

- Shell footer ostaje na dnu.
- Footer ima tri zone: lijevo tema, sredina aplikacijski tabovi, desno status.
- Footer tabovi dolaze iz QNC app registryja, ali zadrzavaju qnc_v4 stil taba i
  underline ponasanje.
- Aktivni tab ima QNC underline.
- Koristi se jedan QNC font sustav i boje iz `contracts/ui/shell.layout.json`.

## changed_behavior

- Odobreno 2026-09-06: jednak unutarnji razmak lijevo/desno u shell baru,
  prema postojecem `theme_metrics.chrome_pad_x` (8 egui tocaka) iz shell
  layout contracta. V4 footer s tri zone ostaje referenca. Mijenja se samo
  horizontalni unutarnji okvir; visina, font, tabovi i okomiti razmaci ostaju.
  Razmak vrijedi jednako za sve aplikacije hostane u shellu.
  Provjereno: 11 shell testova prolazi; nova Windows izgradnja i live prikaz
  potvrduju odmak teksta `Tema` i naziva `projekt 6` od oba vanjska ruba.

- Novi `qnc-app` otvara aplikacije unutar svojeg QNC desktop prostora.
- Shell vise ne hardkodira runtime popis aplikacija iz layouta.
- Runtime popis aplikacija cita se iz `apps/*/qnc-app.json`.
- Klik na tab aktivira pripadnu aplikacijsku komponentu ako postoji.
- Embedded aplikacija ulazi u shell samo preko javnog desktop adapter cratea.
  Standalone app crate ostaje zaseban executable wrapper.
- Trenutno postoji samo `apps/qnc-project/qnc-app.json`, pa runtime footer
  prikazuje samo Project. Ingest, Media Assist i Story dodaju se tek kada budu
  stvarne aplikacije s vlastitim manifestom.
- `qnc-project` vise ne crta vlastiti shell footer.
- `Close project` nije prikazan dok ne postoji ispravan workspace/DB close
  contract.
- Workspace status visine 22 px iz `shell.layout.json` jos nije prikazan kao
  zaseban red u ovom prvom shell koraku. Desni status u footeru je privremeno
  odstupanje dok se ne definira workspace status contract.
- Project `Odaberi...` za `Lokacija projekata:` i `Export direktorij:` sada
  otvara ugradeni location browser u Project formi, kao v4 Project. Browser ima
  `Racunalo / LAN / Internet`, `Gore`, `Diskovi`, `U redu` i `Odustani`.
- LAN i Internet u prvom rezu ne izmisljaju izvore; prikazuju prazno stanje dok
  ne postoji registry/authority contract. Lokalno listanje ne radi media scan,
  probe, filmstrip ni waveform.

## reason_for_change

Novi QNC model tretira Project, Ingest, Media Assist i Story kao zasebne
aplikacije koje shell prikazuje unutar QNC desktopa. Shell smije hostati javni
aplikacijski UI/component ulaz, ali ne smije preuzeti workflow aplikacije niti
importati njezine privatne module. Ovo sprjecava novi monolit.

App registry manifest je shell "kvazi include": opisuje sto shell smije
prikazati i kako se javni desktop entry aplikacije zove. Manifest nije project
template i ne sadrzi poslovnu logiku.

Kod granica za trenutni Project:

```text
apps/qnc-project                  standalone executable
crates/qnc-project-desktop         javna Project desktop povrsina
crates/qnc-project-store           Project owner/store i DB/resolver runtime
crates/qnc-project-desktop-adapter shell adapter za desktop_entry=qnc_project
```

Shell smije ovisiti o adapteru i `qnc-shell-desktop-api`, ali ne o
`apps/qnc-project` niti direktno o `qnc-project-store`.

## live_test_scope

Pokrenuti:

```text
C:\Users\miron\Projects\QNC\target\debug\qnc-app.exe
```

Ocekivano:

- vidi se QNC shell prozor s donjim footerom
- lijevo je tema
- u sredini je Project tab iz `apps/qnc-project/qnc-app.json`
- centralni desktop prikazuje Project aplikaciju unutar shell prozora
- ne otvara se zaseban `qnc-project.exe` prozor
- `Odaberi...` na `Lokacija projekata:` i `Export direktorij:` otvara ugradeni
  browser u istoj formi, ne OS folder dialog
- nove aplikacije se dodaju tek vlastitim standalone executableom,
  `apps/*/qnc-app.json` manifestom i javnim desktop adapterom
- workspace status od 22 px nije zaseban red u ovom koraku i to ostaje
  zabiljezeno odstupanje

## visual_match_expected

Footer mora pratiti `qnc_v4` raspored: tri zone, centrirani tabovi, QNC
underline i isti osnovni font. Jedino dopusteno odstupanje u ovom koraku je
to sto su aplikacije zasebne QNC komponente hostane u desktop prostoru shella,
a ne stari monolitni tabovi, te sto runtime footer prikazuje samo aplikacije
koje stvarno imaju registry manifest.
