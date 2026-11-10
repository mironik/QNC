# 101. AI agenti za brzu montazu TV vijesti (offline)

Korisnik 2026-11-10: "analiza koje AI agente bi mogli dodati QNC-u kako bi ubrzali montazu TV vijesti,
koje imamo, koje bi mogli dodati, koje unaprijediti, a da sve radi offline; dodati ih kao dio aplikacije
ili kao samostalne agente na racunalu, LAN-u ili intranetu; izmedju ostalog sortiranje video klipova za
pokrivalice po sadrzaju kadra."

Polaziste: news cutter na laptopu (Iris Xe, 4 jezgre / 8 niti) s HDMI izlazom, kartica samo za citanje,
baza projekta je jedina istina, player ima prednost pred svim pozadinskim poslom. Sve brojke brzine u
ovom dokumentu oznacene "procjena" treba izmjeriti prije odluke.

## 1. Sto vec imamo

| Agent (sto radi) | Kocke / program | Motor | Stanje |
|---|---|---|---|
| Prijepis govora (Fast) | `qnc-transcribe-queue`, `tools/qnc-transcribe-parakeet` | Parakeet TDT 0.6B v3 int8, procesor | radi, 8-10x stvarnog vremena; slabiji na hrvatskom (imena, dijalekt) |
| Prijepis govora (Best, hrvatski) | `tools/qnc-transcribe-wav2vec2` | wav2vec2 ParlaSpeech-HR, procesor (candle) | radi, za ponovni prijepis izjave; bez interpunkcije |
| Prijepis (Quality) | `tools/qnc-transcribe-whisper` | whisper large-v3-turbo, Vulkan | u katalogu, iskljucen iz sucelja (izmislja na buci) |
| Prijepis za zoom reza | `qnc-zoom-words` | Parakeet | radi |
| Vremena slova | `tools/qnc-align-wav2vec2`, `qnc-letter-times` | wav2vec2 CTC poravnanje | radi |
| Rijeci na val | `qnc-word-align` | cisto pravilo nad valom | radi |
| Provjera procitanog OFF-a | `qnc-read-check`, `qnc-take-heard` | wav2vec2 + usporedba s tekstom | radi |
| Pronalazak citanja OFF-ova u snimci | `qnc-session-readings` | wav2vec2 + tekst | radi (i snimke napravljene drugdje) |
| Analiza kadrova (Video AI) | `qnc-shot-analysis`, `qnc-shot-detect` | cisto: histogram, pomak kamere, ostrina | radi: rezovi, mirno/svenk/trzaj, upotrebljivi dijelovi |
| Pomocnik tona | `qnc-sound-assist` | cisto: glasnoca iz vala | radi: ducking A2 pod govorom, izjednacenje govora |
| Normalizacija EBU R 128 | `qnc-loudness`, `qnc-program-loudness` | cisto | radi (preview i export) |
| Smooth spoj (medjuslike) | `qnc-frame-interp`, `qnc-smooth-joins` | RIFE ncnn Vulkan | radi |
| Pretraga prijepisa | `qnc-transcript` (search) | tocno podudaranje rijeci | radi, bez tolerancije i bez znacenja |

Zajednicko svima: rezultat ide u bazu projekta kroz javnog pisca, motor je zaseban program s ugovorom
(`qnc.transcribe`, `qnc.letter-align`), bira se kroz AI katalog racunala (`catalogs/ai/catalog.json`),
pozadinski posao ceka dok player svira. Taj obrazac je dobar i ostaje osnova za nove agente.

## 2. Gdje se gubi vrijeme u montazi vijesti

1. Trazenje pokrivalica: novinar zna sto govori OFF ("bolnica", "gradonacelnik", "promet"), a pokrivalicu
   trazi pregledavanjem svih klipova.
2. Trazenje izjave: tko je rekao sto i gdje je dobra recenica.
3. Biranje upotrebljivog kadra: mutno, trese, prekratko, crno, kamera se jos namjesta.
4. Ton: sum vjetra i prometa u izjavi, razina OFF-a prema izjavi i ambijentu.
5. Potpisi (ime, funkcija), lokacija, natpisi.
6. Zastita: lica djece, registarske tablice.
7. Pisanje: naslov, najava, sazetak, verzija za web.

## 3. Novi agenti

Prioritet: A = najveci dobitak za montazu, B = korisno, C = kasnije.

### 3.1 A: Sadrzaj kadra i sortiranje pokrivalica (trazeno)

Cilj: pokrivalice u poolu razvrstane po onome sto se u kadru vidi, i prijedlog pokrivalica za slot prema
tekstu OFF-a koji ide ispod njega.

