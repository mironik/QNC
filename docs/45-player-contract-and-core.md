# Broadcast Player: javni ugovor i neutralna jezgra

Datum: 2026-09-08. Nastavak korisnicki potvrdjenog plana iz docs/44.
Ovo je korak ugovora/jezgre, ne zavrsen player za stvarnu reprodukciju.

## Opseg i porijeklo

Iz aktivnog radnog stabla `C:/Users/miron/Projects/qnc_v4/qnc-broadcast-player`
izdvojeni su neutralni modeli, command/event/request ugovori, adapter traits,
frame clock, transport engine i njihovi testovi. Referenca nije mijenjana.
Nisu preneseni app wrapper, stari runtime proces, FFmpeg paket, generatori,
probe, monitor niti UI. Stari veliki `contract.rs` JSON nije prenesen kao
drugi katalog uz nove root module manifeste.

Javni dijelovi:

- `qnc-player-contract`: source/range/AV opis, command/event tipovi i
  verzionirane omotnice. Klijent ga moze koristiti bez player enginea.
- `qnc-broadcast-player`: izvrsna jezgra namijenjena zasebnom player procesu;
  sat, playout buffer i neutralni source/decode/output adapter traits.
- Postojeci `qnc-frame-timebase::FrameTimebase` koristi se izravno kao player
  `Timebase`. Nije uveden drugi FPS model niti float/zaokruzeni playback FPS.

Nema ovisnosti o aplikacijama, egui, DB pristupu, resolver implementaciji,
FFmpegu ili probeu. Konkretne implementacije adaptera dolaze zasebno.
Player manifest zadrzava ciljani out-of-process model, ali eksplicitno navodi
`implementation_stage=contract_and_core_only` i `runtime_available=false`.
Ingest i shell jos ne ovise o playeru i nisu mijenjani u ovom koraku.

## Ugovor i potvrdeno stanje

- Raspon je `[start_frame, end_frame)`. OUT nije frame za dekodiranje.
  Izvor s N frameova ima stvarne frameove 0..N-1. Zadnji frame ostaje
  prikazan do kraja svoga intervala; tek tada slijedi boundary/pause.
- `carrier_frame` je playerova pozicija. `presented_frame=None` znaci da
  novi source jos nema potvrdenu video prezentaciju. Decode ili output
  greska ne potvrduje trazeni frame. `at_end` razlikuje dovrsen playback
  od korisnickog cuea na zadnji frame; play nakon dovrsenja krece od IN-a.
- Sinkroni `FramePresenter` mora vratiti `FramePresented` za trazeni frame.
  Sam enqueue u asinkroni izlaz nije potvrda prezentacije. Buduci asinkroni
  output adapter mora ugovoriti completion put prije integracije.
- Nevaljani range/rate/source metadata odbijaju se prije prekida postojeceg
  playbacka. Preload druge metadata revizije ne smije se tiho aktivirati.
- Omotnice imaju verziju, session_id, source_generation i sequence; naredbe
  i request_id. `validate_for` provjerava primateljevu sesiju, generaciju i
  zadnji sequence. Primatelj ce morati pozvati validator i voditi svoje
  sekvence; ove strukture nisu implementirani server, auth ili reconnect.
- Client ne salje clock tick. Privatni engine tick namijenjen je buducem
  player runneru s monotonic satom, nikad UI render petlji.

## Popravci pri izdvajanju v4 jezgre

1. Pozicija i video potvrda mijenjaju se tek nakon uspjesnog outputa.
2. Ne dekodira se nepostojeci frame na punoj duljini izvora.
3. Nevaljani zahtjevi ne mijenjaju range niti prekidaju clock prije validacije.
4. Provjeravaju se source timebase, AV format i source/metadata revizija.
5. Idle prebuffer ne raste prema cijelom klipu; broj pripremljenih frameova
   ostaje ogranicen ciljanom dubinom. Byte limit ovisi o buducem dekoderu.
6. Izvori i frameovi vraceni iz decode/audio adaptera moraju odgovarati
   zahtjevu. Audio frame paket pokriva jedan engine frame.

## Verifikacija

Na Windows hostu:

```text
cargo test -p qnc-player-contract -p qnc-broadcast-player -p qnc-conformance
48 core + 16 contract + 6 conformance testova: 70/70
cargo clippy -p qnc-player-contract -p qnc-broadcast-player --all-targets --no-deps -- -D warnings
cargo run -p qnc-conformance -- C:/Users/miron/Projects/QNC
```

Core testovi koriste memorijske testne adaptere, ne glume live video test.
Pokriveni su integer i NTSC rateovi, pause/resume, delayed tick, rasponi,
jedan frame, decode/output greske, idle buffer, odvojene engine instance,
stare poruke i metadata revizije. Conformance cita strukturirani Cargo
metadata graf i provjerava dependency granicu i verzije novih modula.
Source scanner je dodatna provjera poznatih I/O obrazaca, ne potpuni Rust
semanticki analizator. Testovi nisu dokaz svih mogucih transportnih gresaka.

Nisu pokretani stvarni media/probe procesi, Sony kartica, UI live playback,
fizicki LAN/Intranet, Linux/macOS/ARM ili cijeli workspace test suite.
Nema promjena baza, projektnih postavki, izvora ni UI rasporeda.

## Sljedece

Javni read-only input adapter treba iz postojeceg DB ugovora pripremiti
konkretan original/proxy media opis prema `playback.input`, bez novih
Project postavki. Minimalni AV format jezgre nije zamjena za potpuni
spremljeni opis kontejnera, codeca, stream indexa i audio channel mape.

Zatim slijede odvojeni decode/output adapteri, samostalni proces, lifecycle
i command/event dispatch, media resolver i lokalni/mrezni transport.
Tek stvarni A/V test tog puta moze potvrditi playback, A/V sync, resursne
limite i LAN/Intranet rad. Do tada nema povezivanja timelinea ili forme na
lokalni zamjenski player. Filmstrip i wave ostaju zasebni buduci moduli.
