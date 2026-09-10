# QNC - audit Broadcast Playera i veze s Project postavkama

> ISPRAVAK 2026-09-09: Zakljucak ovog izvjestaja koji opravdava
> `player-output.json` umjesto primjene projektnih audio postavki je povucen.
> Broj izvornih kanala i source sample rate nisu isto sto i zadani programski
> audio izlaz. Ocuvanje source metapodataka ne opravdava ignoriranje projektnog
> izlaza. Tvrdnja da korisnik pogresno ocekuje primjenu `audio.channels` nije
> ispravna. Mjerodavan novi nalaz, s provjerom stvarne baze, je
> [Broadcast Player: v4 tijek i audio ulaz](C:/Users/miron/Projects/QNC/docs/66-broadcast-player-v4-flow-audit.md).
> Ostatak ispod je povijesni zapis audita, ne pravilo za nastavak razvoja.

Datum: 2026-09-09.
Predmet: koji projektni zapisi stvarno ulaze u Broadcast Player.
Root: `C:\Users\miron\Projects\QNC` (radno stablo).
V4 referenca: `qnc_v4/qnc-host/src/media/play.rs`.
Project kod nije citan radi izmjene i nije mijenjan.

Ovo je nalaz, ne odobrenje novih Project polja.

## Zakljucak

Player ne poznaje Project aplikaciju. Cita aktivni projekt kroz javni
`qnc-work-settings` (read-only) i clip snapshot kroz `ingest_content`.
Od svih polja u `settings_json` za reprodukciju slike koristi **samo**
`playback.input`. Timebase, trajanje i audio kanali dolaze iz spremljenog
media zapisa, ne iz `video.fps` ni `audio.sample_rate` projekta.

To je ispravno prema AGENTS 8.2 i 4.1. Lako se krivo cita: WorkSettings
**zahtijeva** da `video.fps` i `audio.sample_rate` postoje, ali ih player
ne primjenjuje. Mapa fizickih izlaza (`data/player-output.json`) nije
projektna postavka. Transport izvora nije projektna postavka.

Veza s Projectom je DB ugovor, ne crate. Freeze nije narusen.

## Lanac

```text
Project (vlasnik zapisa, zamrznut)
  project_settings.settings_json
  public_project_settings

qnc-work-settings  (query_only)
  aktivni project_id -> WorkSettings
       playback.input
       workspace_db_uri
       video/audio objekti samo kao potpunost snapshota

Ingest komponenta
  SettingsReader.clone() -> prepare_preview

qnc-player-input::InputReader.load(workspace_uri, clip_id)
  1. settings.playback_input()     -> original | proxy | proxy_if_available
  2. ContentTarget u qnc_project.db (ingest tablice, ReadOnly)
  3. Final snapshot -> slika iz odabrane reprezentacije
                     -> audio uvijek iz originala (docs/64)
  4. timebase/duration/stream index iz snapshota

qnc-player-client + qnc-broadcast-player
  ne cita Project bazu
  device_channels iz data/player-output.json ili 1:1
  media binding iz ingest-transport.json
```

Jezgra i runner nemaju `WorkSettings`. Ingest Cargo.toml nema `qnc-project*`.

## Sto je u projektu, a sto player koristi

Iz `seed/system_seed.json` i `WorkSettings::from_saved`:

| Zapis u settings_json | U WorkSettings | U player input | U engine/decode |
| --- | --- | --- | --- |
| `playback.input` | da, parse | da, bira sliku | ne direktno |
| `playback.cache` | unutar `playback` objekta | ne | ne |
| `video.fps` / format / codec | da, fps obavezan za validate | ne | ne |
| `audio.sample_rate` / channels | da, rate obavezan za validate | ne | ne |
| `audio.transcribe_channel` | sirovi JSON | ne | ne |
| `storage.*` | da | ne (to je import) | ne |
| `input.mode` | da | ne | ne |
| `export.*` | nije u snapshotu | ne | ne |
| `ai.enabled` | da | ne | ne |
| `keyboard_shortcuts` | da | ne | ne |
| `workspace` URI | da | da, koji DB citati | ne |

Test `policy_uses_saved_playback_input_not_storage_or_project_format`
fiksira: projektni `video.fps=50` i `audio.sample_rate=44100` ne mijenjaju
timebase `30000/1001` ni 4 originalna kanala.

## Politika `playback.input` (isto kao v4)