Lanac (nadogradnja postojece analize kadrova, isto citanje kartice):

1. `qnc-shot-analysis` vec daje upotrebljive dijelove i njihovu najostriju sliku. Za svaki dio uzme se
   1-3 slike (pocetak, najostrija, kraj); kartica se ne cita ponovno.
2. Novi agent `qnc-shot-content` (zaseban program, ugovor `qnc.shot-content` 1): za svaku sliku vraca
   - vektor slike (image embedding, model tipa SigLIP / CLIP; visejezicni zbog hrvatskih upita),
   - lica: broj, velicina i polozaj (detektor lica tipa YuNet ili SCRFD),
   - tekst u kadru (OCR tipa PaddleOCR: natpisi, table, registracije),
   - vrsta plana iz velicine lica i predmeta: total, srednji, krupni, detalj.
3. Nova kocka tablica `qnc-shot-content-db`: vektori i oznake po dijelu klipa, u bazi projekta.
4. Kocka `qnc-broll-sort` (cista): razvrstavanje dijelova
   - po sadrzaju: grupe iz oznaka kroz upite teksta (ljudi, gomila, zgrada, interijer, promet, priroda,
     sport, sastanak, bolnica, skola...; popis je podatak, ne kod, i redakcija ga mijenja),
   - po vrsti plana (total -> krupni, korisno za slaganje niza),
   - po kvaliteti (mirno > jednoliki svenk > trzaj; ostrina; duljina prema boji trajanja),
   - slicni kadrovi zajedno, gotovo isti kadrovi oznaceni (duplikati).
5. Prijedlog za slot: tekst OFF-a ispod praznog slota (scenarij / prijepis) pretvori se u vektor istim
   modelom, dijelovi se poredaju po slicnosti. Klik na prijedlog = postojeci Cover slot / Overwrite. Nista
   se ne stavlja samo: agent predlaze, korisnik bira.
