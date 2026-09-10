# Ready before Play

2026-09-08. Opseg prije implementacije, nastavak docs/48 i AGENTS 8.2.

Mijenjaju se samo javni player contract i cista player jezgra. UI, Project,
Ingest workflow, DB, izbor original/proxy medija i dekoder ostaju nepromijenjeni.
V4 Jedinstveni model i aktivni player/runtime sluze kao referenca za vlasnistvo
sata i lifecyclea; stari odgodjeni preroll nije obrazac koji se zadrzava.

- Load/activation: Preparing, nikad unaprijed Ready. Izvor i izlaz otvaraju se
  izvan Play puta. Runtime poziva pripremu na svojem workeru, ne UI threadu.
- Idle tick priprema ogranicen broj frameova. Ready/SourceReady dolaze tek
  nakon cijelog pocetnog buffera i potvrdenog, tihog punjenja izlaza.
- Javni play_ready i PlaybackReadinessChanged predstavljaju spremnost za
  nastavak i dok je transport Paused/Stopped. Ne dolaze iz UI-ja.
- Play prije spremnosti vraca NotReady bez decodea i bez zapamcenog auto-playa.
  Play nakon spremnosti predaje pocetni frame i pokrece vec pripremljeni
  izlaz/sat. Nema pocetnog decodea ni prvog preroll ticka nakon Play naredbe.
- Output prepare/queue/commit NE smiju pokrenuti zvuk. Start je zaseban,
  obvezni adapter poziv. Video output takodjer se priprema prije Ready.
- Pause/Stop zadrzavaju otvorene resurse i bounded payload cache; pauzirani
  output se ponovno tiho priprema prije nastavka. Seek i promjena range/rate
  ponistavaju spremnost. Unload/switch gase resurse. Preload nije Ready i ne
  smije promijeniti aktivni audio output.
- Pocetni buffer ogranicen je rangeom i maksimalnim burstom. Nema dekodiranja
  cijelog klipa tijekom cekanja. Neuspjeh pripreme ne proizvodi Ready/Playing.

Contract/core verzija ide na 0.2.0 zbog novog stanja/dogadaja i odvojenog
output start ugovora. Nema migracija ni kompatibilnog starog Play fallbacka.
Nove tipke/forme ne dodaju se; koristi se postojeci vanjski Play/Pause ugovor.

Verifikacija: adapteri koji broje/zabranjuju decode nakon Ready; Play prije
Ready; tihi preroll, prva prezentacija u Playu, pause/resume bez reopena,
seek/rate/OUT/kratki klip, output failure i izolacija sesija. To provjerava
raspored rada u jezgri, ne dokazuje fizicku A/V latenciju. Stvarni output
adapteri i out-of-process runtime jos nisu implementirani; njihov live test
mora posebno mjeriti Play -> stvarni video/audio izlaz prije UI integracije.

## Rezultat implementacije

- Contract i core su 0.2.0; planirani runtime manifest sada upucuje na taj
  input/output/transport ugovor, ali `runtime_available` ostaje false.
- Play put testiran je adapterima koji prekidaju test ako se nakon Ready
  pokusa open, prepare, decode, audio render ili preroll. Dopusteni pozivi
  tijekom Play su samo prezentacija pripremljenog framea i start izlaza.
- Pocetna priprema dekodira najvise 16 frameova po ticku (zadano 4), a
  pocetni cache najvise 64 framea (zadano 16), uz granicu kraja raspona.
  Clone payload ugovor predvidja jeftine dijeljene nepromjenjive handleove;
  stvarni output adapter ne smije kopirati cijeli A/V buffer na Play.
- Pause/Stop cuvaju otvorene resurse i cache; izlazni red ponovno se puni
  iz cachea prije play_ready. Seek/range/rate i OUT ponistavaju spremnost.
- Neuspjela priprema izlaza zatvara aktivni handle i ostavlja Empty.
  Greske dekodiranja/punjenja/prezentacije tijekom rada zaustavljaju izlaz
  i sat; idle tick ih ne ponavlja sam od sebe.
- Preparing/readiness dogadaji prolaze isti session/generation/sequence
  ugovor; stara Ready poruka ne vrijedi za novu generaciju izvora.

## Verifikacija

Izvrseno na Windows razvojnom hostu:

- `cargo test -p qnc-player-contract -p qnc-broadcast-player`: 76 prolaza
  (58 core, 14 contract unit, 4 contract integration).
- `cargo test -p qnc-player-contract -p qnc-broadcast-player -p qnc-player-input -p qnc-media-stream -p qnc-media-decode --all-targets`:
  112 prolaza; tri izricita FFmpeg testa inicijalno ignored.
- `cargo test -p qnc-media-decode --lib -- --ignored --test-threads=1`:
  sva tri FFmpeg testa prolaze, na sintetickim privremenim podacima.
- `cargo clippy -p qnc-broadcast-player -p qnc-player-contract --all-targets --no-deps -- -D warnings`:
  prolazi za promijenjene crateove.
- `cargo run -p qnc-conformance -- C:\Users\miron\Projects\QNC`:
  svi checkovi prolaze, ukljucujuci keyboard v4 extension i player boundary.
- Ciljani `cargo fmt` i `git diff --check`: prolaze.

Nije izvrsen novi live karticni/UI test ni fizicki LAN/Intranet ili drugi OS.
Network testovi koriste kontrolirane transport endpointove, ne stvarnu mrezu.
Projektne baze, kartica, aplikacije i UI nisu predmet ovog zahvata.
Nema novog probea. Ovi rezultati NE dokazuju trenutni fizicki Play.

Sljedeci korak je javni izlazni adapter i player worker koji spaja postojece
input/stream/decode module s jezgrom. Priprema i tihi izlazni red moraju biti
gotovi prije Ready. Live mjeri posebno odabir -> Ready i Play -> prva stvarna
slika/zvuk, ukljucujuci Pause/resume. UI ostaje pasivni potrosac stanja i
remote naredbi, bez vlastitog sata ili lokalne pripreme playbacka.
