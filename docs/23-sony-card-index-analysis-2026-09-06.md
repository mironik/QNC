# Sony: shema kartice, indeksi i vec zapisani media podaci

Datum: 2026-09-06. Dopuna audita `docs/22-ingest-state-audit-2026-09-06.md`.
Ovo je analiza i prijedlog sljedeceg koraka, ne implementirani detector.

## Korisnicki zahtjev

Nakon izbora kartice modul treba iz kataloga shema znati gdje traziti
originale, proxyje, slicice i metapodatke. Kamerine datoteke koje sluze kao
indeks sadrzaja treba procitati i koristiti njihove veze i vec zapisane
tehnicke podatke. Ne pocinjati slijepim trazenjem svih video ekstenzija.

Naknadno korisnicko pojasnjenje: datoteke mogu biti kopirane. Odluka se
donosi prema raspolozivom zapisu, ne pretpostavljenom porijeklu datoteke.
Potpuni upotrebljivi podaci koriste se bez ffprobea. Ako zapis nedostaje ili
nema potrebne podatke, njih pribavlja ffprobe samo za taj medij, tijekom
jedinog Select/Ingest prolaza. Ne pokretati ffprobe za sve zbog nepotpunosti
nekih zapisa. Kasnije aplikacije i moduli nikad ne ponavljaju probe.

Forma ostaje pasivna. Citanje kataloga i izvora pripada javnom modulu kroz
transport, jednako na local/LAN/Intranet. Projektna aplikacija ne sudjeluje.

## Dokaz na dostupnoj kartici

Procitani su direktoriji, `MEDIAPRO.XML` i 103 pripadajuca XML sidecara.
Izvor je citan read-only. Nije pozvan ffprobe/ffmpeg, nije citan video za
dekodiranje, nije pokrenut import. Nista nije zapisano na karticu.

Kartica sada sadrzi `PRIVATE/XDROOT` i `PRIVATE/M4ROOT`. XDROOT sidecari
identificiraju `ILME-FX6V`, softver `6.00`. M4ROOT indeks i pregledane mape
CLIP/SUB/THMBNL/TAKE sada su prazni. Raniji FX3A nalaz u docs/15 nije dokaz
trenutnog broja klipova u M4ROOT i nije zamijenjen pretpostavkom.

| Provjera XDROOT | Nalaz ovog citanja |
| --- | --- |
| Material zapisi u indeksu | 103 |
| Eksplicitne originalne reference | 103 |
| Eksplicitne proxy reference | 103 |
| Eksplicitne XML reference | 103 |
| Eksplicitne JPG reference | 103 |
| Nedostajuce datoteke medju 412 referenci | 0 |
| Sidecar TargetMaterial/umidRef razlicit od roditeljskog Material/umid | 0 |
| Sidecar Duration razlicit od originalnog Material/dur | 0 |
| Originali s dur/fps/ch/videoType/audioType | 103 |
| Proxyji s uri/type/videoType/audioType/umid | 103 |
| Proxyji s dur/fps/ch/aspectRatio | 22 |
| Proxyji bez dur/fps/ch/aspectRatio | 81 |
| Sukob dur/fps kod 22 proxyja koji imaju oba polja | 0 |
| Velicina MEDIAPRO.XML | 66 064 bajta |
| Zbroj 103 XML sidecara | 325 534 bajta |

SHA-256 indeksa jednak je prije i poslije citanja. To nije transakcijski
snapshot cijele kartice. Provjerene su reference i XML veze, ne integritet
videa, potpuna frame-ekvivalentnost proxyja ili ispravnost dekodiranja.

**81 nedostajuca proxy polja nisu 81 dokazana konflikta.** Nedostaje podatak,
ne postoji dokaz da se proxy FPS ili trajanje razlikuju od originala.
Ovaj nalaz takodjer ne dokazuje uzrok: kopiranje, izmjenu ili nacin na koji
je kamera zapisala indeks. Nijedan od tih uzroka nije utvrdjen auditom.
Ni 22 zapisa s navedenim poljima jos nisu dokaz potpunosti svih polja
buduceg player/probe ugovora; kriterij mora biti cijeli potreban skup.