6. Pool: tab B-roll dobiva redoslijed i grupe; polje "Potrazi" trazi i po slici ("most", "policijski
   auto") preko istog vektora.

Brzina (procjena): model slike velicine ~200M parametara na procesoru laptopa nekoliko slika u sekundi;
uz 1-3 slike po upotrebljivom dijelu, sat materijala za nekoliko minuta. Ide iza filmstripa i vala, kao
analiza kadrova.

### 3.2 A: Trazenje po znacenju i tolerantna pretraga

- Pretraga prijepisa koja nalazi i krivo prepisane rijeci (slicnost znakova, fonetika) i oblike rijeci
  (padezi: "bolnica", "bolnici", "bolnicu"): cisto pravilo, u aplikaciji.
- Pretraga po znacenju: vektori recenica prijepisa (visejezicni model recenica), upit "izjava o
  cijenama" nalazi recenicu "sve je poskupjelo". Isti agent kao 3.1 moze nositi i tekstualni model.

### 3.3 A: Tko govori (diarizacija)

Agent razdvaja govornike u klipu (segmentacija + vektor glasa, npr. modeli koje sherpa-onnx podrzava).
Rezultat: retci prijepisa s govornikom (G1, G2...), korisnik imenuje jednom ("G2 = gradonacelnik"), ime
ide u potpis izjave. Pitanje novinara se odvaja od odgovora, pa se izjave brze nalaze.

### 3.4 A: Ciscenje govora

Smanjenje suma (vjetar, promet, klima) u izjavi i OFF-u snimljenom na terenu. DeepFilterNet postoji kao
Rust implementacija i radi u stvarnom vremenu na procesoru. Primjena kao kocka uz razinu (kao krivulja
razine): ukljuci po segmentu, cuje se u previewu, export ga primjenjuje. Original se ne mijenja.

### 3.5 B: Potpisi i natpisi

- Ime i funkcija iz govora ("ja sam Stanko Karaman, fotoreporter") i iz OCR-a (natpis na stolu, akreditacija)
  -> prijedlog potpisa za izjavu.
- Lokacija iz zapisa kamere (GPS gdje postoji) i iz OCR-a.

### 3.6 B: Zastita lica i tablica

Detektor lica i tablica (3.1 vec racuna lica) -> prijedlog zamagljivanja s pracenjem kroz kadar;
korisnik potvrdi, render ga primjenjuje. Za djecu i maloljetnike prema pravilima redakcije.

### 3.7 B: Jezicni pomocnik (lokalni LLM)

Mali lokalni model (3-8B, kvantiziran, llama.cpp s Vulkanom) nad prijepisom i scenarijem:
- prijedlog naslova, najave i odjave, sazetak za web, popis izjava s ocjenom ("najjaca recenica"),
- provjera trajanja (koliko rijeci OFF-a stane u preostale sekunde),
- grubi redoslijed: najava -> OFF -> izjava -> OFF..., kao prijedlog u scenariju.
Hrvatski je kod malih modela slabiji: obavezno mjerenje na nasim tekstovima prije odluke; tekst nikad ne
ide u prilog bez korisnika. Na laptopu je to najteze opterecenje: dobar kandidat za agenta na LAN-u.

### 3.8 C: Ostalo

- Ton okoline (pljesak, glazba, gomila, sirena) iz zvuka: oznake NAT dijelova i upozorenje na glazbu
  (autorska prava).
- Prijevod izjave stranog govornika (lokalni modeli prijevoda za hr-en) za titl.
- Titlovi iz prijepisa (vremena rijeci vec imamo).
- Okvir 9:16 za drustvene mreze (pracenje lica iz 3.1).

## 4. Sto unaprijediti

| Postojece | Unaprjedjenje |
|---|---|
| Prijepis Fast (Parakeet) | ceka se model koji podrzava zadani jezik (Nemotron kad ga sherpa-onnx podrzi); do tada wav2vec2 ponovni prolaz izjave |
| wav2vec2 Best | interpunkcija i velika slova (lokalni model interpunkcije ili LLM 3.7) |
| Analiza kadrova | sadrzaj kadra (3.1), MP4 i formati bez MXF indeksa (sada samo MXF) |
| Pomocnik tona | ciscenje govora (3.4); prepoznavanje glazbe pod govorom |
| Pretraga prijepisa | tolerantna i po znacenju (3.2) |
| Provjera OFF-a | ocjena izgovora i tempa (vec postoji postotak) |

## 5. Dio aplikacije ili samostalni agent

Pravilo:
- Cisto pravilo bez modela (razvrstavanje, pretraga, poravnanje, glasnoca) je kocka u aplikaciji.
- Sve sto ucitava model je samostalni agent (zaseban program s verzioniranim ugovorom), kao danas
  prijepis i vremena slova. Razlozi: pad modela ne rusi QNC, model se mijenja bez nove gradnje, graficka i
  procesor dijele se s playerom pod kontrolom (prioritet ispod normalnog, staje dok player svira), isti
  agent moze raditi na drugom racunalu.

Gdje agent radi (isti ugovor, bira AI katalog racunala):

| Mjesto | Kada | Uvjet |
|---|---|---|
| Lokalno (laptop) | uvijek moguce; teren bez mreze | agent i model uz aplikaciju; razina posla prema snazi racunala (kocka hardverskog testa) |
| LAN (server u redakciji) | teski modeli (LLM, sadrzaj kadra za puno materijala), vise laptopa | agent cita medij kroz QNC transport (URI -> resolver -> endpoint, nikad sirova putanja), rezultat upisuje kroz javnog pisca baze projekta |
| Intranet | dijeljeni agenti vise redakcija | isto kao LAN, s autorizacijom izvora |

Nedostaje za LAN/intranet agente: pisac baze projekta preko mreze (danas je pisac lokalni posrednik) i
red poslova agenata u bazi (tko radi koji klip, najam s otkucajima, kao red prijepisa). Prije toga agenti
rade lokalno.

Zajednicka pravila svih agenata:
1. Offline: model je datoteka na racunalu ili LAN serveru, nikakav poziv vani.
2. Agent predlaze, korisnik odlucuje: rezultat je prijedlog u bazi, nikad sam ne mijenja pricu.
3. Kartica samo za citanje; agent ne dobiva putanju nego bajtove kroz transport.
4. Player ima prednost: agent staje dok player svira.
5. Licenca svakog modela provjerava se prije beta paketa (neki popularni modeli nisu za komercijalnu
   upotrebu, npr. dio prijevodnih i vizualnih modela; detektori pod AGPL licencom se ne koriste).

## 6. Prijedlog redoslijeda

1. Sadrzaj kadra i sortiranje pokrivalica (3.1), prvo lokalno: najveci dobitak, nadogradnja postojece
   analize kadrova. Korak 1: agent `qnc-shot-content` s modelom slike i mjerenje brzine na laptopu.
2. Prijedlog pokrivalica prema tekstu OFF-a (3.1 tocka 5).
3. Tolerantna pretraga i pretraga po znacenju (3.2).
4. Tko govori (3.3) i ciscenje govora (3.4).
5. Red poslova agenata u bazi i agent na LAN serveru.
6. Lokalni LLM (3.7), zastita lica (3.6), ostalo.
