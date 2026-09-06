# Katalog struktura zapisa kamera

Datum: 2026-09-06. Pocetni dataset: `2026.09.06.1`, schema 1.

## Svrha i granica

Katalog opisuje **sheme direktorija i pravila zapisa**, ne listu ekstenzija.
To je zasebna referentna SQLite baza, dostupna svim modulima kroz javni
ugovor. Nije Ingest baza rezultata, nova aplikacija, shared workflow servis
niti veza izmedu aplikacija.

Osnova su tvornicke specifikacije: direktoriji, recording mode, imenovanje,
indeksi/sidecari, original/proxy odnosi, segmenti i audio/metadata komponente.
Nije potrebno posjedovati karticu svake kamere. Tvornicka dokumentacija i
iz nje izvedeni testni primjeri su redovan put verifikacije; stvarna kartica
je dodatni dokaz. Nepoznata pravila ne zamjenjuju se pretpostavkom po ekstenziji.

Artefakti u `catalogs/camera-patterns/`:

- `camera-patterns-v1.sqlite`: stvarna prenosiva baza, ukljucena u Git scope.
- `contract.json`: javni ugovor referentnog kataloga i pravila citanja/promjene.
- `schema.sql`: tablice, FK veze, ogranicenja i javni viewovi.
- `seed.json`: pregledljiv izvor za reprodukciju pocetnog izdanja; nije runtime
  popis hardkodiran u aplikaciji. Nakon revizije izvor istine je novo izdanje
  baze s povijescu, ne ponovno ucitani pocetni seed.

Alat `tools/qnc-camera-catalog` je uski offline publisher/validator. Nema app
ovisnosti, UI, scanner, FFmpeg, probe, mrezu ni pristup kartici. Parametri
datoteka ovog razvojnog alata nisu javni runtime identiteti.

## Sadrzaj baze

| Podatak | Sto se sprema |
| --- | --- |
| Katalog | Identitet, schema, verzija podataka, datum pregleda |
| Izvor | Proizvodjac, dokument, URL, odjeljak/stranica, datum |
| Obrazac | Obitelj zapisa, model/mode ogranicenje, pravilo imena, grupiranje |
| Korijeni | Relativna shema direktorija i njezin opseg |
| Datoteke | Putanja unutar sheme, uloga kandidata i uvjet primjene |
| Metadata | Dokument, XML namespace, selector, znacenje podatka |
| Dokaz | Veza obrasca s izvorom i tocno sto izvor potvrdjuje |
| Status | enabled / disabled / incorrect, razlog |
| Povijest | Operacija, verzija, razlog, zapis prije i poslije promjene |
| Rupe | Sto jos nedostaje u specifikaciji; nije tvrdnja o podrsci |

`public_patterns` prikazuje i neaktivne/pogresne obrasce za administraciju.
`public_analysis_patterns` vraca samo ukljucene dokumentirane/observed obrasce
s poznatim opsegom korijena. Potrosac mora **najprije** koristiti taj view,
pa povezati roots/file_rules/metadata/evidence preko `pattern_id`. Nikad ne
smije koristiti sve `public_file_rules` kao nefiltrirani popis za scan.

Trenutno: **23 obrasca**, od toga **17 dokumentiranih kandidata za buduci
analizator** i **6 nepotpunih, iskljucenih**; dodatnih **7 research backlog
stavki**. To nisu 23 implementirana detektora i nije pokrivenost svih kamera.

| Proizvodjac / obitelj | Pocetno stanje |
| --- | --- |
| Sony XDROOT SD, M4ROOT SD/CFexpress, XDCAM EX BPAV, XDCAM MXF | Sheme upisane; Sony SD dodatno promatran |
| Sony FX6 proxy chunks | Iskljuceno do potpunog cross-card povezivanja |
| Panasonic P2 VIDEO/AUDIO, P2 AVCLIP, PANA_GRP, AVCHD | Dokumentirane razlicite strukture |
| Canon legacy DCIM | Dokumentirano, ograniceno na opisane modele/modeove |
| Canon C70 MP4 | Djelomicno; sub/continuous i firmware veze jos nisu potpune |
| RED RDM/RDC RAW+proxy i ProRes-only | Dva razlicita recording modea |
| ARRI | Primjeri i ogranicenja upisani; potrebno razdvojiti detaljne varijante |
| Blackmagic URSA Cine | Recording-relative BRAW/Proxy/companion obrazac |
| Blackmagic Cinema Camera 6K | Djelomicna struktura; precizno povezivanje jos nedostaje |
| Nikon Z9 | Tvornicko imenovanje RAW/proxy; potpuna folder shema jos nedostaje |
| GoPro standard / Labs NLE proxies | Razlicite direktorijske i naming varijante |
| DJI Osmo LRF | Poznata preview uloga, nepotpuna folder/pairing specifikacija |
| JVC ProHD BPAV / HM790 CQAV | Razdvojeni MP4/MOV direktorijski obrasci |
| Fujifilm, drugi Cinema EOS/Sony, LUMIX, DJI drone i ostali | Eksplicitni backlog, bez izmisljenih shema |

