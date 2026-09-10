# Broadcast Player: plan popravka zajednickog playback puta

Datum: 2026-09-09.
Status: prvi korak (odjeljak 4) korisnik je odobrio i implementiran je u
postojecim javnim modulima. Testovi prolaze, ali live kriterij NIJE zatvoren:
Mironik 2002 dosao je do kraja; Mironik 2679 u prvom prolazu stao je zbog
nespremnog A/V framea uz jos popunjen audio red. Nalazi su u docs/73.
Odjeljak 5 djelomicno je ispitan GPU raster adapterom (docs/74): priprema
je ubrzana, ali live kriterij je PAO. Korisnik 2026-09-10 i dalje vidi trzaje
i preskakanje te odbacuje nastavak zakrpa u tom smjeru. Isporuka i stvarna
prezentacija frameova nisu rijesene. Ne nastavljati implementaciju bez
dogovorenog ispravka izlaznog modela. Odjeljak 6 ostaje plan; nema odobrenja
za kopiranje aktivnog v4 koda.

## 1. Polaziste i provjera DB-first

Popravlja se postojeci javni QNC player, ne gradi treci engine niti vraca
stara aplikacija kao monolit. V4 je read-only referenca za frame-based
Source/Program model, pripremu sljedeceg izvora i GPU monitor.

Provjeren je stvarni zapis aktivnog projekta `bbvvcx` kroz read-only SQLite:
`audio.channels = 2`, `audio.sample_rate = 48000`,
`playback.input = proxy_if_available`. Projekt postoji; nove postavke nisu
potrebne. Projektni FPS nije zamjena za spremljeni FPS izvornog klipa.

Postojeci put koji ostaje:

1. `qnc-work-settings/src/local.rs`: oznaka aktivnog projekta iz javnog
   registra, resolver binding projektne baze, `public_project_settings`,
   read-only open i `query_only`.
2. `qnc-ingest-components/src/playback.rs::prepare_preview`: komponenta
   koristi javni reader, bez ovisnosti o Projects aplikaciji ili shellu.
3. `qnc-player-input::InputReader`: cita postavke i spremljeni media snapshot;
   bira original/proxy sliku i cuva izvorne originalne audio streamove.
4. `qnc-player-runtime/src/input.rs::project_channel_map`: primjenjuje broj
   izlaza iz baze. Nepodrzana promjena sample ratea daje gresku, ne laznu
   pretvorbu ili lokalni default.

Projektni kod/postavke, Ingest Select/probe, UI layout i keyboard katalog
nisu predmet ovog plana. Kartica ostaje read-only. Nema novog probea.

## 2. Sto kod i mjerenja stvarno pokazuju

| Mjesto | Nalaz | Posljedica za plan |
| --- | --- | --- |
| `qnc-broadcast-player/src/transport_engine.rs::decode_frame_to_buffer` | Video se priprema prije audija; `NotReady` slike prekida funkciju | Audio treba zaseban neblokirajuci put pripreme |
| `tick_playing`, `refill_playout_buffer`, `top_up_audio_output_queue` | Jedan decode kursor; dopunjavanje audio izlaza dolazi iza zajednicke pripreme | Nije dovoljno samo zamijeniti dva poziva ili povecati buffer |
| `qnc-player-runtime/src/output.rs::render_audio_for_frame` | Audio vec ima kontinuirane dekodere i neblokirajuci `try_next_packet` | Iskoristiti postojeci rad, ne stvarati drugi player |
| `qnc-player-runtime/src/lib.rs::tick` | Nativni GPU `inflight` moze zaustaviti cijeli engine tick | Cekanje video izlaza ne smije zaustaviti pripremu audija |
| `qnc-player-runtime/src/output.rs::Presenter` | HTTP monitor put nema nativni GPU `inflight` | Prethodni red nije dokaz uzroka aktualnog HTTP zastoja |
| Docs/71 | Serijska obrada oko 9.7 ms u prosjeku ipak je imala underrun; paralelni pokus nije pomogao | Prosjek ispod 20 ms nije dokaz stabilne reprodukcije |
| Docs/70 i docs/71 | Monitor transfer ima zastoje oko 320 ms | Popravak audija sam ne popravlja isporuku slike |

`Ready` prije Playa i provjera novog `(generation, sequence)` vec postoje.
Ne prikazivati ih kao nedostajuce funkcije. Dev optimizacije konverzije vec
postoje; `cargo run` sam po sebi ih ne ukida. Mjerenje CPU predaje egui slike
nije dokaz vremena prikaza na ekranu niti fizicke A/V sinkronizacije.

## 3. Ciljni tok unutar jedne player sesije

