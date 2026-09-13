# QNC: dubinski audit usporedjen s AGENTS.md

Datum: 2026-09-12, navecer. Zakon: `AGENTS.md` (ROOT ZAKON).
Kod nije mijenjan. Prethodni audit komponenti: `docs/79`.

Pravilo iz preambule: ako se kod i AGENTS razilaze, vrijedi AGENTS.md.

## Zakljucak

Ukupna uskladjenost s AGENTS.md: **C+ / 3.4 od 5**.

Model (javni moduli, DB-first, forma pasivna, jedan probe) je u zakonu
jasan i u kodu **vecinom namjerno** postovan. Najtezi prekrsaj zakona je
i dalje **aplikacijski umbrella**: `qnc-ingest-application` skuplja Select,
katalog, thumbnail, player, filmstrip, wave i browser. Preambula i §12 to
izricito zabranjuju, samo pod drugim imenom od zabranjenog
`qnc-ingest-components` (taj crate vise ne postoji).

Drugi prekrsaj zakona: **§16 je zastario** i proturjeci samom AGENTS.md
(Close project, Filmstrip/Wave u manifestu). Treci: **§8.3** trazi
prihvat Playa prije Filmstrip/Wave integracije; integracija je vec tu,
live prihvat Playa nije zatvoren.

Od docs/79 na bolje: forma predaje artefakte timelineu, shortcut ide kroz
javni modul, `timeline-assets` vise ne ovisi o `ingest-store`, filmstrip
ima 13 slicica i ime klipa u putanji, worker pise kroz
`ContentWriteTransport`.

## Skala

`Uskladjeno` = kod prati zakon. `Djelomicno` = namjera tocna, rupa ostaje.
`Prekrsaj` = zakon kaze jedno, kod ili §16 kaze drugo. `Zakon zastario`
= sam AGENTS.md vise ne opisuje stablo.

## 1. Matrica zakon -> kod

| AGENTS | Tema | Ocjena | Stanje |
| --- | --- | --- | --- |
| Preambula | Nema umbrella `components` | **Prekrsaj** | `ingest-components` uklonjen. `ingest-application` je isti sloj pod drugim imenom |
| §2 | Obitelj, javni moduli, nema app-to-app | Uskladjeno | Nema Story cratea; veza je baza |
| §3 | Forma pasivna; shell nije owner | Djelomicno | Forma: view + intent. Shell ima Close project preko javnog modula. Factory i dalje hardkodira dva adaptera |
| §4 | DB-first; write samo kroz writer | Djelomicno | Worker koristi `ContentWriteTransport`. Select i store i dalje zive u application crateu |
| §4.1 | Postavke iz baze, bez defaulta | Uskladjeno | work-settings; fail ako nema projekta |
| §5 | Ingest samostalan, jedan probe, katalog | Djelomicno | Select/katalog rade. Import stub. Application je monolit |
| §5.1 | JPEG + DB veza; ime klipa u diru | Uskladjeno | `filmstrip/<safe_name>/`; `clip_id` fallback. Wave u bazi |
| §6 | Jedan probe | Uskladjeno | Player/FS/wave ne zovu ffprobe |
| §7 | Original/proxy | Uskladjeno | Player-input i filmstrip plan |
| §8 | 13 slicica; auto generate; catalog decoder | Djelomicno | 13 da. Catalog da. `LinearScan` i dalje postoji. Auto sync nakon projekta/Selecta da |
| §8 | Worker ne pise DB sam | Uskladjeno | `ContentWriteTransport`, ne `ReadWrite` u generatoru |
| §8 | Wave/FS ne konkuriraju playeru | Djelomicno | `playback_priority` na generate. Select/thumb/publisher nisu u tom pravilu |
| §8.1 | Timeline pasivan | Uskladjeno | Forma zove `with_artifacts` iz view modela |
| §8.2–8.3 | Player prvi; live prihvat | **Prekrsaj redoslijeda** | FS/Wave/Timeline spojeni; Play nije prihvacen |
| §9 | URI/resolver | Djelomicno | Lokalni put jak. LAN filmstrip/player nije live |
| §10 | Shortcut katalog; UI iz contracta | Djelomicno | `consume_egui_action_presses` javni. Soft theme i dalje §16 |
| §12 | Razbiti aplikacijski sloj | **Prekrsaj** | Application crate nije smanjen |
| §13 | Manifest = stvarni moduli | Djelomicno | Manifest ima FS/Wave. §16 to jos zabranjuje |
| §14 | Project freeze | Uskladjeno* | `qnc-project-close` je javni modul. Project desktop crate nije predmet ovog audita linija-po-linija |
| §15 | Test + live | Djelomicno | Ciljani testovi postoje. Live Play nije zatvoren |
| §16 | Odstupanja | **Zakon zastario** | Vidi odjeljak 3 |

\*Close project u shell footeru je u skladu s §3 (javni `qnc-project-close`),
ne s §16 koji jos kaze da Close project nije u footeru.

## 2. Sto se poklopilo s AGENTS.md (ne dirati)

- `qnc-ingest-components` ne postoji.
- Ingest ne ovisi o Project crateovima.
- Jedan probe u Selectu.
- Player out-of-process; sat nije u formi ni timelineu.
- Filmstrip 13; putanja iz `clips.name`; decoder catalog.
- Write artefakata kroz javni writer, ne iz forme.
- Timeline prima ready-only projekciju i artefakte; emitira intent.
- Keyboard Play/step kroz javni shortcut helper, ne rucni Space parser.
- Work-settings read-only iz aktivnog projekta.