## Konkretna shema FX6 uzorka

Naziv `CLIP_A` ispod je anonimizirani primjer, ne obavezni prefiks kamere.

```text
PRIVATE/XDROOT/
  MEDIAPRO.XML
  Clip/
    CLIP_A.MXF
    CLIP_AM01.XML
  Sub/
    CLIP_AS03.MP4
  Thmbnl/
    CLIP_AT01.JPG
  Edit/
  General/
  Take/
  UserData/
```

Namespace indeksa: `http://xmlns.sony.net/pro/metadata/mediaprofile`.
Ovo su stvarni selektori iz pregledanog indeksa:

| Relativno na MediaProfile | Znacenje |
| --- | --- |
| Contents/Material/@uri | Original, npr. ./Clip/CLIP_A.MXF |
| Contents/Material/Proxy/@uri | Proxy tog roditeljskog originala |
| Contents/Material/RelevantInfo[@type='XML']/@uri | Pripadajuci sidecar |
| Contents/Material/RelevantInfo[@type='JPG']/@uri | Pripadajuca kamerina slicica |
| Contents/Material/@umid | Identitet originalnog materijala |
| Contents/Material/@dur, @fps | Deklarirano trajanje u jedinicama zapisa i FPS |
| Contents/Material/@videoType, @audioType, @ch | Video/audio opis i kanali |

Proxy ima vlastiti UMID; na ovom uzorku nijedan nije jednak originalnom
UMID-u. Povezuje ga roditeljski Material i njegova eksplicitna Proxy veza.
Ne zahtijevati jednaki UMID originala i proxyja. Sidecarov `umidRef` jest
veza na original. Naziv i nastavci S03/M01/T01 dopunski su dokaz, ne zamjena
za vec zapisane veze. Nazivi na kartici mogu sadrzavati razmake.

## Sto vec postoji u XML-u klipa

Namespace sidecara ovog uzorka:
`urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20`.

Svih 103 sidecara imaju sljedece relevantne elemente. Vrijednosti u
primjerima opisuju pregledani uzorak, ne svaki Sony model i recording mode.

| Element/atribut | Upotrebljiv podatak |
| --- | --- |
| TargetMaterial/@umidRef | Veza na original |
| Duration/@value | Broj jedinica trajanja; uz odgovarajuci timebase |
| CreationDate/@value | Kamera-zapisani datum/vrijeme s UTC pomakom |
| VideoFormat/VideoFrame/@videoCodec | Npr. AVC50_1920_1080_H422P@L42 |
| VideoFormat/VideoFrame/@captureFps | Npr. 50.00p |
| VideoFormat/VideoFrame/@formatFps | Npr. 50p |
| VideoFormat/VideoLayout | Sirina 1920, visina 1080, aspectRatio 16:9, flip |
| AudioFormat/@numOfChannel, AudioRecPort | 4 kanala, LPCM24, raspored CH1-CH4 |
| SubStream/@codec | Deklarirani proxy video codec |
| LtcChangeTable | TC baza, halfStep i promjene na pozicijama |
| Device | Proizvodjac, model, serijski broj kamere, softver |
| AcquisitionRecord | Camera metadata; u primjeru gamma/primaries/coding rec709 |

Detaljno procitani primjer ima 9260 jedinica, format 50p i Duration=9260,
uz zavrsnu frameCount poziciju 9259. Tumacenjem toga kao 50 frameova/s
trajanje je 185,2 s. Istodobno LTC zapis ima tcFps=25 i halfStep=true.
**TC bazu nije ispravno prepisati kao video FPS.** Za druge timebaseove,
slow/quick modeove i namespace verzije treba dokumentirano mapiranje.

