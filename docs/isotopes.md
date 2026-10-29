# Isotope identification

Radiadash ranks isotope candidates from a measured spectrum. It is deliberately
conservative: results are *candidates* with an evidence table, not a certified
identification.

## Library

`data/isotopes/library.json` (version `2026.10-r2`) contains 74 nuclides and
196 gamma lines:

- NORM chains: K-40, U-238 series (Th-234, Ra-226, Pb-214, Bi-214), Th-232 series
  (Ac-228, Pb-212, Tl-208), U-235
- Medical: Tc-99m, I-131, Lu-177, Sm-153, In-111, Ga-67, F-18, Mo-99, I-125, Xe-133
- Industrial / calibration: Cs-137, Co-60, Am-241, Ba-133, Eu-152, Na-22, Ir-192,
  Co-57, Se-75, Sr-85
- Fallout / activation: Cs-134, Ce-144, Ru-106, Zr-95, Sb-125, Ag-110m

Energies follow ENSDF values; intensities are rounded and intended for
identification only. Verify against NNDC/ENSDF before quantitative use.

## Pipeline

1. **Baseline**: SNIP with a log–log transform, 28 iterations.
2. **Peaks**: 3-point smoothing, local maxima that are ≥4σ above the baseline,
   with net ≥25 counts and significance ≥5. Widths outside 0.25–2.2× the
   expected FWHM are rejected (this removes the RC-110 low-energy continuum).
3. **Resolution model**: `FWHM(E) = E · 0.084 · sqrt(662/E)` keV, from the
   RC-110's 8.4 % FWHM at 662 keV.
4. **Matching**: each library line is compared with peaks within
   `max(2, 0.35 · FWHM(E))` keV, capped at 30 keV. Matches score by relative
   energy distance; line coverage is intensity-weighted.
5. **Ranking**: `score = coverage × mean match quality`; lines below 95 keV are
   ignored (except Am-241's 59.5 keV check-source line) because the RC-110
   spectrum there is dominated by detector artefacts.

### Confidence

| Level | Rule |
| --- | --- |
| high | ≥2 matched lines, ≥50 % intensity coverage, score ≥0.65 and at least one line ≥20 % intensity |
| medium | one strong line (≥50 % intensity, within 1.5 keV), or ≥2 lines with ≥25 % coverage, or score ≥0.45 |
| low | any other match |

## Known limitations

- No detector efficiency curve, so peak areas are not converted to activities.
- A single line can belong to several nuclides (511 keV, 583 keV, 609 keV…).
- With a large library, single-line coincidences are common; that is why a single
  line can never reach "high" confidence.
- The device's energy calibration can be off by ~1 %; the UI shows the median
  offset of matched lines as a hint.
- Escape peaks, backscatter and sum peaks are not modelled.

## Adding an isotope

Add an entry to `data/isotopes/library.json`, bump `version`, and run the tests.
Keep lines above 95 keV unless the nuclide has no high-energy line.
