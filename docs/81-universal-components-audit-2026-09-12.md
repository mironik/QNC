# Potpuni audit: forma je UI, kod je u uskim javnim komponentama

Datum: 2026-09-12. Zakon: `AGENTS.md` + korisnicko pravilo ovog zahtjeva.

Kod nije mijenjan.

## Korisnicko pravilo (mjerilo ovog audita)

1. Forma je samo UI/layout.
2. Sav aktivni kod zivi u **univerzalnim javnim komponentama**.
3. Ista komponenta ide u Ingest, Story, Media Assist ili buducu formu
   **bez izmjene te komponente**.
4. Komponenta nije mali monolit.
5. Aplikacija nije monolit niti "umbrella" sloj pod drugim imenom.

Ovo nije drugaciji zakon od AGENTS preambule i §2/§12. Pojašnjenje:
**univerzalno** znaci da API i implementacija ne znaju za formu. Ne znaci
da svaka forma smije pokrenuti svaki capability. Probe i Select ostaju
zabrana Storyja (§6, §12). Ta zabrana pise se na **aplikaciji**, ne kao
`if ingest` unutar modula.

Ako Story treba filmstrip, zove isti `qnc-filmstrip-worker` kao Ingest.
Ako worker treba `IngestWorkPlan` ili `qnc-ingest-select`, pravilo je
prekršeno.

## Zakljucak

Ukupna ocjena prema ovom pravilu: **C / 2.8 od 5**.

Javnih uskih crateova ima dovoljno. Lanci ovisnosti ih **vezuju uz Ingest**.
`qnc-ingest-application` je aplikacijski monolit. `qnc-ingest-select` je
valjan Ingest workflow, ali predebeo. `filmstrip-worker` i `wave-worker`
nisu univerzalni dok u Cargo.toml imaju `qnc-ingest-*`.

Forma je blizu pravila (ocjena **4.2/5**). Aplikacija i "javni" generatori
nisu.

Play Guard postoji i pokriva vise nego u docs/79, ali je **privatni** kod
Ingesta s Ingest `action_id` listom. Story ga ne moze uzeti.

## 1. Ciljni model (zakljucan, bez varijacija imena)

```text
FORMA (samo layout)
  crta view
  šalje action_id
  ne zove store, decode, probe, generate, player clock

APLIKACIJSKI SASTAV (tanak)
  dispatch(action_id) -> javni moduli
  zivotni ciklus sesije
  workflow zabrane TE aplikacije
  nema decode, nema paint, nema drugi sat

JAVNI MODULI (isti u svakoj formi)
  jedan posao, ugovor, trait ulaz
  ne uvoze ime ni crate druge aplikacije
  owner store IMPLEMENTIRA trait; modul ne ovisi o owner crateu

OWNER STORE (po aplikaciji)
  ingest-store, project-store
  jedini write SQL
  nije javni modul za Story
```

Test univerzalnosti: `cargo tree -p qnc-filmstrip-worker` ne smije
sadrzavati `qnc-ingest-store`, `qnc-ingest-select`, `qnc-ingest-work-plan`.
Isto za wave-worker i player-input. Ako Story doda ovisnost samo na
worker i proprijeti trait, worker se ne rekompajlira zbog Ingest izmjene.

Zabranjeno: preimenovati `ingest-application` u `ingest-runtime` i
ostaviti iste ovisnosti. To je varijacija, ne popravak.

## 2. Inventar crateova

Klasa:

- **U** — univerzalan; Story bi ga koristio bez izmjene
- **U-kontaminiran** — zove se javnim, ali ovisi o Ingest crateu
- **W** — uski aplikacijski workflow (samo ta aplikacija; i dalje nije monolit)
- **O** — owner store
- **A** — aplikacijski sastav / forma / adapter
- **M** — mali ili veliki monolit (prekrsaj)