Katalog namjerno ne koristi ime kamere kao jedini kljuc: ista kamera moze
snimati razlicite formate, a jedna kartica moze sadrzavati vise obitelji.
Jednaki nazivi direktorija kod razlicitih proizvodjaca ostaju konflikt koji
treba razrijesiti dodatnim dokazima, ne redoslijedom patterna.

## Vazna pravila za buduci analizator

1. Popis direktorija i camera-written indeksi ulaze kroz transport, read-only.
   Nema skeniranja OS diska ili ffprobe poziva u formi.
2. Pronaci sve odgovarajuce recording rootove, ne stati na prvom PRIVATE/BPAV.
   Korijeni su relativni; sacuvati case i razmake. URI authority nije u obrascu.
3. Shema daje kandidate. Mode, indeksi, identiteti i dokumentirana pravila
   moraju potvrditi uloge. `*.MP4` ili `*.MXF` sami nikad nisu dovoljan dokaz.
4. Grupirati original, opcionalni proxy, poster, audio i podijeljene dijelove.
   RED ProRes-only MOV jest original; MOV u RAW+proxy modeu moze biti proxy.
   P2 AUDIO MXF nije novi video klip. Proxy bez dostupnog originala je nepotpun
   ulaz, ne samostalan original.
5. Postovati camera-written veze prije heuristic naming pravila. XML namespace
   i schema moraju biti provjereni. Bez external entities, mreznog XML fetcha
   i izlaska relativnih referenci iz odabranog izvora.
6. Katalog ne sadrzi izmjerene codec/fps/duration podatke konkretnog medija.
   Jedini Select/Ingest probe upisuje njih u bazu rezultata, ukljucujuci proxy
   podatke vezane uz original. Nema naknadnog probe fallbacka.
7. Nepoznat ili proturjecan format ostaje unresolved. Ne odbacivati takve
   datoteke nevidljivo i ne tvrditi da su podrzane.
8. Factory-only obrasci smiju se implementirati i testirati na sintetskim
   direktorijima/indeksima izvedenima iz specifikacije. Nije uvjet imati
   fizicku karticu za svaki model. Firmware i record mode moraju ostati u scopeu.

## Uredjivanje bez izmjene aplikacija

Publisher cita postojecu bazu read-only, promjene izvodi u zasebnoj memorijskoj
bazi i objavljuje novu SQLite datoteku. Ne prepisuje postojece izdanje. Nema
migracije poslovnih baza i nijedna operacija ne brise karticu ili media datoteku.

Podrzane operacije:

| Operacija | Ucinak |
| --- | --- |
| add | Dodaje cijeli obrazac s novim ID-om i izvorima |
| replace | Zamjenjuje postojeci obrazac, ukljucujuci shemu i pravila |
| delete | Uklanja obrazac i njegove vezane redove, cuva history snapshot |
| mark_incorrect | Zadrzava obrazac s razlogom, iskljucuje iz analize |
| disable | Privremeno iskljucuje bez tvrdnje da je pogresan |
| enable | Eksplicitno ukljucuje; validacija i dalje zahtijeva dokumentaciju/sheme |

`replace` nosi puni zapis, ne djelomicne skrivene promjene. `show` vraca tocnu
JSON strukturu obrasca; `add` i `replace` koriste je u polju `pattern`.
Novi izvor dokumentacije dodaje se u `sources` revizije s novim ID-om.
`base_version` sprjecava primjenu zastarjele izmjene na pogresnu verziju.
Svaka operacija zahtijeva razlog; duplicirana izmjena istog ID-a unutar iste
revizije odbija se. Za ponovno ukljucivanje neispravnog obrasca prvo ispraviti
zapis i navesti izvor; ne proglasavati nepotpun obrazac dokumentiranim bez dokaza.

Primjer datoteke revizije (administrativni primjer, nije primijenjen):

