# Radiadash user guide

The same content is available in-app via the **?** button in the header, which
opens the guide for the current page.

## Getting started

Radiadash connects to a RadiaCode 110 over Bluetooth or USB and stores everything
locally.

1. Turn the detector on and keep it near the computer.
2. Click **Connect** in the sidebar (auto: Bluetooth first, USB fallback), or open
   *transport options* to force one.
3. The first spectrum appears within a minute; the status pill turns teal.

**If the device is not found:** check the cable, close the phone app (it holds the
Bluetooth connection), and run `radiadash-probe list` in a terminal. USB needs the
udev rule from `packaging/60-radiacode.rules` installed under
`/etc/udev/rules.d/`.

## Overview

The default workspace combines the live count rate, dose rate, accumulated dose,
device status, spectrum and isotope candidates.

- **Zoom any chart**: scroll or pinch around the cursor, drag to pan, `−` / `+`
  for stepped zoom, `⟲` to reset.
- **Units**: the header selects dose-rate, dose and count-rate units independently.
- **Density**: Comfortable / Compact / Focus changes the grid.
- **Sound**: the header button toggles Geiger audio; volume is on the Device page.

## Spectrum and isotopes

Bars are counts on a log scale; colour emphasises stronger peaks. Zoom into a
region to inspect a peak.

- **Amber sticks** are library lines matched to detected peaks. **Dashed grey
  sticks** are expected lines that were not observed.
- **Candidates** are ranked by intensity-weighted line coverage and energy
  agreement. Click one to switch the overlay and evidence table.
- **Confidence**: high requires two or more matched lines with good coverage, or
  one strong line at the right energy. Single-line nuclides (K-40, Cs-137) are
  usually medium.
- **Δ** is measured minus expected energy. A median offset above a few keV means
  the device calibration is drifting.

Isotope results are candidates, not a certified identification.

## Rate and dose

- 5m/1h use the live session; 6h/24h query SQLite and survive restarts.
- CPS and dose-rate histograms show mean and standard deviation.
- Zooming affects the loaded data only and does not re-query the database.

## Survey map

The RC-110 has no GPS (the GPS model is the RC-103G), so locations are imported or
tagged manually.

**Import a track**

1. Record the survey in the Radiacode phone app and export it as `.rctrk`.
2. Open **Map** → *Import .rctrk / .gpx*.
3. Points appear coloured by dose rate; click one for time, dose and CPS.

**Tag the current reading**

Place the map, use *Use map centre* or type coordinates, then click *Tag current
reading*. The point stores the live dose rate and CPS.

**Time filter and export**

The two sliders limit which points are shown. Export writes GeoJSON, GPX or CSV to
`~/Downloads` (or the app data directory) and shows the full path.

Colours: teal < 0.2 µSv/h, amber < 0.6, orange < 2, red above, grey for sources
without a dose value. Imported `.rctrk` dose rates are assumed to be µSv/h.

## Device and calibration

**Calibration wizard**

1. Collect a spectrum with several known lines (Cs-137, K-40, Co-60 are ideal).
2. Click *Fit from identified peaks* and check the anchor table and RMS.
3. If RMS is above ~8 keV, do not apply — lines were probably misidentified.
4. Record the current coefficients, then *Apply to device* and confirm.

**Geiger audio**

Clicks are generated from the measured CPS with Poisson jitter; it is a
sonification, not a second measurement.

**Battery and rare records**

Battery readings on the RC-110 are noisy and may jump. Rare records (battery,
temperature, accumulated dose) arrive infrequently, so values are seeded from the
last stored record when you connect.

## History

Every connection creates a session. Samples (1 Hz), rare records, spectra and
events are stored in
`~/.local/share/io.github.martintondel.radiadash/radiadash.db`. Browsing and
export are planned for a later milestone; for now, open the database with any
SQLite client (schema in `docs/architecture.md`).

## Privacy and limitations

- Data stays local. Map tiles are fetched from OpenStreetMap when the map is open.
- Isotope identification is a candidate ranking, not metrology.
- Dose values are converted from the device's roentgen-based output and are
  indicative, not certified.
- Geiger audio is derived from the measured count rate.