CreationDate treba sacuvati s originalnim vremenskim pomakom; filesystem
datum kopiranja nije datum snimanja. Serijski broj kamere, camera mediaId,
serijski broj volumena i fizicke kartice razliciti su identiteti.

Indeks i sidecar jesu koristan deklarirani media opis, ali nisu automatski
potpuni decoder/probe ugovor. U detaljnom primjeru nema svih eksplicitnih
stream indeksa, stream timebaseova, audio sample ratea i ostalih potrebnih
polja. Polja o proxyju posebno su nepotpuna u 81 zapisu. Bez media probea
ovim auditom nisu potvrdeni stvarni sadrzaj kontejnera i svi streamovi.

## Sony nije jedna univerzalna shema

Sony i u Catalyst dokumentaciji razlikuje XD, M4, PX, AXS/CINEROOT, BPAV
i AVCHD obitelji. To podupire katalog obrazaca po obitelji/modeu/mediju,
ne jedan `if Sony` i ne prepoznavanje samo po ekstenziji.
[Sony Catalyst: podrzane strukture](https://helpguide.sony.net/di-app/cb/v1/en/Content/Supported_devices.htm).

- FX3/XAVC MP4: originali CLIP, proxyji SUB; SD koristi PRIVATE/M4ROOT,
  CFexpress Type A M4ROOT. MP4 stoga moze biti original, ne samo proxy.
  [Sony FX3: prijenos i struktura medija](https://helpguide.sony.net/ilc/2210/v1/en/contents/TP1000868300.html).
- XDCAM EX: BPAV, podmape CLPR s medijima klipova i metapodatkovne veze;
  drugaciji je raspored od XDROOT/Clip. Sony upozorava da su nazivi i
  direktoriji povezani s metapodacima. Ne treba izjednaciti broj datoteka
  s brojem logickih klipova.
  [Sony XDCAM EX Clip Browser, str. 39-40](https://pro.sony/s3/cms-static-content/operation-manual/3280782151.pdf).
- FX6 chunk proxy: dokumentiran je poseban GENERAL/PXTMP put i moguce
  odvojene kartice za original i proxy segmente. To nije obicni Sub obrazac
  opazen na dostupnoj kartici. Takav ulaz treba odnos original-segmenti;
  proxy-only kartica ne pretvara proxy u original.
  [Sony FX6 v6, Proxy Recording, str. 60](https://pro.sony/support/res/manuals/5024/c3bfbc891ee0f149e46d142754fd6aa7/50244581M.pdf).

Prazan ili ostecen indeks ne dokazuje prazan cijeli medij. Kandidati iz
poznatih direktorija mogu pokazati neindeksirane datoteke; ne smiju biti
nevidljivo odbaceni. Neprovjerene veze ostaju unresolved. Ne rekonstruirati
kamerinu bazu na izvornoj kartici.

## Sto QNC vec ima, a sto nema

`catalogs/camera-patterns/camera-patterns-v1.sqlite` i publisher vec postoje.
Sony XDROOT obrazac vec opisuje gore navedene direktorije, tri vrste URI
veza, UMID, model i CreationDate. Ne treba stvarati drugi katalog.

Postojeci `metadata_field` ima document_pattern, namespace, selector i
meaning. To je opis, ne izvrsno mapiranje u tipizirani media ugovor.
U XDROOT seed metapodacima jos nisu pobrojana polja Duration, VideoFormat,
AudioFormat, LTC i AcquisitionRecord. M4ROOT metadata popis je prazan.
To su konkretna mjesta dopune, ne razlog za novi monolit ili ponovni scan
svakog foldera. U ovom auditu seed i objavljena baza nisu mijenjani.

Stari v4 ima scanner i suffix/directory grupiranje u
`qnc-host/src/ingest/scanner.rs` i `qnc-host/src/media/resolve.rs`.
Pretraga tih ingest/media izvora nije nasla MEDIAPRO/NonRealTimeMeta reader.
Novi katalog je bolja osnova za camera-index dio od slijepog vracanja
stare heuristike. To ne znaci da ostale uske v4 rutine treba pisati ponovno.

## Predlozeni slijed za implementaciju

1. Izabrani source URI i read-only katalog ulaze u javni analizator izvora.
   Shema ogranicava trazenje na odgovarajuce recording rootove i indekse;
   pregledaju se svi odgovarajuci rootovi na mjesovitoj kartici.
2. Citac indeksa vraca original i povezane proxy/thumbnail/sidecar reference.
   Provjerava ih kroz transport. Ne otvara svaki video samo da sazna njegovu
   ulogu i ne pokrece ffprobe.
3. Citac metapodataka izvlaci deklarirana polja. Rezultat cuva originalnu
   vrijednost, podrijetlo (dokument/selector/schema), tipizirano tumacenje
   i oznaku nedostajuceg ili konfliktnog podatka. Proxy polja se ne pune
   kopiranjem codec/audio/timebase podataka originala.
4. Ingest komponenta zapisuje grupirane zapise u vlastitu bazu kratkim
   transakcijama. UI dobiva zapise i postojece thumb URI-je u batchevima.
   Postojeca kamerina slicica moze se prikazati bez izvodenja novog postera.
5. Prema potrebnim poljima javnog media ugovora utvrdi se sto zapis vec
   daje. Ako je upotrebljiv zapis potpun, ne izvrsava se ffprobe. Ako
   nedostaje zapis ili potreban podatak, Media Probe tijekom istog
   Select/Ingest prolaza jednom poziva ffprobe za odgovarajucu datoteku
   i popunjava rezultat. To vrijedi i za datoteke kopirane bez sidecara.
   Ne radi se dodatni kontrolni ffprobe nad svim vec potpunim zapisima.
   Original i proxy zadrzavaju svoje tehnicke podatke unutar jednog
   logickog klipa; proxy time ne postaje odvojeni klip. Rezultat se zapisuje
   u Ingest bazu, nikad natrag u kamerin XML ili bazu na kartici.
6. Filmstrip, player, wave i ostali kasnije koriste samo rezultat iz baze.
   Nedostatak obaveznog podatka je greska ingest ugovora, ne novi probe.

Ovo je poboljsanje ulaznog plana: **shema -> kamerin indeks/metapodaci ->
usmjereni popis i veze -> ffprobe samo gdje nedostaju potrebni podaci ->
potpuna baza**. Sav probe ostaje u jednom Select/Ingest prolazu.
Ne mijenja DB-only odnos aplikacija niti postojecu podjelu odgovornosti.

Katalog drzi deklarativne obrasce, ne izvrsne skripte. Nepoznat parser ili
semantika zahtijeva uski javni adapter/ugovor. Ne uvoditi poslovni workflow,
FFmpeg, UI ili vlasnistvo baza u camera-patterns publisher/reader.

## Verifikacijska granica

Potvrdjeni su factory dokumenti za navedene obitelji i trenutni FX6 XML
uzorak. XML audit je koristio namespace-aware parser s DTD zabranom i bez
vanjskog XML resolvera; reference su provjerene unutar recording roota.
U trajni izvjestaj nisu kopirani osobni nazivi klipova, UMID-i ili serijski
brojevi. Ove PowerShell naredbe bile su read-only istrazivanje, ne nova
Windows-specific runtime implementacija.

Nisu provjereni dekodiranje, puni probe, LAN/intranet reader, FX3 sadrzaj,
chunk/spanned zapisi ni drugi OS/CPU. Nema izmjene runtime koda, AGENTS,
objavljenog kataloga ili poslovnih baza. Nema novog implementacijskog testa.

Sljedeci uski korak za potvrdu: dopuna postojeceg opisa Sony metadata polja
i javni camera-index reader s pozitivnim/negativnim fixtures, bez media
obrade. Testirati i prazna proxy polja, razlicite UMID-e, vise rootova,
nepotpun/stari indeks, namespace verzije i izlazak URI-ja iz izvora.
