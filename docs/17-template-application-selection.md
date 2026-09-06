# Project: izbor aplikacija iz kataloga

Odobreni zahvat 2026-09-06, prije izmjene UI-ja.

Referenca: qnc_v4/qnc-app/src/project/settings.rs, sekcija
`Plugin tabovi u workflowu`. Zadrzavaju se sekcija, font i razmak.
Odobreno odstupanje: popis dolazi iskljucivo iz generiranog kataloga; oznaka
grupe prikazuje se uz naziv. Sve dostupne aplikacije, ukljucujuci varijante,
slijede grupe a-z. Naknadna izricita uputa korisnika: option/radio dugmad po
grupama zamjenjuju checkboxe. Izbor druge varijante zamjenjuje prethodnu.
Opcija `Bez odabira` preskace neobaveznu grupu. Grupa a mora imati odabranu
aplikaciju: ako je Project jedini, ostaje odabran; dostupna alternativa moze
ga zamijeniti. Ovo provjeravaju Project komponenta i owner, ne javni UI modul.
Stanje pripada samo trenutnom templateu.
Naknadna uputa za raspored: grupe su stupci jedan uz drugi u nevidljivoj
tablici, varijante su redovi unutar svojeg stupca. Na uskom prikazu koristi
se horizontalni scroll, bez premjestanja grupa jedne ispod druge.
Pasivni obrazac za option stupce pripada javnom qnc-ui-kit modulu, ne browseru.

Katalog nije poslovni bridge i njegov generator se ne poziva iz forme.
Javni reader/selection modul nema app/store/egui ovisnosti. Project komponenta
ucitava katalog i obraduje intent; forma samo crta pripremljene stavke.
Ucitavanje kataloga ide u pozadinsku nit preko resolvera. Lokalno se URI
veze na privatnu datoteku; LAN/intranet koristi HTTP(S) GET kroz authority
konfiguraciju. Nema OS patha u spremljenom izboru.

Nova workspace.application_selection vrijednost sadrzi verziju, catalog URI,
vrijeme snapshota i sortirane application_id/tab_id/label/priority_group.
Owner provjerava integritet prije novog upisa. workspace.tabs je izvedeni
kompatibilni prikaz, a workflow rows nose isti slijed i metadata u settings_json.
Stari template se samo cita: nema migracije ni prepisivanja postojece baze.
Nedostupne legacy stavke ne prikazuju se. Radni izbor je presjek spremljenog
izbora i dostupnih aplikacija. Ne blokira kreiranje zbog nedostupne legacy
stavke i ne trazi dodatno uklanjanje. Izvorni seed/template ostaje nepromijenjen.

Nova akcija bez hardkodirane tipke: project_workflow_group_select.
Shell automatizacija nije dio ovog koraka i ne smije se tvrditi da radi.

Verifikacija: ciljani testovi kataloga, transporta, izbora i DB zapisa;
workspace testovi i conformance; live standalone Project prije zatvaranja.

## Transport konfiguracija

Bez overridea owner veze `qnc://local/catalog/applications` na privatni
`data/application-catalog.json`. Generator se pokrece zasebno nakon promjene
instalacije. Project ucitava objavljeni katalog pri pokretanju. Nema UI dugmeta
za osvjezavanje, skeniranja instalacije niti generiranja popisa iz forme.

Za udaljeni katalog vlasnik instalacije postavlja okolinske varijable prije
pokretanja standalone aplikacije ili shella:

- `QNC_APPLICATION_CATALOG_URI`: `qnc://lan/studio/catalog/applications`
  ili `qnc://intranet/studio/catalog/applications`.
- `QNC_APPLICATION_CATALOG_ENDPOINT`: HTTP(S) base URL proxyja za authority.
  Reader trazi `<base>/catalog/applications`; JSON mora nositi trazeni QNC URI.

Reader: read-only, limit 8 MiB, timeout 10 sekundi, bez redirecta, TLS provjera
ukljucena. Ne otkriva racunala ni aplikacije. Endpoint mora biti konfiguriran
i dostupan kroz postojeci proxy/HTTP servis; novi posluzitelj nije dio zahvata.

## DB zapis

Oba owner ulaza za kreiranje projekta i korisnickog templatea zahtijevaju
`SelectionSnapshot`. Ne mogu upisati dva izbora iste grupe ni obrnuti slijed.
Novi javni view `public_project_application_sequence` iz projektne baze daje
application_id, tab_id, priority_group, position i veze sljedeceg koraka.
Poslovna komunikacija ostaje samo citanje baze. Postojece baze nisu migrirane.

## Provjereno 2026-09-06

- `cargo test --workspace --quiet`: 131 test prolazi.
- Conformance: svi ugovori i dependency/boundary provjere prolaze.
- Strogi clippy prolazi za catalog reader, publisher i Project store.
  Project desktop ima prethodni `needless_borrows_for_generic_args` u
  location_browser.rs:254; uz izuzece samo tog linta ostale provjere prolaze.
- Novi Project i shell executablei izgradeni su iz ovog workspacea.
- Live standalone Project: vidljive samo grupe a/Project i b/Ingest,
  option/radio kontrole i Bez odabira; nema Media Assist/Story placeholdera.
- Korisnik je u novom procesu kreirao `test--novog odabira`.
  Read-only provjera stvarne globalne i projektne baze potvrduje zapis,
  javni slijed qnc.project/a/0 pa qnc.ingest/b/1 i postojeci default export.
- Regresija iz prvog reza (blokiranje kreiranja zbog nedostupnih stavki u
  starom seed/templateu) uklonjena je i pokrivena testom.

Nije live provjereno: fizicka LAN/intranet instalacija, Linux/macOS GUI,
vise stvarno instaliranih varijanti iste grupe. Grupna iskljucivost i stvarni
HTTP reader testirani su izoliranim fixtureima. Shell automatska navigacija
jos nije implementirana. Ostatak Projects audita nije zatvoren ovim korakom.
