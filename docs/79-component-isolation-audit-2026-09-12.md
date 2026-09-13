# QNC: audit komponenti, pasivnosti formi i zastite Playa

Datum: 2026-09-12. Radno stablo `C:\Users\miron\Projects\QNC`.
Ovo je ocjena i uputa. Kod nije mijenjan.

Kontekst: agenti rade varijacije (preimenovanja, drugi sloj, pola spoja)
umjesto jednog fiksnog modela. Ovaj zapis zakljucava model i meri
odstupanja.

## Zakljucak

Univerzalni javni moduli **jesu** ispravan model za ovu vrstu proizvoda
(obitelj NLE formi, jedan player, jedan timeline, jedan generator
artefakata). Problem nije taj model. Problem je sto se on ne drzi
cvrsto: `qnc-ingest-application` je postao drugi monolit; forma je
uglavnom pasivna; Play je **djelomicno** zasticen od filmstrip/wave
generiranja, ali **nije** zasticen od Selecta, thumbnaila, browsera i
DB publisha.

Ukupna ocjena komponentnog modela: **C+**.
Ocjena zastite Playa: **3/5**.
Ocjena pasivnosti forme: **4/5**.
Ocjena "sve sto se ponavlja je javni modul": **3/5**.
Ocjena konzistentnosti agenata: **2/5**.

## 1. Cvrsti model (ne varijacija)

Cetiri sloja. Ime cratea se smije mijenjati samo ako se sloj ne mijenja.

```text
1. UGOVOR     contracts/modules + DB/transport
2. JAVNI MODUL  uska odgovornost, bez imena aplikacije u API-ju
3. ORKESTRATOR  jedna aplikacijska crate: dispatch(action_id) + zivotni ciklus
                + pravilo izolacije sesija. Ne dekodira, ne crta, ne posjeduje sat.
4. FORMA        view model + intent. Nema store, FS, probe, decode, player clock.
```

Zabranjene varijacije koje su se vec pojavile:

| Varijacija | Status u stablu | Pravilo |
| --- | --- | --- |
| `ingest-components` | conformance zabranjuje crate | ne vracati |
| `ingest-surface` / `shell-adapter` | folderi izvan workspace clana | ne uvoditi treci UI sloj |
| `filmstrip-assets` pa `timeline-assets` | drugi loader za isti posao | jedan read modul |
| Timeline API: `with_filmstrip` / `with_artifacts` / bez | forma zove najuzi API | jedan javni paint ulaz |
| Owner store kao ovisnost javnog loadera | `timeline-assets` -> `ingest-store` | samo javni DB/transport ugovor |

Ako agent treba novo ime, prvo dokaze da sloj 2 nema tu capability.
Ne smije dodati crate "da bude cistije".

## 2. Jesu li forme pasivne?

### Ingest desktop — ocjena 4/5

`qnc-ingest-desktop` crta `view` i salje `IngestIntent` s `action_id`.
Ne otvara store, ne zove probe, ne dekodira media. Timeline i UI kit
su javni paint.

Rupe:

- Space se trosi u formi posebnom petljom, usporedo s katalogom shortcuta.
  To je aktivna politika unosa u formi, ne samo paint.
- Mapiranje egui tipki na katalog (`ArrowLeft`, `Space`) je u formi, ne u
  javnom keyboard modulu.
- `show_source_player_timeline` se zove **bez** filmstrip/wave artefakata,
  iako orkestrator puni `view.timeline_assets` i ima helpere
  `timeline_filmstrip_background()`, `a1_peaks()`... Prikazni tok je
  prekinut u formi. To je klasicna agentska varijacija: loader spojen,
  paint nije.

### Project desktop — ocjena 4/5 (nije diran)

Zamrznut. Ostaje referenca: forma + javni adapter, ne store u shellu.

### Orkestrator nije forma — ocjena 2.5/5 kao "uski sloj"

`qnc-ingest-application` (~1600 linija `lib.rs` plus playback/timeline)
u istom crateu:

