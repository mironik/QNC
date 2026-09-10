# Audio clock and monitor delivery

Scope: public audio-output, pixel-convert, player-runtime and player-client.
No Project changes, new probe, database changes or keyboard bindings.

The live baseline on Mironik 2002 showed approximately 75 ms of audio lead
at CPU UI paint, with playback stopping on an audio underrun. Measurement
does not include physical display or speaker latency.

Implementation contract, recorded before the monitor raster change:

- The audio device's estimated playback position is the playback clock when
  audio exists. It cannot advance beyond PCM actually submitted to the device.
  Silent sources retain the existing monotonic clock.
- Native output keeps the saved full raster and source identity. A passive
  GUI monitor uses a separate aspect-preserving preview, at most 960 x 540,
  without upscaling. This is not a new media file or metadata override.
- Pixel conversion and preview resizing run on the bounded conversion worker,
  not the UI or playback owner thread. Output headers describe actual raster
  dimensions; source frame numbers, timestamps and formats remain unchanged.
- Resizing uses fast_image_resize 6.1.0 with bilinear convolution and reusable
  buffers. No hand-written image resampler or platform-specific path.
- The 8-bit preview resizes the three YUV planes before RGB conversion;
  ten-bit conversion retains its validated conversion path and scales the
  resulting RGBA. Source metadata and the native output are untouched.
- UI state and shortcut dispatch remain passive and catalog-driven.
- The process client can notify a passive display when its view changes.
  Ingest connects this to request_repaint, not a UI playback clock.
- Monitor preparation uses a bounded half-second A/V read-ahead (8..120
  frames from the saved source timebase), prepared before Play. Native output
  retains its previous buffer policy. This adds no fixed A/V offset.

Primary dependency reference: https://docs.rs/fast_image_resize/6.1.0/fast_image_resize/struct.ResizeOptions.html

Live verification and remaining limitations are recorded below after testing.

## Windows Ingest live result, 2026-09-09

Log: `target/ingest-av-buffered.stderr.log`, session
`9501b0eb-0b00-4695-907a-61aea4d1693c`. Mironik 2002 played to its final
frame 10193 of 10194 at 50 fps (203.88 seconds), without the earlier underrun
stop. The test used actual Ingest and its catalog-driven keyboard actions.

`node tools/diagnostics/av-offset.mjs target/ingest-av-buffered.stderr.log 50 1`:

- 12168 CPU paint observations; audio lead median 26.57 ms, p95 50.42 ms.
- After the first 5 seconds: median 26.45 ms, p95 49.82 ms.
- Fresh-frame transfer median 7.37 ms, p95 13.60 ms.
- Residual spikes reached 363.63 ms. The monitor is not yet a frame-locked
  physical broadcast output; these results are not a claim of zero A/V offset.
- Mironik 2676 also played, paused and stepped forward/back via the existing
  keyboard catalog. This is not a completed full-length test for that clip.

This measurement preceded the original mono audio correction in docs/64:
the old route used proxy AAC and two device channels. It must not be cited
as verification of four discrete physical outputs or their A/V offset.
No physical speaker/display calibration or real LAN/Intranet latency test
was performed. Software transport tests do not replace those live tests.
