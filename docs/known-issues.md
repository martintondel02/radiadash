# Known issues and open questions

## Upstream dependency advisory

`glib 0.18.5` (pulled in by Tauri / gtk-rs) is flagged by RustSec for unsound
`Iterator`/`DoubleEndedIterator` impls on `VariantStrIter`. Radiadash does not use
that iterator, and the fix requires a gtk-rs major-version bump that Tauri has not
released yet. Tracked here instead of pinning an old Tauri.

## RC-110 semantics still being confirmed

1. `RareData.duration` jumps by large amounts between buffered records
   (e.g. 1.35 M → 1.87 M) while wall-clock time moves by minutes. It is probably
   not plain uptime seconds. Radiadash uses its own timestamps for sessions and
   does not rely on this field yet.
2. `RareData.charge_percent` dropped from 99 % to 83 % during a session with
   several spectrum transfers. Either the battery gauge is coarse, the field is
   not a percentage, or the BLE load is real. Needs a longer observation.
3. `DS_UNITS` reads as `1` while the device configuration and display are in
   roentgen. The register may be inverted on RC-110 or means something else.
   Radiadash prefers the configuration keys (`*_R` alarm limits) until this is
   confirmed against the display.
4. The battery field is noisy: 99 % → 61 % → 90 % within twenty minutes while
   USB-powered. It may be a gauge in different units, not a percentage.
5. `RareData` (dose, battery, temperature) is sent infrequently and unpredictably.
   M2 seeds those values from the last database row and from the temperature
   register at connect, then updates them when a new rare record arrives.

## Isotope analysis

- The device calibration currently reads K-40 at ~1443 keV instead of 1460.8
  (−1.2 %) and Tl-208 at 2599 instead of 2614.5. The UI surfaces the median
  offset; a calibration wizard is planned.
- Below ~95 keV the spectrum is dominated by a detector artefact, so low-energy
  lines are ignored except for Am-241.
- Candidates are ranked by line coverage and energy agreement only; no
  efficiency model and no activity quantification.

## UI

Map and history views are placeholders (M5/M6).