## 3. Gdje AGENTS.md i kod nisu isti

### P1. Umbrella crate (preambula + §2 + §12)

Zakon: kompletan aktivni kod u uskim javnim modulima. Aplikacijski sloj
samo composition/root i mora se smanjivati. Naziv nije izgovor.

Kod: `qnc-ingest-application` i dalje drzi dispatch, Select, katalog,
thumbnail, player, filmstrip service, wave service, browser, settings.

To je **isti prekrsaj** koji je zakon imenovao za `components`.
Preimenovanje nije uskladjenje.

### P1. §8.3 vs spojeni Filmstrip/Wave/Timeline

Zakon: tek nakon live prihvata Playa smiju ici te integracije.

Kod: integracije postoje. Play priority na generate postoji. Live kriterij
(Mironik do kraja, bez trzaja) nije zatvoren u ovom auditu.

Ako korisnik zeli zadrzati spojeni prikaz, §8.3 treba izricito azurirati.
Dok to nije, redoslijed je prekrsaj zakona.

### P1. §16 proturjeci §3, §8 i manifestu

| §16 tvrdi | Stvarnost / drugi clanak AGENTS |
| --- | --- |
| Close project nije u footeru | Jest; `qnc-project-close`; §3 to trazi |
| Filmstrip i Wave ne u Ingest manifestu dok ne postoje | Postoje i navedeni su |
| Playback nije nužno implementiran (stari Select odstupak) | Player je spojen |

§16 vise nije popis odstupanja. Vodi agente u krivu.

### P2. LinearScan (§8)

Zakon: ne citati cijeli klip linearno samo za 13 slicica; keyframe/intra.

Kod: `FilmstripExtractionMode::LinearScan` i `extract_linear` ostaju kao
put. Catalog to i dalje nudi.

### P2. Play vs ostala pozadina (§8 wave/FS + §8.2)

Zakon: worker ne smije konkurirati playeru.

Kod: generate staje. Select, thumbnail, browser i publisher nisu pod istim
guardom. To moze poremesti Play iako engine ne zna za te niti.

### P2. Shell factory (§3)

Zakon: nema `if desktop_entry == "qnc_project"` u render putu; mapa
`desktop_entry -> adapter`.

Kod: mapa postoji, ali se puni hardkodiranim `project` + `ingest`
factory pozivima. Nije switch u renderu; nije ni potpuno registry-driven
za sljedecu aplikaciju.

### P2. docs/04 (§13)

Zakon i dalje pokazuje tu matricu kao pocetni ugovor. Matrica je od
2026-09-04 i ne prati 13 slicica ni player stack. Agenti koji je citaju
krse duh §13.

### P3. Import

Zakon: produkt Ingesta je baza i artefakti; kopiranje medija nije
obavezno u §16. Stub je uskladjen s odstupanjem, ne sa §5 produktom.

## 4. Promjena od docs/79 (isti dan)

| Stavka | docs/79 | Sada vs AGENTS |
| --- | --- | --- |
| Timeline paint bez artefakata | Prekrsaj forme | **Uskladjeno** (`with_artifacts`) |
| Space parser u formi | Prekrsaj §10 | **Uskladjeno** (javni helper) |
| timeline-assets -> ingest-store | Prekrsaj §12 | **Uskladjeno** (trait reader) |
| Filmstrip 14 | Starost | **13, zakon** |
| ingest-components | Zabranjen | **Nema cratea** |
| Close project | Nije tema | Footer + javni modul; §16 laze |
| Umbrella application | 2.5/5 | I dalje **prekrsaj** |
| Play vs Select | 3/5 | I dalje **djelomicno** |

## 5. Jesu li javni moduli i dalje najbolji model?

Da, i AGENTS.md to **naredjuje**, nije opcija. Ispravak nije treci sloj
imena. Ispravak je smanjiti `ingest-application` dok samo sastavlja
vec postojece javne module i jedan Guard.

## 6. Uputa uskladjenja (redoslijed po zakonu)

1. Azurirati **§16** da odgovara stablu, ili ukloniti mrtve stavke.
   To nije odobrenje za novi kod.
2. Odluciti **§8.3**: ili pauzirati nove FS/Wave zahvate do live Playa,
   ili zakonito zapisati da je prikaz dopusten, a generate ostaje
   pod `playback_priority`.
3. **Razbiti** `ingest-application` na composition: dispatch + Guard.
   Ne preimenovati crate. Svaki thread vani vec ima modul.
4. Jedan Guard: Preparing/Playing => nema Select/probe, nema thumbnail
   batch, nema generate, nema publish poll.
5. Ukloniti `LinearScan` kao tihi put; samo catalog + keyframe/intra
   ili kontrolirana greska.
6. Shell: sljedeci adapter samo kroz registry, ne treci red u factory
   listi kao uzor.
7. `docs/04` oznaciti povijesnim ili prepisati iz 62 ugovora.
8. Project ostaje zamrznut. Close ide samo kroz `qnc-project-close`.

## Nije provjereno

- Live Play na kartici.
- Svaka linija Project desktop cratea (freeze; samo Close put u shellu).
- Stvarni LAN/Intranet write transport.
- Koliko dugo `extract_linear` traje na dugom klipu.
- Conformance prolaz ovog trenutka (nije pokrenut u ovom zahvatu).