| Vrijednost | Ponasanje |
| --- | --- |
| `original` | slika s original URI/streamova |
| `proxy` | slika s proxy; nema proxy zapisa = greska, nije tihi original |
| `proxy_if_available` | proxy ako snapshot ima proxy, inace original |

Audio pri proxy slici ostaje originalni inventar; proxy i original moraju
imati isti spremljeni video timing. Nema novog probea.

v4 `resolve_playback_input_media` radi istu politiku, zatim trazi **putanju**
i jedan `probe_json` po `clip_id`. Novi QNC trazi **DB reprezentacije**.
To je namjerna korekcija docs/24, ne nova Project postavka.

## Sto nije projektna postavka, a ulazi u play

1. **Clip snapshot** u Ingest tablicama iste `qnc_project.db`. Owner je Ingest.
2. **`data/ingest-transport.json`** — URI -> root/endpoint. Bez toga nema
   media bindinga. Nije u `settings_json`.
3. **`data/player-output.json`** — `device_channels` za slusni uredjaj.
   Komentar u kodu: "Host-device selection only, not project/media settings."
   Nedostaje datoteka = native 1:1. Projektni `audio.channels: 2` to ne
   zamjenjuje.

Mijesanje ova tri izvora s Project postavkama bilo bi krivo: uredjaj i
kartica nisu odluka templatea.

## Granice 4.1 / 8.2

- Citanje `query_only`. Player input otvara content `Access::ReadOnly`.
- Nema pisanja `project_settings`.
- Nema defaulta kad `playback.input` nedostaje ili je nepoznat.
- Project/export FPS nije source FPS.
- Player ne aktivira projekt i ne bira drugi workspace od onog u snapshotu.
- Promjena postavki usred `load` odbija ulaz (`ChangedSettings`).

## Nalazi

### P1

Nema narušene veze Player -> Project crate niti izmišljanja FPS iz
projektnog formata.

Preostali proizvodni jaz (isti kao docs/59/60): **Uvezi** nije izvršen;
play ide na izvorni URI. To nije rupa u citanju postavki.

### P2

- **Dva "audio" ugovora.** Projekt `audio.channels` / `sample_rate` su
  template/export/transcribe kontekst. Player koristi spremljene native
  kanale. Operator koji ocekuje da `audio.channels: 2` stisne izlaz na
  stereo gresi. docs/64 to zabranjuje; UI to ne objasnjava.
- **Slusni par izvan projekta.** `player-output.json` s `[0,1]` je
  workstation datoteka. Nije u public_project_settings. Nema je u seedu
  kao Project polje. To je ispravno, ali nedokumentirano u ownership
  matrici.
- **WorkSettings.validate zahtijeva `video.fps`.** Bez toga Ingest/player
  input uopce ne krene, iako taj fps ne ide u decode. To je gate
  potpunosti templatea, ne playback matematika. Nije regresija, ali je
  sprega "player ne radi ako template nema fps" iako player fps ne koristi.

### P3

- `playback.cache` u seedu nema potrosaca.
- `storage.ingest_media` ceka import, ne play.
- §16 i dalje kaze da playback nije implementiran.

## Usporedba s v4 na postavkama

v4: host cita `project_effective_settings` -> `playback.input` -> path +
`ingest_assets.probe_json`. Player jezgra ne otvara Project bazu.

Novi: ista politika, javni modul umjesto host funkcije, snapshot umjesto
jednog probe_json, audio s originala pri proxy slici.

Nije prenesen privatni `qnc-host` lookup. Nisu dodavana Project polja za
player.

## Provjereno

- `WorkSettings` validate vs `playback_input()`.
- `qnc-player-input` choose/layout i test formata.
- Ingest `playback.rs` (settings reader, transport, player-output.json).
- seed `settings_json`.
- v4 `play.rs` politika.
- Odsutnost WorkSettings u runtime/runner.

## Nije provjereno

- Live Play uz namjerno krivi `video.fps` u stvarnoj bazi (test to pokriva).
- Fizicki uredjaj vs `device_channels` [0,1] na 4-kanalnom izvoru.
- Linux/macOS.

## Sljedeci rizik

Dodati `video.fps` ili `audio.channels` u player "da prati projekt" vratilo
bi docs/24 gresku i AGENTS 8.2. Ako treba slusni stereo par, ostaje
workstation output mapa, ne nova Project postavka, dok korisnik izricito
ne zatrazi drukcije.
)
