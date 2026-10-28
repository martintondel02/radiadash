# Device notes

Observations from real hardware, kept so firmware quirks stay documented.

## RadiaCode 110 — serial `RC-110-007780`

| | |
| --- | --- |
| Firmware | 4.14 (2025-07-07) |
| Bootloader | 4.0 (2024-08-30) |
| Hardware serial | `0037002A-59335017-20343552` |
| Spectrum format | 1 |
| Config | `DType=4`, `Id=9`, `SpecFormatVersion=1`, `FrameTimeStep_us=10000` |
| Connection | BLE only in our first session (USB not yet tested on this unit) |

### What works

- BLE connect without pairing, GATT write/notify, 4 KB spectrum transfer
- `data_buf`, `spectrum`, `configuration`, `GET_VERSION`, `GET_SERIAL`
- Energy calibration from the device (`a0=4.281989`, `a1=2.367718`, `a2=0.000342982`)

### Quirks

1. **No `CPS` / `DR_uR_h` / `DS_uR` registers.** Register reads of `0x8020`,
   `0x8021`, `0x8022` return "invalid". Live values must come from the data
   buffer; `TEMP_degC` (0x8024) and `VBIAS_mV` (0xC000) do read.
2. **Real-time dose rate is in R/h.** Values around `4.1e-6` correspond to
   ≈0.041 µSv/h. Multiply by 1e4 for µSv/h.
3. **Accumulated dose in `RareData` is a float.** The 32-bit word around
   `999_955_360` is not an integer count; reinterpreted as `f32` it reads
   `0.00472 R` (≈47 µSv), which fits 15.6 days at background. The Python
   reference library treats this field as `u32`, so RC-110 needs the float
   interpretation.
4. **`DS_UNITS` reads as `1`** even though the device display/config is in
   roentgen (`DR_AlarmMax_R_h`, `DS_AlarmMax_R`). Treat the unit flag as
   "device configured for Sv" only after checking against the display; the
   config keys are currently the more reliable signal.
5. `flags=0x4040` on every real-time record at background level; meaning not
   yet decoded.

### Open questions

- Exact `flags` bits for RC-110.
- Whether `DS_UNITS` semantics differ between RC-103 and RC-110.
- USB (bulk endpoint) behavior on this unit.
- Dose-rate vs CPS agreement with the Cs-137 sensitivity spec (77 cps ≈ 1 µSv/h).
