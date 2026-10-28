# Energy calibration

The device stores a quadratic calibration: `keV = a0 + a1·ch + a2·ch²`. Radiadash
reads it at connect and can fit a replacement from identified peaks.

## Why

The test unit reports K-40 at ~1443 keV instead of 1460.8 keV (−1.2 %) and Tl-208
at ~2599 instead of 2614.5 keV. A ~15 keV offset is small for isotope
identification but visible, and a bad calibration propagates into every spectrum.

## How the fit works

1. Collect matched lines from the candidate analysis: one point per peak channel,
   keeping the strongest line when several candidates claim the same peak.
   Only lines with ≥10 % intensity are used.
2. Fit `a0/a1` with two anchors, or `a0/a1/a2` with four or more.
3. Reject outliers: compute residuals, drop points beyond `max(4 keV, 3·MAD)`,
   refit, repeat up to three times.
4. Report the RMS of the remaining anchors.

## Guardrails

- Apply is disabled when the fit RMS exceeds 12 keV.
- A poor fit (>8 keV) is highlighted: it usually means a line was misidentified.
- Writing requires an explicit confirmation dialog; the old coefficients are shown
  in the card so they can be recorded first.
- The backend rejects implausible values (`a1` outside 0.1–20 keV/ch, `|a2| > 0.01`,
  `|a0| > 100`) before touching the device.

## Known limitation

The fit anchors come from candidate identification, so a wrong assignment shifts
the calibration. Prefer a spectrum with several strong, unambiguous lines
(Cs-137, K-40, Co-60) before applying.
