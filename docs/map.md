# Survey map

The RC-110 has no GPS; the GPS model is the RC-103G. Radiadash therefore treats
location as an import or a manual action, not something the detector provides.

## Import formats

### `.rctrk` (Radiacode phone app)

JSON with a `markers` array (or a bare array). Each marker is read tolerantly:

| Field | Accepted keys |
| --- | --- |
| latitude | `lat`, `latitude` |
| longitude | `lon`, `lng`, `longitude` |
| dose rate | `doseRate`, `dose_rate`, `dose_rate_usvh` |
| count rate | `countRate`, `count_rate`, `cps` |
| time | `date`, `timestamp`, `time` (epoch s/ms or RFC 3339) |

Invalid coordinates are skipped. The dose-rate value is assumed to be **µSv/h**;
the phone app's exported unit is not documented, so treat imported values as
indicative until confirmed.

### GPX

`<trkpt>` and `<wpt>` elements with `lat`/`lon` attributes; `<time>` is optional.
GPX points carry no dose rate and render neutral grey.

## Storage

Imported points go to the `track_points` table (session-independent) with
`imported_at`, `ts`, coordinates, dose rate, count rate and source name. The map
fits to the bounds of the imported track once.

## Rendering

- MapLibre GL with raster OSM tiles (network required; the style is inline so a
  custom tile server can be swapped in later).
- Track points are **DOM markers**, not a vector layer. Vector layers do not
  render on software GL stacks (verified under Xvfb/llvmpipe), and markers keep
  the app usable without GPU acceleration. Marker count is capped at 500 by
  striding.
- Colour ramp: teal < 0.2 µSv/h, amber < 0.6, orange < 2, red above; grey when
  the source has no dose rate.
- Clicking a marker shows time, dose rate and count rate.

## Not implemented yet

- Live geotagging from `gpsd`, a USB GPS dongle, or manual pin placement
- Time filtering and heatmap aggregation
- GeoJSON/GPX export
