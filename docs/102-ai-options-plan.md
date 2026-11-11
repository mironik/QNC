# 102 AI kao opcije projekta i AI server (audit i plan, 2026-11-11)

Korisnik: "napravi audit i plan kako AI mogucnosti postaviti kao opcije koje se ukljucuju ili iskljucuju u
Advanced postavkama Projecta; cilj je da QNC bude neovisan od AI mogucnosti, a da korisnik ukljuci one koje
treba; AI moze biti na LAN serveru, na Intranetu ili kao dodatni server na laptopu".

## 1. Audit: sto danas postoji

### 1.1 Gradnja: QNC vec ne nosi AI u sebi

- Nijedna aplikacija (`qnc-app`, forme) ne povezuje ML biblioteke (candle, ort, whisper, tokenizers):
  provjereno `cargo tree -p qnc-app -i <crate>`.
- Svaki AI motor je zaseban program uz aplikaciju, kroz ugovor dodatka (`qnc-plugin-process`):

| Mogucnost | Program | Model (data/models) | Ugovor |
|---|---|---|---|
| Prijepis (brzi) | qnc-transcribe-parakeet | parakeet-tdt-0.6b-v3-int8 | qnc.transcribe 1 |
| Prijepis (hrvatski, Best, ponovni) | qnc-transcribe-wav2vec2 | wav2vec2-parlaspeech-hr | qnc.transcribe 1 |
| Prijepis (whisper, u katalogu, ne izabran) | qnc-transcribe-whisper | ggml-large-v3-turbo | qnc.transcribe 1 |
| Vremena slova | qnc-align-wav2vec2 | wav2vec2-parlaspeech-hr | qnc.letter-align 1 |
| Sadrzaj kadra (AI pokrivalice, rjecnik) | qnc-shot-content-siglip | siglip2-base-patch16-224 | qnc.shot-content 1 |
| Opis kadra | qnc-shot-caption-florence | florence-2-base-ft (+ onnxruntime.dll) | qnc.shot-caption 1 |
| Prijevod OFF-a | qnc-translate-opus | opus-mt-tc-big-sh-en | qnc.text-translate 1 |
| Smooth spoj (meduslike) | rife-ncnn-vulkan | rife-v4.6 | vanjski program (nacin mape) |

- Modeli su izvan gita, 9 GB; od toga se ne koriste `moondream-q4`, `siglip-base-patch16-224` i
  `ggml-large-v3-turbo-q5_0.bin` (mogu van iz instalacije).
- Bez AI-ja (bez modela) nisu: analiza kadrova (`qnc-shot-analysis`: rezovi, mirni dijelovi, cisti Rust nad
  slikama), glasnoca/normalizacija (`qnc-loudness`), pomocnik tona (`qnc-sound-assist`), provjera citanja
  (`qnc-read-check`, ali treba prijepis snimke).

### 1.2 Gdje motor radi: samo ovo racunalo

- Katalog AI-ja racunala (`catalogs/ai/catalog.json`, `qnc-ai-catalog`) imenuje program i mapu modela na
  ovom racunalu. `qnc-plugin-process` pokrece program kao dijete procesa i govori JSON retke na stdin/stdout.
- Zahtjevi nose putanje ovog racunala: WAV u privremenoj mapi (prijepis, slova), JPEG putanje (sadrzaj,
  opis), mapu modela. Takav zahtjev ne moze ici na drugi stroj.

### 1.3 Ukljucivanje: nedosljedno

Projektne postavke (Advanced, `ai_switches`): `ai.enabled` ("AI analiza kadrova i SubClipovi", zadano
iskljuceno), `ai.coverage_suggestions` (zadano ukljuceno), `ai.transcription` (zadano iskljuceno),
`ai.language`, `audio.transcribe_channel`.

| Mogucnost | Cime se ukljucuje danas |
|---|---|
| Analiza kadrova, sadrzaj kadra, opis kadra | `ai.enabled` |
| Prijepis (red, zoom, provjera citanja) | `ai.transcription` |
| Vremena slova | nicim (radi cim je u katalogu racunala) |
| Prijedlog pokrivalica, prijevod | nicim (radi cim forma ima tab AI pokrivalice) |
| Smooth spoj | korisnikovim gumbom, bez projektne postavke |
| `ai.coverage_suggestions` | zapisuje se, nitko ga ne cita |