```text
Javni read-only DB ugovori
  projektne postavke + spremljeni media/stream podaci
                         |
                 javni player input
                         |
        +----------------+----------------+
        |                                 |
  VIDEO priprema                    AUDIO priprema
  decode / obrada                   decode / mapa kanala
  ograniceni red frameova           ograniceni red uzoraka
        |                                 |
        +--------------+------------------+
                       |
           Broadcast Player: JEDAN sat
           frame/sample rokovi i lifecycle
                       |
          +------------+-------------+
          |                          |
     video izlaz                audio uredaj
          |
  pasivni Monitor / Timeline
  potvrdjeno stanje + neutralne naredbe natrag playeru
```

Dva kursora pripreme nisu dva sata niti dva playheada. Source identitet,
generation, frame i tocne sample granice veza su izmedu redova. Oba koriste
istu racionalnu vremensku osnovu; UI ne racuna napredovanje reprodukcije.

Pripremljeni audio mora moci u izlazni red i kad se buduca slika jos
obradjuje. To NE dopusta neogranicen nastavak zvuka bez pripadajuce slike:
ako obvezni A/V izlaz stvarno propusti rok, sesija prijavljuje kvar i
kontrolirano zaustavlja reprodukciju. Nema preskakanja frameova, podmetanja
tisine, preimenovanja PTS-a ili prikrivenog automatskog nastavka.

Obrada, cekanje dekodera/mreze i konverzija ostaju izvan vremenski osjetljivog
izlaznog puta. Audio callback samo preuzima vec spremne uzorke i daje timing;
ne cita DB, ne dekodira, ne ceka UI/mrezu niti alocira media buffere.
Ne uvoditi dodatne procese ili thread pool bez pokazane potrebe.

## 4. Prvi implementacijski korak: neovisna priprema

Opseg je postojeci `qnc-broadcast-player`, `qnc-player-runtime` i njihovi
javni ugovori/testovi. Audio-output mijenja se samo ako je za ovaj korak
potrebna uska neblokirajuca queue/telemetry operacija, ne novi audio model.

1. U engineu razdvojiti sljedeci video frame i sljedeci audio sample raspon.
   Svaki red ima vlastiti tvrdi kapacitet i ograniceni rad po prolazu.
   Jedan `NotReady` ne preskace polling/dopunjavanje drugog reda.
2. Audio koji je spreman dopunjava izlaz bez cekanja konverzije buduce slike.
   Za stvarno dospjeli frame i dalje vrijedi zajednicka kontrola spremnosti.
   Drzati razliku izmedu decoded, queued, submitted i presented.
3. U runtimeu ukloniti uvjet da pending video izlaz zaustavlja svu pripremu.
   Ne premjestati blokirajuci decode ili konverziju na owner/audio callback.
4. Zadrzati strogi `Ready`: otvoreni resursi, pripremljen pocetni A/V raspon,
   spreman izlaz. Play koristi te resurse. Pause cuva resurse i ogranicava
   memoriju; seek priprema tocno trazeni frame.
5. Seek/promjena klipa ponistava oba reda istom generacijom. Novi klip prvo
   potpuno prekida stari. Kasni rezultati ne smiju vratiti staru sliku/zvuk.
   Thumbnail ostaje u monitoru do Playa, kao i sada.

Prije prvog live pokretanja izgraditi kompatibilan Ingest i sibling player
iz istog stabla/profila te provjeriti tocnu putanju pokrenutog helpera.
Ne brisati cijeli target niti ubijati druge procese kao redovni popravak.

Prvi korak prolazi tek kada ciljani testovi potvrde neovisno dopunjavanje
pri odgodjenoj video pripremi, granice memorije u Ready/Pause, tocne sample
granice, otkazivanje i iskrenu gresku pri stvarnom underrunu. Zatim se kroz
stvarnu Ingest formu ponavlja Mironik 2679 i 2002, cijeli klip, Pause/Play,
seek i promjena klipa. Helper test nije zamjena za taj live test.

## 5. Drugi korak: slika bez univerzalnog CPU RGBA uskog grla

Javni video/monitor adapter treba prihvatiti opisane video ravnine ili
izlazni frame resurs. Za podrzani YUV format pretvorba boje i skaliranje za
monitor mogu ici na GPU, kao u v4. Neutralni ugovor mora nositi stvarni
pixel format, dimenzije, stride, bit depth i color/range iz spremljenih
podataka. Ne kopirati v4 pretpostavku da je sve 8-bit limited Rec.709.

UI ostaje pasivan: javni monitor adapter prikazuje frame, ne posjeduje sat
ili decode. Zasebni output adapteri koriste isti engine; SDI/NDI nisu novi
playeri. Njihova implementacija nije dio prvog koraka.

