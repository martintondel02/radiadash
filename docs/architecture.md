# Architecture

## Layers

```
┌──────────────────────────── Tauri app ────────────────────────────┐
│ React UI (widgets, charts, map, audio)                            │
│        ▲ commands / events (typed with ts-rs, M2)                 │
│        ▼                                                          │
│ Rust app core                                                     │
│  ├ DeviceManager   connection policy, reconnect, session state    │
│  ├ Pipeline        normalize units, dedupe, ring buffers          │
│  ├ Store           SQLite (WAL): samples, spectra, sessions, map  │
│  └ Analysis        peaks, calibration, isotope scoring            │
└───────────────────────────────────────────────────────────────────┘
```

`crates/radiadash-core` is UI-agnostic and synchronous-free: all transports are
async (`tokio`), so the same code drives the probe CLI and the Tauri runtime.

## Connection policy

1. If a Bluetooth address is configured, use it.
2. Otherwise scan for a device whose name contains `radia` (10 s default).
3. Fall back to USB (`0483:f123`), optionally filtered by serial number.
4. Reconnect with exponential backoff; surface a diagnostic log to the UI.

## Data flow (M2)

```
transport → RadiaCode session (1 Hz poll, spectrum every N s)
          → decoder → canonical record (µSv/h, µSv, cps, °C, %)
          → ring buffer (UI, 10 min)  +  SQLite writer (batched, 1 s)
          → Tauri event (throttled 4 Hz) → React store → widgets
```

History queries never scan the raw table for long ranges; rollups (1 min, 1 h,
1 day) are computed on write.

## Units

One canonical unit per quantity: µSv, µSv/h, cps, °C, keV. Display units are a UI
concern. Gy/rad require an explicit "absorbed dose" assumption (photon factor 1.0
by default) and are labelled accordingly; R is exposure and is converted with
1 R ≈ 0.01 Sv.

## Storage schema (M2)

```sql
sessions(id, started_at, ended_at, name, notes, device_serial, unit_config)
samples(id, session_id, ts, cps, cps_err, dose_rate_usvh, dose_rate_err, temp_c, flags)
rare(session_id, ts, dose_usv, duration_s, charge_pct, flags)
spectra(id, session_id, ts, duration_s, a0, a1, a2, counts BLOB, kind)
rollups(bucket, session_id, start_ts, n, cps_mean, cps_max, dose_rate_mean, dose_rate_max)
tracks(id, session_id, ts, lat, lon, alt, source)
events(id, session_id, ts, kind, payload)
settings(key, value)
```

## Isotope identification (M3)

1. Build the energy axis from calibration.
2. Estimate background (SNIP), smooth, find peaks with a FWHM prior
   (~8.4 % at 662 keV for CsI(Tl)).
3. Net peak areas with uncertainty; reject escape/backscatter artefacts.
4. Score library nuclides by matched intensity-weighted lines, penalising strong
   unexplained lines; require two lines for "high" confidence.
5. Present candidates with an evidence table and an overlay of library lines on the
   measured spectrum. Never claim quantitative activity.

## Geotagging (M5)

RC-110 has no GPS. Sources: manual pin, `gpsd`, GPX import, `.rctrk` import from the
phone app (matched by timestamp), and fixed-site mode. MapLibre GL with OSM raster
tiles by default; offline PMTiles optional.

## Testing

- Decoder unit tests with hand-built frames and recorded fixtures.
- Property-style checks for unit conversions.
- Synthetic spectra with known peaks (golden files) for peak/isotope logic.
- Simulator-backed UI tests (Playwright, M4+).
- Python reference implementation used as an oracle for new fixtures (dev only).