- Tabovi i paneli AI-ja (AI pokrivalice, Video AI, Prijepis, dugme Predlozi pokrivalice, AI u traci
  razine) vide se bez obzira na postavke i na to ima li racunalo motor; kad motora nema, dobije se greska
  ili prazan prikaz.

### 1.4 Gdje su AI podaci

- U `project.db`: prijepisi (`clip_transcripts`, potrebni prici), analiza kadrova (`clip_shots`), sadrzaj
  kadra (`clip_content`, `shot_label_vectors`).
- Na racunalu: opisi kadrova (`data/qnc-shot-captions.db`), ispravci grupa (`data/qnc-shot-examples.db`),
  rjecnik (`data/qnc-shot-vocabulary.db`).
- Otvoreno od ranije: AI tablice iz `project.db` u zasebnu `ai.db` uz projekt.

## 2. Cilj

1. QNC radi potpuno bez AI-ja: novi projekt ima sav AI iskljucen, a forme ne pokazuju nista AI dok ga
   korisnik ne ukljuci.
2. Svaka AI mogucnost ima svoju sklopku u Project Advanced; ovisnosti su vidljive (npr. AI pokrivalice
   trebaju sadrzaj kadra).
3. Gdje motor radi bira se odvojeno: ovo racunalo, AI server na LAN-u, AI server na Intranetu. "Dodatni
   server na laptopu" je isti AI server pokrenut na laptopu (za sebe ili za druge na LAN-u).
4. Isti ugovori vrijede lokalno i preko mreze; nijedan tihi zamjenski motor.

## 3. Model

### 3.1 Mogucnosti (podatak, ne kod)

Novi popis `catalogs/ai/features.json`: sifra, naziv (rjecnik), motor(i) iz kataloga, ovisnosti.

| Sifra | Sto daje | Treba |
|---|---|---|
| `transcription` | prijepis klipova, tab Prijepis, izjave iz teksta | motor prijepisa |
| `letter_times` | fino vrijeme slova (rez po slovima) | `transcription`, motor slova |
| `read_check` | provjera procitanog OFF-a, nadena citanja | `transcription` |
| `shot_content` | sadrzaj kadra, tab AI pokrivalice | analiza kadrova (nije AI), motor sadrzaja |
| `shot_caption` | opis kadra (Florence), korektor SigLIP-a | `shot_content`, motor opisa |
| `broll_suggest` | prijedlog pokrivalica (Story, MA Script) | `shot_content` |
| `translate` | prijevod OFF-a za prijedlog | `broll_suggest`, motor prijevoda |
| `smooth_cut` | Smooth spoj slozene izjave | motor meduslika |

Analiza kadrova (Video AI: rezovi i upotrebljivi dijelovi) nije AI model; vrijedi kao zasebna sklopka
`shot_analysis` jer cita karticu u pozadini.

### 3.2 Projekt kaze sto zeli, racunalo/mjesto kaze gdje radi

- Projekt (baza, Advanced): `ai.features.<sifra>` ukljuceno/iskljuceno; `ai.where`: `local`, `lan`,
  `intranet` (isti pojmovi kao transport). Zadano: sve iskljuceno, `local`.
- Adresa AI servera nije u projektu: to je konfiguracija racunala/mjesta (kao veze izvora i katalog
  dekodera), iz owner registry/transport konfiguracije, nikad hardkodirani URL.
- Nova kocka `qnc-ai-features` (cista): iz postavki projekta i kataloga racunala daje za svaku mogucnost
  `On`, `Off` ili `Unavailable(razlog)` (motor nije na ovom racunalu/serveru, ovisnost iskljucena). Svaki
  pokretac i svaki prikaz pita samo nju. Stare sklopke (`ai.enabled`, `ai.transcription`,
  `ai.coverage_suggestions`) zamjenjuju se njome.

### 3.3 AI server