| Crate | Klasa | Ocjena | Smije li Story danas? |
| --- | --- | --- | --- |
| qnc-timeline | U | 5 | Da, isti paint |
| qnc-player-timeline | U | 5 | Da |
| qnc-timeline-assets | U | 4.5 | Da (trait reader) |
| qnc-ui-kit | U | 5 | Da |
| qnc-dir-browser | U | 5 | Da |
| qnc-keyboard-shortcut | U | 5 | Da |
| qnc-work-settings | U | 5 | Da |
| qnc-transport-resolver, contracts, db-contract, json-transport | U | 5 | Da |
| qnc-frame-timebase, media-*, source-*, scanner, sony, camera-* | U | 4–5 | Da, osim sto Story ne smije zvati probe/scan |
| qnc-player-contract/client/launcher/frame-transport | U | 4.5 | Da |
| qnc-broadcast-player/engine, decode, audio/video out | U | 4 | Da |
| qnc-filmstrip, qnc-wave, qnc-wave-view | U | 4.5 | Da |
| qnc-decoder-catalog, image-assets, media-thumbnail | U | 4 | Da |
| qnc-project-close | U | 4 | Nije Story posao; API je uski |
| **qnc-filmstrip-worker** | **U-kontaminiran** | **2** | Ne: `ingest-select/store/work-plan` |
| **qnc-wave-worker** | **U-kontaminiran** | **2** | Isto |
| **qnc-player-input** | **U-kontaminiran** | **2.5** | Ne: `ingest-store` |
| qnc-ingest-work-plan | W lošeg imena | 3 | To je projektni raspored; ime Ingest |
| qnc-ingest-catalog | W | 4 | Samo Ingest katalog |
| qnc-ingest-select | **W + M** | **2.5** | Samo Ingest; u sebi scan+probe+vise DB |
| qnc-ingest-store | O | 4 | Ne; owner |
| **qnc-ingest-application** | **A + M** | **1.5** | Ne; svi tokovi u jednom crateu |
| qnc-ingest-desktop | A forma | 4.2 | UI |
| qnc-ingest-desktop-adapter | A | 4 | Shell ulaz |
| qnc-project-* | A/O zamrznuto | — | Ne dirati |

Broj crateova nije problem. Problem su **strelice prema Ingest owneru**
iz navodno javnih generatora i ulaza.

## 3. Forma

`qnc-ingest-desktop`: crta `IngestViewModel`, zove javni timeline
`with_artifacts`, šalje `action_id`. Shortcut kroz javni helper.
`action_enabled` gleda view, ne store.

Rupe: layout i chrome su veliki (Ingest-specific, to je dopusteno).
Ne smije dobiti Guard, Select ili generate.

Ocjena forme: **4.2/5**. Blizu pravila.

## 4. Aplikacija kao monolit

`IngestApplication` (~1700 linija + playback + guard + artifacts):

- settings i katalog
- Dir Browser sesija
- Select thread
- thumbnail thread
- player prepare/play
- filmstrip i wave servisi
- view model i action_id katalog
- PlaybackGuard

Zakon i korisnik: sastav smije orkestrirati, ne smije **sadrzavati** te
poslove. Danas ih sadrzi. Guard je korak naprijed i krivo mjesto:
privatni `action_ids::INGEST_*` umjesto javnih capabilityja.

Ciljna debljina sastava: jedan `dispatch` + `poll` koji zove vanjske
servise. Redoslijed odluke (što smije dok svira) je javni
`playback.session.priority`, ne Ingest enumeracija.

Ocjena: **1.5/5**.

## 5. Kontaminirani "javni" moduli

### filmstrip-worker i wave-worker

`FilmstripContext` / `WaveContext` nose `IngestWorkPlan`,
`SelectionConfig`, `ContentTarget`. Cargo vuce cijeli Select stack.

Story bi morao ovisiti o Ingest Select da generira filmstrip. To je
app-to-app veza preko cratea, zabranjena §4 i ovim pravilom.

Ciljni ulaz:

```text
ArtifactJob {
  project_id, clip_id,
  media_facts: saved snapshot (trait/read),
  artifact_root_uri,
  source_bindings: URI -> stream (isti kao player),
  write: ContentWritePort,   // trait, ne ingest-store
  playback_priority: bool,
}
```

Ime cratea ostaje. Mijenja se samo granica ovisnosti.

### player-input

Cita clip preko `qnc-ingest-store::ContentTarget`. Player ulaz mora ici
kroz javni media-record / content **read** ugovor, ne owner crate.

### ingest-work-plan

Raspored `original/proxy/filmstrip` vrijedi za svaku aplikaciju (§5.1).
Ime i modul_id `ingest-work-plan` lazu da je Ingest-only. Smjernica:
preimenovati **ugovor** u `project-layout` kad se dira, ili ostaviti ime
ali maknuti Ingest iz API tipova (`WorkPlan`, ne `IngestWorkPlan`).
To nije hitnije od inverzije ovisnosti workera.

### ingest-select

Jedini vlasnik jednog probe prolaza. To je W, ne U. Ipak je **M**:
detector, scanner, sony, probe, compose, index-db, record-db, store
writer u jednom crateu. Smjernica: ostaje Ingest-only, ali se cijepa na
orkestraciju `source.select` koja zove vec postojece uske module. Ne
postaje univerzalan probe za Story.

## 6. Play i pozadina (stanje 2026-09-12 navece)

