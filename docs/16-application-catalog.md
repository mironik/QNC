# Samostalni katalog aplikacija

Datum: 2026-09-06

## Korisnicki dogovor

- Katalog kreira samostalan alat na osnovu postojecih registriranih aplikacija.
- Zapis smije biti JSON ili baza. Ovaj korak koristi JSON.
- Svaka registracija ima `priority_group`, jedno malo slovo a-z.
- Vise aplikacija moze biti u istoj grupi: razlicite izvedbe iste faze.
- Jedan template smije odabrati najvise jednu aplikaciju iz svake grupe.
  Izborom jedne ostale iz te grupe postaju nedostupne za taj template.
- Grupa se oslobada uklanjanjem izbora. Nema globalnog zakljucavanja grupe
  za druge templatee ili korisnike.
- Odabrane aplikacije slijede red grupa: a-b-c ili a-c-e, nikad a-e-c.
- Nema tvrdog popisa Project/Ingest/Story u generatoru. Grupu daje registracija.
- Ne postoji obveza odabira svih grupa; pravila pojedinog templatea ostaju u
  njegovoj komponenti, ne u katalogu.

## Implementirani alat

`tools/qnc-app-catalog` je samostalan Rust executable bez GUI-ja i bez ovisnosti
na Project, Ingest, njihove storeove ili shell. Zavrsava nakon naredbe.
Ne poziva drugu aplikaciju, ne pristupa projektnim bazama niti medijima.

Ulaz su `qnc-app.json` datoteke u neposrednim poddirektorijima eksplicitno
zadane mape registracija. Izvrsne datoteke trazi u zasebno zadanoj instalacijskoj
mapi, bez trazenja po PATH-u ili pokretanja aplikacija. Platformski executable
suffix dodaje adapter; registracija sadrzi samo neutralni basename.

Registracije Projecta i Ingesta imaju grupe a i b. Numericki `order` je
uklonjen. Katalog, provjera izbora i shell koriste samo `priority_group`.
Stari razvojni zapisi ne migriraju se u novi slijed.

## Zapis

Ugovori:

- `contracts/modules/application-catalog.module.json`
- `catalogs/applications/contract.json`

Lokalni generirani zapis je `data/application-catalog.json` i nije dio Gita:
opisuje prisutnost aplikacija na konkretnoj instalaciji, ne popis izvornog koda.

Polje `applications` sadrzi samo dostupne aplikacije, sortirane po grupi.
Unutar grupe label i ID daju stabilan prikaz alternativa, ne redoslijed
izvrsavanja. Polje `unavailable` je dijagnostika i nikad nije izvor izbornika.

Za dostupnost su potrebni valjana registracija, `enabled: true` i neprazna
regularna izvrsna datoteka. Unix dodatno zahtijeva execute permission.
To ne dokazuje ABI kompatibilnost, uspjesno pokretanje ili postojanje embedded
adaptera u konkretnom shellu; katalog ne pokrece procese da bi to provjeravao.

Osvjezavanje uklanja nestale registracije, a registracije bez executablea
premjesta iz dostupnih u dijagnostiku. Neispravan ulaz ili duplicirani ID/tab
prekida objavu i zadrzava prethodni ispravan zapis. Vise varijanti iste grupe
nije greska. Prazna provjerena mapa daje prazan popis, bez default aplikacija.

Objava koristi potpunu privremenu datoteku i atomsku zamjenu. Smije zamijeniti
samo postojeci ispravan katalog istog QNC URI-ja, nikad proizvoljni JSON ili
poslovnu bazu. Ne mijenja postojece templatee/projekte.

## Pokretanje

Iz QNC root direktorija, za trenutnu razvojnu instalaciju:

```text
cargo run -p qnc-app-catalog -- refresh apps target/debug data/application-catalog.json
cargo run -p qnc-app-catalog -- check data/application-catalog.json
cargo run -p qnc-app-catalog -- show data/application-catalog.json
cargo run -p qnc-app-catalog -- select data/application-catalog.json qnc.ingest qnc.project
```

Nakon builda dovoljan je executable `qnc-app-catalog` (Windows: `.exe`), bez
Cargoa ili drugih QNC procesa. Ulazne/izlazne putanje su privatna konfiguracija
tog alata, ne javna referenca koju forme razmjenjuju.

`select` je read-only upit/validacija: prihvaca ID-jeve odabranih aplikacija,
odbija nepoznate/nedostupne ID-jeve i dvije aplikacije iste grupe, te vraca
JSON odabira sortiran po grupi. Ne rezervira grupu globalno i ne zapisuje
template. Project komponenta sada koristi zajednicku validaciju iz javnog
`qnc-application-catalog` modula. Radio grupe u pasivnom UI-ju zamjenjuju
odabranu varijantu; owner ponovno provjerava grupe i slijed prije spremanja.

## Local / LAN / Intranet

Alat se pokrece na racunalu cija se instalacija popisuje. Opcionalni zadnji
argument `refresh` naredbe je javni identitet izlaznog artefakta:

```text
qnc://local/catalog/applications
qnc://lan/studio/catalog/applications
qnc://intranet/studio/catalog/applications
```

U JSON se ne zapisuju lokalne OS putanje. Isti zapis moze biti dostupan preko
read-only resolver/proxy endpointa. Generator ne otkriva udaljene instalacije
i ne pokrece HTTP posluzitelj. Javni reader sada cita HTTP(S) kroz resolver;
testovi imaju stvarni loopback HTTP endpoint za LAN i intranet URI-je.
To nije provjera fizicke LAN/intranet mreze ili Linux/macOS GUI-ja.

Katalog je snapshot s `observed_at_unix_ms`, ne stalni nadzor. Osvjeziti ga
nakon instalacije/uklanjanja/promjene registracije i prije osvjezavanja
izbornika. Sam stari snapshot nije jamstvo trenutne dostupnosti aplikacije.

## Verifikacija i granica koraka

- Ciljani testovi: filtriranje dostupnosti, nestanak executablea/registracije,
  neispravni ulazi, ocuvanje prethodnog zapisa, neutralni identiteti, grupiranje,
  jedna varijanta po grupi, redoslijed a-c-d, izolacija izbora razlicitih
  templatea i read-only provjera.
- Live CLI nad stvarnim `apps` i `target/debug` daje a: Project, b: Ingest.
- Obrnuti zahtjev `qnc.ingest qnc.project` vraca Project pa Ingest.
- Windows: 20 ciljanih testova alata i clippy prolaze. Workspace testovi i
  conformance prolaze; dodatno je provjereno ograniceno odmrzavanje Projecta.
- Projects UI i spremanje templatea/projekta povezani su s javnim readerom.
  Shell citanje kataloga i automatski prelazak jos nisu implementirani.
  Nema prijenosa poslovnog stanja izmedu aplikacija.
- Odmrzavanje Projecta za dogovoreni nastavak zapisano je u
  `docs/11-project-freeze.md`; cijeli odobreni zahvat jos nije zavrsen.
