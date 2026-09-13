# QNC AGENTS compliance repair

Datum: 2026-09-12

Ovaj zapis nastavlja `docs/80-agents-compliance-audit-2026-09-12.md` i
biljezi sto je popravljeno nakon audita. `docs/80` ostaje povijesni audit,
ne trenutno stanje.

## Popravljeno

- `AGENTS.md` §16 je uskladjen sa stvarnim stablom:
  - `Close project` postoji u footeru samo kao poziv javnog modula
    `qnc-project-close`.
  - Ingest manifest smije navoditi Filmstrip/Wave/Timeline/Broadcast Player
    samo kada stvarni javni runtime modul ili adapter postoji.
  - Broadcast Player acceptance iz §8.3 nije zatvoren; vec spojeni pasivni
    prikazi smiju ostati samo ako ne preuzimaju player sat, ne rade probe, ne
    pisu direktno u bazu i ne konkuriraju playeru.
- Filmstrip linearni/full-clip scan je uklonjen iz javnog filmstrip modela,
  decoder cataloga, ffmpeg adaptera i filmstrip workera. Filmstrip put ostaje
  catalog-selected keyframe/intra seek ili kontrolirana greska.
- Playback Guard je izdvojen u `qnc-ingest-application/src/playback_guard.rs`
  kao jedno mjesto za zabrane dok je Broadcast Player u `Preparing` ili
  `Playing`.
- Guard sada:
  - blokira nove source/browser/select/thumbnail akcije,
  - prekida vec pokrenuti Select bez cekanja na thread,
  - gasi thumbnail batch,
  - odgadja filmstrip/wave `sync_content_db`,
  - pauzira filmstrip/wave generate i publish poll kroz postojece worker
    priority mehanizme.
- `qnc-ingest-store` vise ne ovisi o `qnc-filmstrip`, pa `qnc-player-input`
  ne vuce `qnc-filmstrip` ni `qnc-image-assets` tranzitivno kroz store.

## Jos ostaje

- `qnc-ingest-application` je i dalje vece composition mjesto nego sto zakon
  dugorocno zeli. Smjer ostaje smanjivanje prema uskim javnim modulima, bez
  novog `components` sloja i bez preimenovanja crateova.
- Broadcast Player live acceptance iz §8.3 nije zatvoren ovim zapisom.
- Shell factory jos ima eksplicitnu mapu dostupnih adaptera; ne smije se siriti
  trecom aplikacijom kao novi monolitni switch.

## Provjera

- `cargo test -p qnc-ingest-application -p qnc-filmstrip -p qnc-filmstrip-worker -p qnc-decoder-catalog -p qnc-ffmpeg-decode --quiet`
- `cargo run -p qnc-conformance --quiet`
- `cargo build -p qnc-app -p qnc-ingest -p qnc-broadcast-player -p qnc-dev-diagnostics-app --quiet`
- `cargo test -p qnc-ingest-application -p qnc-ingest-desktop -p qnc-keyboard-shortcut -p qnc-timeline-assets -p qnc-filmstrip -p qnc-filmstrip-worker -p qnc-wave -p qnc-wave-worker -p qnc-ingest-store -p qnc-player-input -p qnc-player-client -p qnc-broadcast-engine -p qnc-app --quiet`