Postoji `playback_guard`:

- active = play_when_ready || preparing || playing
- gasi thumbnail i Select/browser kad je aktivan
- blokira reload, dir, select-all/toggle i sl.
- filmstrip/wave i dalje imaju `playback_priority`

To je bolje nego docs/79. I dalje nije univerzalna komponenta. Publisher
nije nužno pauziran u Guardu (priority na workeru). Guard poznaje Ingest
akcije, ne capabilityje.

Ocjena zastite Playa: **3.7/5** kao Ingest zakrpa; **2/5** kao javni model.

## 7. Uskladjenost s AGENTS.md (kratko)

| Pravilo | Ovaj rez |
| --- | --- |
| Forma = UI | Uskladjeno uz sitnice |
| Nema umbrella | **Prekrsaj**: application |
| Javni modul bez app imena u API-ju | **Prekrsaj**: worker context |
| Jedan probe | Uskladjeno (Select only) |
| Worker ne pise SQL sam | Uskladjeno (ContentWriteTransport) |
| §8.3 Play prije FS/Wave | Integracija postoji; live nije zatvoren |
| §16 | I dalje zastario |

Korisnikovo pravilo je **stroze** od "imamo 60 crateova". Broj crateova
ne dokazuje univerzalnost.

## 8. Smjernice popravka (cvrsti redoslijed)

Ne raditi sve odjednom. Svaki korak ostavlja Story mogucnost da doda
ovisnost na isti modul.

### Korak A — inverzija ovisnosti (obavezno prvo)

1. Trait `MediaCatalogRead` / `ContentWritePort` u neutralnom crateu
   (media-records ili novi uski `qnc-content-port`, samo traitovi).
2. `ingest-store` implementira trait.
3. `filmstrip-worker`, `wave-worker`, `player-input` ovise samo o traitu
   + work-settings + media-metadata + stream.
4. Conformance: zabraniti `qnc-ingest-store` u tim Cargo.toml.

Bez ovoga nijedna forma osim Ingesta ne moze koristiti generate/play
ulaz. To je korisnikovo pravilo.

### Korak B — javni PlaybackPriority

Izdvojiti `qnc-playback-priority` (ili capability na player-client):

- `active: bool` iz player view + queued play
- `set_priority(bool)` na filmstrip/wave/thumbnail servisima
- blokiranje capabilityja (`source.scan`, `catalog.mutate`, `dir.list`
  ako smeta), ne liste `ingest_dir_*`

Ingest sastav samo prosljedjuje. Forma ne zna za Guard.

### Korak C — smanjiti application

Nakon A i B, `IngestApplication` samo:

- drzi handle servisa
- `dispatch` / `poll`
- slaže view model iz odgovora modula

Thumbnail, Select, browser, player, FS, wave vise nisu polja s
privatnom logikom u istom `lib.rs`; samo klijenti javnih servisa.

Ne preimenovati crate dok se ovisnosti ne smanje. Zatim smije ostati
`qnc-ingest-application` kao tanki sastav.

### Korak D — ingest-select ostaje W, cijepa se

Orkestrator Select zove postojece U module. Ne vuce se u worker.

### Korak E — forma dirana samo za layout

Nova forma (Story) = novi layout crate + tanki sastav. Nula izmjena u
timeline, player, filmstrip-worker, wave-worker.

### Korak F — zakon

Azurirati §16. Zapisati korisnicko pravilo vec postoji; dodati conformance
test `no_ingest_crate_in_public_generators`. §8.3 odluka ostaje korisnikova.

Sto **ne** raditi:

- Novi `qnc-ingest-runtime` / `core` / `host`
- Drugi filmstrip loader
- Story ovisnost na `qnc-ingest-application`
- Guard u formi ili u broadcast-engine
- Otvaranje Projecta
- LinearScan kao tihi fallback (§8)

## 9. Ciljni dijagram (nakon popravka)

```text
Ingest forma          Story forma
     |                     |
Ingest sastav          Story sastav
     |                     |
     +-- dir-browser ------+
     +-- timeline ---------+
     +-- player-client ----+
     +-- filmstrip-worker -+   (trait port)
     +-- wave-worker ------+
     +-- timeline-assets --+
     |
     +-- source-select (samo Ingest)
     +-- ingest-store  (samo Ingest owner)
```

Danas strelice idu `worker -> ingest-store/select`. To treba okrenuti.

## Nije provjereno

- `cargo tree` ispis (ocjena je iz Cargo.toml).
- Live Play uz Guard.
- Project desktop linija-po-linija (freeze).
- Je li ContentWriteTransport vec dovoljno trait ili je jos ingest-store tip.