- Novi program `qnc-ai-server`: drzi iste programe dodataka i njihove modele na svom stroju i sluzi njihove
  ugovore preko mreze (isti JSON retci, TCP s TLS-om i tokenom; adresa i token iz konfiguracije mjesta).
- `qnc-plugin-process` dobiva drugi prijenos: `local` (dijete procesa, kao danas) ili `remote` (veza na AI
  server); ugovori i pozivatelji se ne mijenjaju.
- Ugovori dodataka verzija 2: umjesto putanja nose sadrzaj (JPEG bajtovi slika, WAV bajtovi zvuka) ili QNC
  URI medija koji server sam cita kroz transport; model se imenuje sifrom iz kataloga servera, ne mapom.
  Verzija 1 ostaje za lokalni rad dok se svi ne prebace.
- Laptop kao dodatni server: `qnc-ai-server` pokrenut na laptopu; QNC na istom laptopu ga vidi kao `local`
  (ili localhost), drugi na LAN-u kao `lan`.
- Server ima prednost pred playerom samo na svom stroju; na laptopu koji montira i dalje ceka player.

### 3.4 Podaci

- AI rezultati projekta (`clip_shots`, `clip_content`, `shot_label_vectors`) u `ai.db` uz projekt
  (prijepisi ostaju u `project.db` jer ih prica treba). Brisanje `ai.db` ne dira projekt.
- Opisi, ispravci i rjecnik: danas po racunalu; s AI serverom na LAN-u trebaju biti zajednicki (na serveru
  ili u `ai.db` projekta). Odluka korisnika.

## 4. Plan (redom)

1. **Popis mogucnosti i kocka `qnc-ai-features`**; Project Advanced: odjeljak AI s jednom sklopkom po
   mogucnosti (ovisne sive dok ovisnost nije ukljucena) i izbor "Gdje radi AI" (ovo racunalo / LAN /
   Intranet). Treba otkljucati Project. Zadano sve iskljuceno; stare sklopke se citaju kao nove dok se ne
   zamijene (bez migracije: razvojni projekti dobivaju zadano).
2. **Svaki pokretac pita `qnc-ai-features`**: pozadinski radnik (analiza, sadrzaj, opis), red prijepisa,
   zoom rijeci, vremena slova, provjera citanja, prijedlog i prijevod, Smooth. Iskljuceno = nista se ne
   pokrece i nista ne cita kartica.
3. **Forme slijede mogucnosti**: tabovi AI pokrivalice, Video AI, Prijepis, dugme Predlozi pokrivalice,
   Smooth, AI u traci razine skriveni (ili sivi s razlogom) kad je mogucnost iskljucena ili nedostupna.
   Layout se ne mijenja: prikaz pita kocku.
4. **Advanced pokazuje stanje**: uz svaku sklopku "radi na ovom racunalu / na serveru X / nije instalirano
   (motor Y)" (iz `qnc-installed-versions`).
5. **Ugovori dodataka v2** (sadrzaj ili URI umjesto putanja), lokalno najprije, isti testovi prihvata.
6. **`qnc-ai-server` i prijenos `remote`** u `qnc-plugin-process`; konfiguracija mjesta (adresa, token);
   proba: laptop kao server za sebe (localhost), zatim drugi stroj na LAN-u.
7. **`ai.db` uz projekt** i odluka o zajednickim opisima/ispravcima.
8. **AI paket**: instalacija QNC-a bez AI-ja; AI paket (programi + modeli, licence: CC-BY 4.0 za OPUS-MT,
   MIT Florence-2, Apache SigLIP 2) zasebno, za racunalo ili server; neiskoristeni modeli van.

## 5. Odluke korisnika prije koda

1. Zadano za novi projekt: sav AI iskljucen (predlozeno) ili neke mogucnosti ukljucene?
2. "Gdje radi AI": jedan izbor za cijeli projekt (predlozeno) ili po mogucnosti (npr. prijepis lokalno,
   sadrzaj kadra na serveru)?
3. Opisi kadrova i ispravci grupa na LAN-u: zajednicki (na serveru / u projektu) ili po racunalu?
4. Otkljucavanje Projecta za korak 1 (Advanced odjeljak AI).
