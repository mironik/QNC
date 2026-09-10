# Broadcast Player: prvi sljedeci javni runtime modul

Datum: 2026-09-08.

## Korisnicka odrednica

Prvo se razvija javni Broadcast Player. Timeline nema vlastiti playback:
samo prikazuje potvrdeno player stanje i emitira neutralne zahtjeve.
Monitor, filmstrip pozadina i wave prikaz takodjer su pasivni. Generatori
filmstripa i wavea nisu dio playera i ne moraju ovisiti o njemu.
Obvezno pravilo je u root AGENTS.md, odjeljci 8.1, 8.2 i 13.

## Provjereno u kodu

- Novi QNC ima samo `contracts/modules/broadcast-player.module.json`,
  verzija 0.1.0. Nema implementiranog Broadcast Player cratea ili procesa.
- V4 aktivni `qnc-broadcast-player` ima odvojene frame/timebase, transport,
  command i event ugovore. Core Cargo ovisi samo o serde/serde_json.
- `qnc-player-runtime` i `qnc-player-runner` odvojeni su od egui aplikacije.
  Procesni protokol ima verziju, request_id, OpenSource/OpenProgram,
  Command, Synchronize i Shutdown.
- `qnc-player-runtime/src/process_client.rs` pokrece child proces, prenosi
  naredbe kroz stdin/stdout JSONL i cita latest-frame map. To je lokalni
  adapter, ne dokaz LAN/Intranet video/audio transporta.
- `qnc-media-ffmpeg/src/lib.rs` u istom paketu izvozi proxy, filmstrip,
  waveform, poster i audio-wrap kod uz dekoder. Paket se ne smije prenijeti
  cijeli kao uska player ovisnost. Player prepare koristi dobivene metadata;
  postojanje probe funkcija drugdje u paketu nije dokaz player probe poziva.
- V4 `AGENTS.md` trazi source racionalni FPS/timebase, frame-precizni
  playback i player-owned sat. Njegova stara odrednica jedne workstation
  aplikacije ne prenosi se: novi QNC ima samostalne aplikacije i DB-only vezu.

Ulazni put vec postoji u novom QNC-u: javni `qnc-work-settings` cita bazu
aktivnog projekta i radne postavke read-only; javni Ingest content/media
ugovori sadrze objavljene klipove i original/proxy metadata. Player input
komponenta treba koristiti taj put, ne stvarati nove Project postavke.
Raniji audit specificnih video/audio podataka i v4 ogranicenja je docs/24.

## Predlozeni opseg za potvrdu prije prijenosa aktivnog koda

1. Javni player ugovor i neutralna jezgra: source/range/timebase, naredbe,
   potvrde, runtime state, greske i lifecycle. Odvojiti prihvacenu naredbu
   od potvrdenog/prezentiranog framea; izolirati sesije i odbaciti zastarjele
   odgovore. Bez egui, aplikacijskih crateova, DB pisanja i probea.
2. Javni read-only input adapter: projektne postavke + clip/probe zapis ->
   opis odabranog medija i QNC URI. Original/proxy odabir slijedi
   `playback.input`; svaki medij koristi vlastiti opis streamova. Audio
   stream/channel map ne zamjenjuje se pretpostavkom da je sve `0:a:0`.
3. Samostalni runner i uski decode/output adapteri: stvarni video/audio,
   player-owned sat i ograniceni bufferi. Verzijski isti command/event
   ugovor za lokalni i mrezni adapter. Prijenos slike/zvuka je zasebna
   output granica, ne JSON kontrolna poruka niti javna lokalna putanja.
4. Ciljani testovi i stvarni read-only media test bez Ingest/Project/shell
   procesa. Provjeriti pause, frame seek, kraj rangea, audio mapu,
   raskid veze, izolaciju sesija i zabranu dodatnog probea. Lokalni test
   nije dokaz fizickog LAN/Intranet ili svih OS/CPU konfiguracija.
5. Tek zatim spojiti Ingest kroz javni klijent i pasivne monitor/timeline
   komponente. Nema UI-owned playback stanja. Filmstrip i wave dolaze
   kao zasebni generatori i spremljeni prikazni artefakti.

## Status prije implementacije

Korisnik je zatim potvrdio nastavak. Izdvajanje ugovora i jezgre opisano je u
`docs/45-player-contract-and-core.md`; donji zapis opisuje prethodni korak.

Ovaj nastavak biljezi prioritet i prijedlog opsega, ne implementira player.
Nije kopiran aktivni v4 kod niti je potvrden live playback.

Prije promjene prioriteta uklonjeni su lokalni `playing` i `cue_frame` iz
Ingest komponente. Nepovezane player naredbe sada su odbijene umjesto
simulacije; timeline manifest je stateless i dobio je provjere ugovora.
To nije zamjena za Broadcast Player. Ciljani testovi tog zahvata:
qnc-contracts 9/9, qnc-ingest-components 28/28, qnc-ingest-desktop 4/4;
conformance prolazi. Live UI provjera nije zatvorena prije preusmjeravanja.

Sljedeci rizik: kopiranje cijelog v4 runtime/decode paketa vratilo bi probe,
generatore i lokalni IPC kao skriveno ogranicenje javnog player modula.
