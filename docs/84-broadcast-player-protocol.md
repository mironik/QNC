# Broadcast Player protokol

Zakljucano 2026-09-13. Player je uski playback proces. Nije QNC poslovna
aplikacija. Ne poznaje Ingest, Story, Project, shell ni formu.

Okruzenje se prilagodava ovom protokolu. Player se ne prilagodava okruzenju.

## Player smije

- Session open: `PreparedInput` + privatni media binding (nije dio naredbi).
- Naredbe: Play, Pause, Stop, CueFrame, Shutdown.
- Evente: status, carrier, timebase, ready, CommandAccepted/Rejected, PlaybackError.
- Izlaze koje on definira: audio na hostu, lokalni `qnc-player-frame-map` preview.

## Player ne smije

- Probe, scan, filmstrip, wave, import, DB write, app workflow, UI paint.
- Mapirati `ingest_*` action_id. App prevodi vlastiti `action_id` u naredbu.
- Gasiti proces ili command socket zbog `NotReady`. To pauzira sat.
- HTTP/URL kao decode ulaz.

## Zica

- Naredba/stanje: `qnc-player+tcp`. Malo, bez RGBA.
- Preview pikseli: samo `qnc-player-frame-map`. Nema socket RGBA fallback.
- Preview paint je javni `qnc-monitor`. Player ne crta UI. Forma ne posjeduje
  preview. Svaka aplikacija ugradi istu komponentu.
- Idle timeout ne smije prekinuti `Playing` ili `Preparing`.

## Prihvat 5.0

§8.3 na samom procesu (Mironik 2002 i 2679 do kraja, Pause/Play, Cue) prije
novog Ingest spoja. Ovaj dokument ne tvrdi da je prihvat zatvoren.