Za isti host moze se koristiti ograniceni shared-memory prijenos s jasnim
vlasnistvom slotova i generacijom. Lokalni handle/mmap nije LAN/Intranet
ugovor. Udaljeni adapter mora imati vlastitu provjeru propusnosti, latencije,
spremnosti i otkazivanja, preko istog verzioniranog frame/state ugovora.
Ne zamjenjivati HTTP samo po nazivu protokola bez mjerenja.

Odvojeno mjeriti receive, upload/submit i stvarni present, gdje ga backend
moze potvrditi. UI ACK nije dozvola da audio callback ceka UI. Istodobno se
zastoji monitora ne smiju sakriti proglasavanjem monitora nebitnim izlazom:
ako je to jedini korisnicki video izlaz, njegov neuspjeh rusi live kriterij.
Fizicku A/V sinkronizaciju ne proglasiti potvrdenom samo CPU timestampovima.

## 6. Priprema za montazu bez drugog enginea

V4 `program_playlist.rs` daje koristan raspored source rangeova, audio
busova i ogranicenu pripremu sljedeceg izvora. Nije gotov opci compositor:
bira jedan video izvor po program frameu i odbija mijesane source FPS-ove.

Source klip je najjednostavniji ulaz istom playeru. Program ulaz je plan
frame rangeova iz javnog DB/EDL ugovora, pripremljen javnom komponentom,
ne lista koju aktivno vodi forma. Stotinu uzastopnih klipova ne znaci
stotinu istodobno otvorenih dekodera: pripremaju se aktivni i ograniceni
sljedeci izvori, uz limit memorije, broja dekodera i vremena pripreme.

A1 OFF/izjava i A2 ambijent su uloge zapisanog montaznog plana, ne automatski
stereo miks niti zakljucak iz CH1/CH2 source previewa. Istodobni video slojevi
i prijelazi zahtijevaju zasebnu javnu composition komponentu; ne dodavati ih
u Ingest formu ili prosirivati player aplikacijskim if/switch granama.

Prije tog koraka dogovoriti tocno mapiranje program/source frameova i
sample rangeova. Razlicit FPS zahtijeva stvarni ugovor pretvorbe; ne mijenjati
metapodatak bez obrade niti koristiti project/export FPS kao source fallback.
Za sada se ne uvodi novi Story UI, Project polje ili zamjenska poslovna baza.

Provjera montaze mora ukljuciti rez na tocnoj granici, ograniceni prewarm,
odvojene A1/A2 zapise i promjenu izvora bez zaostatka starog framea/PCM-a.
Uspjeh jednog dugog klipa nije potvrda ove faze niti opce profesionalne montaze.

## 7. Sto se ne prenosi iz v4

- Neograniceno dopunjavanje engine buffera pri Pause/Ready.
- Audio izbor samo `0:a:0` i automatski stereo downmix.
- `Playing` prije dovrsene stvarne pripreme.
- Nastavak s tihim audio rupama pri underrunu.
- Cijeli `qnc-media-ffmpeg` paket s probeom/generatorima ili app playback stack.
- Lokalni mmap kao navodna potvrda udaljenog playbacka.

FFmpeg ostaje zamjenjivi decode adapter. Ne mijenja se vec dogovoreni model
dekoderskog kataloga niti uvodi automatski drugi backend.

## 8. Dokaz i sljedeca odluka

U izvornom planskom prolazu: procitani aktualni QNC/v4 kod i pravila, provjeren stvarni
read-only DB zapis i put primjene postavki; zapisan ovaj plan.
Nisu mijenjani engine, aplikacije, baze ni kartica. Nisu ponovljeni build,
unit testovi ili live Play jer implementacija nije mijenjana.

Reference: AGENTS 4.1/8.2/14/15; docs/61 (v4 nalazi, ne zastarjela usporedba
novog runtimea), docs/66, docs/67, docs/70-player-stutter-environment i docs/71.
Izvorni v4 putovi: `qnc-broadcast-player/src/transport_engine.rs`,
`qnc-player-runtime/src/program_playlist.rs`,
`qnc-player-workstation-monitor/src/lib.rs` i `src/yuv420.wgsl`.

Izvorni prijedlog odluke: potvrditi samo korak 4 (neovisna priprema u
postojecim javnim modulima) i njegov Ingest live kriterij. Koraci 5 i 6
ostaju obvezni nastavak za isporuku slike i dokaz montaznog modela, ne
tvrdnja da su rijeseni ovim prvim popravkom. Windows/local uspjeh ne
certificira Linux/macOS ili LAN/Intranet; to trazi zasebne stvarne provjere.

Nakon korisnikove potvrde provedeni su korak 4 i Ingest live test (docs/73).
Neovisna priprema nije rijesila video rokove ni HTTP monitor zastoje.
Sljedeca odluka odnosi se na odjeljak 5; ne dodavati veci buffer ili novi
engine radi prikrivanja preostalog kvara. Ovaj zapis ne zatvara live kriterij.