- Select workflow i probe (preko `ingest-select`)
- katalog i checkbox DB
- thumbnail thread
- player prepare/play
- filmstrip worker servis
- wave worker servis
- timeline asset reader
- Dir Browser sesija
- work-settings ucitavanje

To je dopusteno kao **jedini** vlasnik Ingest workflowa, ali vise nije
uski orkestrator. Svaka nova agencija dodaje jos jedan thread ovdje.
Sljedeci rez mora izdvojiti samo `dispatch` + `PlaybackGuard`, a Select,
thumbnail i artefakte zvati kao javne servise.

## 3. Je li Play zasticen od pozadine?

Player proces je izvan UI-ja (`qnc-broadcast-player`). Sat je u engineu.
To je ispravna osnova. Zastita **unutar Ingest procesa** nije potpuna.

### Sto jest zastita — 4/5 za filmstrip/wave generate

`TimelineFilmstripService` i `TimelineWaveService` imaju
`set_playback_priority`. Kad je `play_when_ready || preparing || playing`:

- `start_next()` ne pokrece nove workere
- aktivni workeri dobiju `cancel` flag
- testovi to pokrivaju

Cancel je kooperativan: decode/JPEG moze nastaviti do sljedece provjere.
`cancel_active_workers` ne radi `join` odmah, da UI ne blokira. CPU i I/O
dakle mogu jos kratko trajati nakon Play.

### Sto nije zastita — ocjena 2/5

| Pozadina | Tijekom Play | Ucinak |
| --- | --- | --- |
| Select (`ingest-select` + probe) | **nije zabranjen** | scan/probe/DB upis u istom procesu |
| Thumbnail load | **nastavlja** | I/O i decode slika kartice |
| Dir Browser listing | **nastavlja** | disk thread |
| Work-settings / katalog refresh | moze doci | `stop_player` samo na novi projekt |
| Filmstrip/wave **publisher** | `publisher.poll()` i dalje | DB write dok svira |
| UI poll `poll()` | uvijek | do 64 Select eventa po frameu |
| Repaint | `source_frame_interval` kad svira | dobro; 100 ms ako ima drugi pending |

`start_selection` ne gleda `playing()`. Korisnik moze potvrditi izvor
dok Play traje. Select `Removed` na aktivnom klipu zove `stop_player`.
To je ispravno za katalog, ali nije zastita sata: prekid je nuspojava
Selecta.

`play_when_ready` ceka Ready i ne salje lazni Play. To nije izolacija
od pozadine, nego ispravan lifecycle naredbe.

### Uputa za zastitu Playa

Uvesti **jedno** javno pravilo, ne zastavu u svakom workeru posebno.

Ime: `playback.session.priority` (modul ili metoda orkestratora).

Kad je sesija `Preparing` ili `Playing`:

1. Ne pokretati Select, probe, masovni thumbnail, filmstrip/wave generate.
2. Kooperativno otkazati vec pokrenute generate workere (vec postoji).
3. Pauzirati i publisher (sada se ne pauzira).
4. Thumbnail: samo pasivni prikaz vec ucitanih slika; ne novi batch.
5. Browser listing smije ostati ako je citanje, ne scan/probe.
6. Promjena klipa i dalje `stop_player()` prije pripreme (vec postoji).
7. Novi projekt i dalje `stop_player()` (vec postoji).

Ne stavljati ovo u formu. Ne stavljati u engine (engine ne zna za Select).

## 4. Sto mora biti univerzalni javni modul

Da. Za QNC to je najbolji model: iste kocke u Ingest, Story, Media Assist;
aplikacija samo bira sloj i pise vlastitu bazu.

### Mora ostati javno (bez imena aplikacije u API-ju)

