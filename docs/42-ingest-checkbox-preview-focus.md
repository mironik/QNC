# Ingest checkbox i preview fokus

Korisnicka uputa 2026-09-08: checkbox ne dodaje crveni okvir. Crveni okvir
pripada samo klipu odabranom za pregled/reprodukciju.

Referenca je qnc_v4/qnc-app/src/qnc_media_card.rs: MediaCardInput ima
neovisne focused i checked podatke. Time se ispravlja pogresno preslikavanje,
ne uvodi novi playback workflow.

- checked dolazi iz ClipView.selected i crta samo checkbox.
- focused dolazi iz IngestViewModel.preview_clip_id i crta crveni okvir.
- Klik checkboxa salje postojeci ingest_clip_toggle; klik kartice izvan
  checkboxa salje ingest_preview_focus. Ne salju se obje akcije.
- Fokus ne mijenja checkbox ni DB odabir; checkbox ne mijenja fokus.
- Nema promjene baze, Selecta, probea, playera, transporta ili projektnog koda.

Plan provjere: kombinacije checked/focused, jedan fokus kroz grid, odvojeni
klik intenti i DB potvrda selekcije bez promjene fokusa, zatim Windows live.

## Rezultat

- 4/4 desktop testova prolaze: sve checked/focused kombinacije, samo jedan
  crveni okvir u gridu i odvojeni intenti za klik checkboxa i tijelo kartice.
- Ciljani DB integracijski test prolazi: preview ne pokrece DB odabir;
  potvrdeni checkbox upis ne mijenja prethodno odabrani preview klip.
- Build qnc-ingest i conformance prolaze.
- Windows live novog builda: u korisnickom prikazu Mironik 1491-1494 imaju
  oznacene checkboxove bez crvenog okvira; samo Mironik 1487 ima crveni
  preview okvir, s neoznacenim checkboxom. Nova aplikacija ostaje pokrenuta.
- Reprodukcija nije dodana niti testirana ovim UI zahvatom.
