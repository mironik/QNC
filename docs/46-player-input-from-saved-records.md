# Broadcast Player: javni read-only ulaz iz baze

Datum: 2026-09-08. Nastavak docs/45. Ovaj korak priprema opis izvora za
player, ne izvrsava reprodukciju. Nema promjene UI-ja niti integracije s
timelineom, monitorom, filmstripom ili waveom.

## Provjeren postojeci put

Prije implementacije procitan je stvarni aktivni zapis u globalnom registru
i `public_project_settings` u njegovoj projektnoj bazi. Aktivni projekt je
`novi-cjeloviti-1`, sa 98 javnih clip zapisa i `playback.input=proxy_if_available`.
Postavke i original/proxy metadata vec postoje; nisu dodavana nova Project polja.

V4 referenca: `AGENTS.md` (playback.input i Jedinstveni model),
`qnc-host/src/media/play.rs`; zahtjevi iz docs/24. Ne prenosi se privatni
Project lookup, raw path playback ulaz ili pretpostavka da prvi audio stream
predstavlja sve kanale izvora.

Put novog modula:

```text
DB oznaka aktivnog projekta
  -> qnc-work-settings::SettingsReader (read-only)
  -> zapisane WorkSettings + workspace_db_uri
  -> ContentTarget / javni qnc-ingest-store::content (Access::ReadOnly)
  -> Read { clip_id } -> spremljeni CatalogClip/Snapshot
  -> qnc-player-input::InputReader -> PreparedInput
```

`qnc-player-input` nema ovisnost o aplikacijama, player jezgri, UI-ju,
source readeru, scanneru, probeu ili generatorima. Public DB adapter
poznaje njegov DB ugovor, ne workflow Ingest aplikacije. Jezgra playera
i ugovor iz docs/45 nisu dobili DB ili app ovisnosti.

## Ugovor

- `InputReader::load(workspace_uri, clip_id)` cita postavke i jedan klip.
  Ocekivani workspace URI dolazi iz vec procitanih DB postavki pozivatelja;
  ne moze se koristiti za aktivaciju ili odabir proizvoljnog projekta.
  Drugi aktivni projekt odbija se prije clip dohvata. Nakon dohvata ponovo
  se provjerava snapshot postavki; promjena tijekom citanja odbija ulaz.
  To nije lease niti obecanje da se projekt ne moze promijeniti kasnije.
- Zajednicki `PlaybackInput` i tumacenje postavke sada pripadaju javnom
  `qnc-work-settings`. IngestWorkPlan i player input koriste isti helper.
  Nema druge kopije parsera niti promjene spremljenih postavki.
- `original` bira original, `proxy` zahtijeva proxy, `proxy_if_available`
  bira zapisani proxy, inace original. Postojanje znaci DB vezu: ovaj modul
  ne otvara niti provjerava izvor. Nedostupan transport ili nevaljan proxy
  nije razlog za tihi original fallback ili novi probe.
- Prihvaca se samo finalni zapis. Nepotpuni odabrani medij odbija se;
  nepotpuni neodabrani proxy sam po sebi ne onemogucuje potpuni original.
  Nevaljane/inconsistentne cinjenice i lazni report odbijaju se u javnom
  metadata validatoru. Validacija zapisa nije probe medija.
- `PreparedInput` zadrzava cijeli spremljeni snapshot: URI-je, reviziju,
  original/proxy povezanost, kontejner, codec, PTS/time_base, frame rate,
  frame count, scan/color/rotation, sve audio streamove, tagove i evidence.
  Projekcija samo navodi video stream i native audio stream/channel adrese.
  Nema downmixa ni pretpostavke jednakih original/proxy audio kanala.
- Video koristi zapisani racionalni source FPS i tocni frame count, ne
  Project/export FPS ili duration * FPS procjenu. Procijenjeni frame count
  odbija se. Unknown/variable frame-rate mode ostaje takav u descriptoru;
  ovo nije potvrda CFR-a niti dokaz da ga buduci dekoder podrzava.