| Modul | Svrha | Sada |
| --- | --- | --- |
| timeline | pasivni paint + intent | 5; API preklapanja |
| player-timeline | player reply -> projekcija | 5 |
| player-client / launcher / contract / frame-transport | UI remote | 4 |
| broadcast-engine + player proces | sat i decode | 4; live nije A |
| filmstrip plan + worker + TCP decode | izrada 14 JPEG | 4; worker zna ingest-store |
| wave + wave-worker + wave-view | peaks | 4; noviji, isti obrazac |
| timeline-assets | read-only artefakti | 3; ovisi o ingest-store i ingest-work-plan |
| dir-browser, ui-kit, keyboard | UI kocke | 5 / keyboard map u formi |
| work-settings, resolver, DB contract | ulaz iz baze | 5 |
| media-probe, scanner, source-* | samo Select vlasnik zove | 5 |

### Ne smije biti javni modul

- Ingest `dispatch` i Select redoslijed
- Project create/activate (zamrznuto)
- "Ingest filmstrip service" kao poseban API za druge aplikacije
- Forma koja uci clip_id ili player naredbe mimo `action_id`

### Mora se ocistiti

1. `timeline-assets` prima neutralni `ArtifactRoot { filmstrip_uri, content_read }`
   ne `IngestWorkPlan` / `ContentTarget` iz ingest cratea.
2. Forma zove **jedan** `show_source_player_timeline_with_artifacts` s
   `view.timeline` + `view.timeline_assets`.
3. Jedan generate servis obrazac za filmstrip i wave (vec skoro isti);
   `playback_priority` dolazi izvan, isti signal.
4. Ukloniti mrtve folder/imena. Workspace ne smije imati drugi Ingest UI crate.

## 5. Ocjene komponenti (kratko)

| Komponenta | Ocjena | Jedna recenica |
| --- | --- | --- |
| Ingest forma | 4 | Pasivna uz Space i prekinut timeline paint |
| Ingest application | 2.5 | Tocan vlasnik workflowa, previse niti u jednom crateu |
| Player client/proces | 4 | Izvan UI; Ready prije Playa; helper sibling |
| Broadcast engine | 3.5 | Sat ispravan; live prihvat otvoren |
| Filmstrip/wave worker | 4 | Priority postoji; publisher i Select nisu u njemu |
| timeline-assets | 3 | Pravi sloj, krive ovisnosti |
| Timeline UI | 5 | Pasivan; Ingest ga ne hrani artefaktima |
| Select/probe | 4 | Ispravan jedan probe; smije uci u Play |
| Store | 3 | I dalje filmstrip/wave sheme uz JPEG/peaks pravila |
| docs/00 i docs/04 | 2 | Nisu karta |
| Agent konzistencija | 2 | Imena i pola spojeva |

## 6. Redoslijed ispravka (cvrsto, bez varijacija)

1. **Zakljucati sloj** ovim dokumentom. Nova crate samo uz novu capability
   koje nema u `contracts/modules`.
2. **Spojiti paint**: forma predaje `timeline_assets` u jedan timeline API.
   Ne praviti `ingest-timeline` crate.
3. **PlaybackGuard u orkestratoru**: Select/thumbnail/generate/publish
   staju dok `Preparing|Playing`. Test: Play + Select ne smije pokrenuti
   probe. Test: Play postavlja cancel na filmstrip i wave.
4. **Pauzirati publisher** uz generate cancel.
5. **Ocistiti `timeline-assets` ovisnosti** od ingest owner crateova.
6. **Keyboard**: Space i strelice samo kroz javni shortcut modul.
7. Tek onda Import i daljnji generatori. Ne otvarati Project.
8. Ne zamjenjivati engine zbog pozadinskih niti.

## 7. Sto agent ne smije uraditi "da popravi"

- Preimenovati `ingest-application` u `ingest-runtime` / `ingest-core`
  bez izdvajanja odgovornosti.
- Dodati `playback_priority` u thumbnail kao kopiju, umjesto jednog guarda.
- Staviti Guard u formu ili u Broadcast Engine.
- Drugi filmstrip loader.
- HTTP za frameove ili probe u playeru.
- Spajati Story na privatni Ingest crate.

## Nije provjereno

- Live Play uz istovremeni Select na ovom stroju.
- Koliko dugo filmstrip thread zivi nakon cancel flag-a.
- Project forma linija-po-linija (freeze).
- LAN/Intranet isti Guard.
