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
   Radiadash prefers the configuration keys until this is resolved.
4. USB transport is implemented but has not yet been exercised on this unit.

## UI

The M0 UI is a static scaffold with demo values; it is not connected to the device
until M2.