```json
{
  "base_version": "2026.09.06.1",
  "dataset_version": "2026.09.06.2",
  "changed_on": "2026-09-06",
  "sources": [],
  "changes": [
    {
      "operation": "mark_incorrect",
      "id": "sony-xdroot-sd",
      "reason": "Primjer: potreban pregled prijavljene greske; nije stvarni nalaz."
    }
  ]
}
```

Naredbe iz QNC roota:

```text
cargo run -p qnc-camera-catalog -- check catalogs/camera-patterns/camera-patterns-v1.sqlite
cargo run -p qnc-camera-catalog -- show catalogs/camera-patterns/camera-patterns-v1.sqlite sony-xdroot-sd
cargo run -p qnc-camera-catalog -- revise CURRENT.sqlite CHANGES.json NEW.sqlite
cargo run -p qnc-camera-catalog -- build catalogs/camera-patterns/seed.json NEW.sqlite
cargo test -p qnc-camera-catalog
```

Novo izdanje moze dodati/promijeniti/obrisati podatkovni obrazac bez promjene
UI-ja ili aplikacije. Ako novi proizvodjac zahtijeva dosad nepoznatu vrstu
parsera/povezivanja, potreban je novi javni module contract/adapter, ne skripta
sakrivena u bazi. Catalog trenutno nema administrativni GUI.

## Local / LAN / Intranet

Baza ne sadrzi slova diskova, OS putanje, camera/card serijske brojeve ni
identitete korisnickih projekata. Patterni su relativne sheme i stoga jednaki
na Windowsu, Linuxu i macOS-u.

Buduci runtime koristi konfigurirani QNC URI, npr.
`qnc://local/catalog/camera-patterns`, kroz resolver i read-only endpoint.
LAN/intranet authority bira konfiguracija, ne UI i ne katalog. Distribucija
nepromjenjive verzije baze nije dijeljeni aktivni Ingest servis. U ovom koraku
**nije implementiran ni testiran mrezni catalog reader**; ne tvrditi da lokalni
publisher dokazuje LAN/intranet rad aplikacije.

## Sony card observation

Dodatni read-only pregled dostavljene kartice potvrdio je `PRIVATE/XDROOT`,
`Clip`, `Sub`, `Thmbnl`, camera-written `MEDIAPRO.XML` namespace i 103 Material
zapisa sa 103 eksplicitne Proxy reference. Sidecari identificiraju ILME-FX6V.
M4ROOT struktura i ILME-FX3A sidecari takodjer su promatrani. Sadrzaj M4ROOT
se razlikovao izmedju uzastopnih citanja pa se ne koristi fiksni broj FX3A
klipova kao dokaz ni kao testni fixture. Ne pretpostavlja se uzrok promjene.

Na karticu nije pisan ni obrisan nijedan podatak ovim korakom. Nije pokrenut
ffprobe/ffmpeg. U katalog se spremaju pravila/anonimizirani dokazi, ne stvarni
serijski brojevi kartice/kamere, UMID-i ili korisnicki nazivi klipova.

## Verifikacija i sljedeci korak

Izvrsene su provjere sheme, FK/integriteta, izvora, neutralnih putanja,
duplikata, zabrane nepoznatih executable polja, dokumentacije bez kartice,
svih sest uredjivackih operacija, pogresnog obrasca, stale revisiona,
neuspjele batch promjene, odbijanja tudje/modificirane baze, read-only otvaranja
i zabrane prepisivanja. Paket baze usporedjuje se s reproduciranim seedom.

Rezultat: `cargo test --workspace` 97/97 (od toga 16 testova kataloga),
`qnc-conformance` sve provjere prolaze, `cargo clippy -p qnc-camera-catalog
--all-targets -- -D warnings` prolazi.

To su **testovi kataloga/publishera**, ne test izvrsnog prepoznavanja svih
navedenih formata. Nema UI promjene pa nema novog UI live koraka.

Sljedece: dokumentaciju nepotpunih obitelji dopuniti konkretnim tvornickim
shema/naming/metadata odjeljcima; zatim javni camera-detector/scanner povezati
s katalogom preko ugovora. Za svaki parser napraviti pozitivne i negativne
fixtures iz specifikacija: mjesovita kartica, isti extension razlicite uloge,
spanned clip, odsutan proxy, proxy-only ulaz, duplicirani nazivi, krivi
namespace, pogresan obrazac i nepoznat recording mode. Tek tada govoriti o
runtime podrsci.

Postojece poslovne baze, Project, Ingest runtime i UI nisu mijenjani ovim
korakom. Nisu provjereni Linux/macOS/ARM executable ni mrezni end-to-end rad.