- Audio-only opis ne dobiva izmisljeni video FPS. Izlazni audio format,
  sample-rate konverzija, downmix i uredjaj nisu odgovornost input modula.
  Izvorna mapa ide po stream indexu i zero-based channel indexu; ogranicena
  je na u16 ukupan broj kanala kao player audio ugovor.
- Vise video streamova zahtijeva buduci eksplicitni stream-selection ugovor;
  ne bira se proizvoljno prvi. Ako imported URI nema spremljenu vezu s
  original/proxy representationom, ne pogadja se metadata ni novi path.
- Wire descriptor ima verziju i `validate_for(workspace_uri, clip_id)`:
  provjerava kontekst, snapshot, izbor i projekciju. Primatelj mora pozvati
  validator; deserializacija sama nije validacija niti autorizacija.

## Javni DB dohvat

Postojecem `/v1/ingest-content` ugovoru dodan je read-only `Read { clip_id }`.
Vraca jedan stored clip ili `None`; nema prolaza kroz cijeli katalog pri
otvaranju pojedinog klipa. Lokalno koristi postojeci indeks primarnog kljuca.
Mrezni odgovor mora odgovarati trazenom URI-ju, verziji i clip ID-u.

Wire verzija je `0.2.1`. Fizicka shema ostaje `SCHEMA_VERSION=0.2.0`.
Razdvojene su konstante za wire i shemu; nema migracije, promjene tablica
ili prepisivanja razvojnih podataka. Obje strane mreznog endpointa moraju
koristiti istu wire verziju; nema starog protokolnog fallbacka.

## Verifikacija

```text
cargo test -p qnc-player-input -p qnc-ingest-store -p qnc-work-settings
  -p qnc-ingest-components -p qnc-player-contract -p qnc-broadcast-player
  -p qnc-conformance
cargo clippy -p qnc-player-input --all-targets --no-deps -- -D warnings
cargo run -p qnc-conformance -- C:/Users/miron/Projects/QNC
```

Testovi pokrivaju original/proxy politiku, frakcijski FPS, native 4 mono
streama nasuprot 1 stereo streamu, AAC bez PCM bit-depth pretpostavke,
nepotpune/stare/pogresne zapise, read-only DB otvaranje, promjenu postavki,
nepoznate vrijednosti, ogranicenje kanala i nevaljane mrezne odgovore.
Local i LAN/Intranet prolaze preko stvarnih javnih DB adaptera. Mrezni
testovi koriste autentificirane loopback HTTP endpointove, ne fizicki LAN
ili intranet server. Granica modula provjerava se Cargo dependency grafom
i dodatnim ogranicenim source scannerom; to nije potpuni semanticki audit.

Rezultat zavrsnog ciljanog prolaza: 137/137 testova (48 jezgra, 16 player
contract, 13 input, 16 content/store, 28 Ingest component, 9 work-settings,
7 conformance). Clippy za novi input modul i conformance prolaze.

Stvarni razvojni DB provjeren je read-only primjerom:

```text
cargo run -p qnc-player-input --example inspect_saved -- C:/Users/miron/Projects/QNC
read=98, failed=0, media_opened=0, database_writes=0
```

SHA-256 globalnog registra i projektne baze jednak je prije i poslije.
Kartica nije otvorena niti mijenjana, nema izvrsavanja ffprobea. Dijagnosticki
primjer moze primiti i jedan `CLIP_ID`; bez njega namjerno obilazi spremljeni
katalog radi provjere svih zapisa. To nije scan kartice ni player runtime put.

Nisu provjereni stvarni decode/A/V sync, seek, fizicki mrezeni media pristup,
Linux/macOS/ARM ili cijeli workspace. UI live test nije dio ovog nevidljivog
DB adapter koraka; stvarni playback ostaje obavezan prije UI integracije.

## Sljedece

Odvojeni decode/output i media transport adapteri te samostalni player
proces. Oni moraju primijeniti puni spremljeni opis, definirati podrsku za
unknown/variable timing i color signale te iz njega pripremiti izvrsni
SourceRuntime bez gubitka stream mape. Minimalni AV model jezgre nije
zamjena za ovaj ulaz. Tek nakon stvarnog A/V testa spajaju se pasivni UI
prikazi. Filmstrip i Wave ostaju zasebni generatori, ne dio playera.
