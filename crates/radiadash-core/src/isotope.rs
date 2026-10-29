//! Gamma-line library and spectrum analysis for isotope candidate ranking.
//!
//! The pipeline is deliberately conservative:
//! 1. estimate a baseline with SNIP,
//! 2. find significant peaks with a CsI(Tl) FWHM prior,
//! 3. match library lines within an energy tolerance,
//! 4. score candidates by line coverage and match quality.
//!
//! Results are *candidates*, not a certified identification: a single line can
//! come from several nuclides, and the detector's efficiency is not modelled.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::spectrum::channel_to_energy;
use crate::types::Spectrum;

const LIBRARY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/isotopes/library.json"
));

/// Resolution at 662 keV for the RC-110 CsI(Tl) crystal.
const RESOLUTION_AT_662: f64 = 0.084;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub review: String,
    pub isotopes: Vec<Isotope>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Isotope {
    pub id: String,
    pub name: String,
    pub half_life: String,
    pub category: String,
    pub lines: Vec<GammaLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GammaLine {
    pub energy_kev: f64,
    /// Relative gamma intensity in percent.
    pub intensity: f64,
}

pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| serde_json::from_str(LIBRARY_JSON).expect("embedded isotope library is valid"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peak {
    pub channel: usize,
    pub energy_kev: f64,
    /// Net area after baseline subtraction.
    pub net: f64,
    pub net_err: f64,
    pub significance: f64,
    pub fwhm_kev: f64,
    pub gross: f64,
    pub background: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub line_energy_kev: f64,
    pub intensity: f64,
    pub matched: bool,
    pub peak_channel: Option<usize>,
    pub peak_energy_kev: Option<f64>,
    pub delta_kev: Option<f64>,
    pub significance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub half_life: String,
    pub category: String,
    pub confidence: String,
    pub score: f64,
    pub coverage: f64,
    pub matched_lines: usize,
    pub total_lines: usize,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub peaks: Vec<Peak>,
    pub candidates: Vec<Candidate>,
    pub library_version: String,
    pub duration_s: u64,
    pub total_counts: u64,
}

/// CsI(Tl) FWHM model: R(E) = 8.4 % * sqrt(662/E).
pub fn fwhm_kev(energy_kev: f64) -> f64 {
    let energy = energy_kev.max(30.0);
    energy * RESOLUTION_AT_662 * (662.0 / energy).sqrt()
}

/// Full analysis of a spectrum: peaks plus ranked isotope candidates.
pub fn analyze(spectrum: &Spectrum) -> Analysis {
    let peaks = detect_peaks(spectrum);
    let candidates = rank(&peaks);

    Analysis {
        peaks,
        candidates,
        library_version: library().version.clone(),
        duration_s: spectrum.duration_s,
        total_counts: spectrum.total_counts(),
    }
}

fn smooth3(values: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(values.len());
    for index in 0..values.len() {
        let lo = index.saturating_sub(1);
        let hi = (index + 1).min(values.len() - 1);
        let window = &values[lo..=hi];
        out.push(window.iter().sum::<f64>() / window.len() as f64);
    }
    out
}

/// SNIP baseline estimation (log-log transform, iterative clipping).
fn snip(counts: &[f64], iterations: usize) -> Vec<f64> {
    let n = counts.len();
    let mut transformed: Vec<f64> = counts
        .iter()
        .map(|&y| {
            let s = (y.max(0.0) + 1.0).sqrt();
            (s + 1.0).ln().ln_1p()
        })
        .collect();

    for p in (1..=iterations.min(n / 2)).rev() {
        for i in p..n - p {
            let candidate = 0.5 * (transformed[i - p] + transformed[i + p]);
            if candidate < transformed[i] {
                transformed[i] = candidate;
            }
        }
    }

    transformed
        .into_iter()
        .map(|v| {
            let inner = v.exp() - 1.0; // ln(sqrt(y+1) + 1)
            let root = inner.exp() - 1.0; // sqrt(y+1)
            (root * root - 1.0).max(0.0)
        })
        .collect()
}

fn detect_peaks(spectrum: &Spectrum) -> Vec<Peak> {
    if spectrum.counts.len() < 16 || spectrum.a1 <= 0.0 {
        return Vec::new();
    }

    let counts: Vec<f64> = spectrum.counts.iter().map(|&c| c as f64).collect();
    let smooth = smooth3(&counts);
    let background = snip(&counts, 28);
    let a1 = spectrum.a1 as f64;

    let mut peaks: Vec<Peak> = Vec::new();
    for i in 2..counts.len() - 2 {
        let is_max = smooth[i] > smooth[i - 1]
            && smooth[i] >= smooth[i + 1]
            && smooth[i] > smooth[i - 2]
            && smooth[i] >= smooth[i + 2];
        if !is_max {
            continue;
        }

        let energy = channel_to_energy(i, spectrum.a0, spectrum.a1, spectrum.a2) as f64;
        if energy < 30.0 {
            continue;
        }

        let baseline = background[i].max(0.0);
        let above = smooth[i] - baseline;
        if above < 4.0 * (baseline + 1.0).sqrt() {
            continue;
        }

        let fwhm = fwhm_kev(energy);
        let fwhm_channels = (fwhm / a1).max(2.0);
        let half = (fwhm_channels * 0.75).ceil() as usize;
        let lo = i.saturating_sub(half);
        let hi = (i + half).min(counts.len() - 1);

        let gross: f64 = counts[lo..=hi].iter().sum();
        let baseline_sum: f64 = background[lo..=hi].iter().sum();
        let net = gross - baseline_sum;
        let err = (gross + baseline_sum).sqrt().max(1.0);
        let significance = net / err;

        if net < 25.0 || significance < 5.0 {
            continue;
        }

        // Reject structures much broader than the detector resolution (the
        // low-energy continuum and electronics artefacts look like this).
        let half_level = baseline + (smooth[i] - baseline) / 2.0;
        let mut left_edge = i;
        while left_edge > lo && counts[left_edge] > half_level {
            left_edge -= 1;
        }
        let mut right_edge = i;
        while right_edge < hi && counts[right_edge] > half_level {
            right_edge += 1;
        }
        let measured_width = (right_edge - left_edge) as f64;
        if measured_width > 2.2 * fwhm_channels || measured_width < 0.25 * fwhm_channels {
            continue;
        }

        peaks.push(Peak {
            channel: i,
            energy_kev: energy,
            net,
            net_err: err,
            significance,
            fwhm_kev: fwhm,
            gross,
            background: baseline_sum,
        });
    }

    // Merge peaks that are closer than a third of the FWHM.
    peaks.sort_by_key(|peak| peak.channel);
    let mut merged: Vec<Peak> = Vec::new();
    for peak in peaks {
        match merged.last_mut() {
            Some(last) if peak.channel - last.channel < 3 => {
                if peak.net > last.net {
                    *last = peak;
                }
            }
            _ => merged.push(peak),
        }
    }
    merged
}

fn best_peak(peaks: &[Peak], energy: f64, tolerance: f64) -> Option<&Peak> {
    peaks
        .iter()
        .filter(|peak| (peak.energy_kev - energy).abs() <= tolerance)
        .max_by(|a, b| {
            let qa = a.net / tolerance.max(1.0);
            let qb = b.net / tolerance.max(1.0);
            qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn confidence(matched: usize, coverage: f64, score: f64, evidence: &[Evidence]) -> &'static str {
    // A single line is never enough for "high": with a 74-nuclide library many
    // peaks coincide with some line. High needs two or more matched lines with
    // real coverage, at least one of them reasonably strong.
    let has_strong_line = evidence.iter().any(|item| item.matched && item.intensity >= 20.0);
    let strong_single = matched == 1
        && evidence.iter().any(|item| {
            item.matched
                && item.intensity >= 50.0
                && item.delta_kev.map(|delta| delta.abs() < 1.5).unwrap_or(false)
        });

    if matched >= 2 && coverage >= 0.5 && score >= 0.65 && has_strong_line {
        "high"
    } else if strong_single || (matched >= 2 && coverage >= 0.25) || score >= 0.45 {
        "medium"
    } else if matched >= 1 {
        "low"
    } else {
        "none"
    }
}

fn rank(peaks: &[Peak]) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    for isotope in &library().isotopes {
        let mut evidence = Vec::new();
        let mut matched = 0usize;
        let mut total = 0usize;
        let mut matched_intensity = 0.0;
        let mut total_intensity = 0.0;
        let mut quality_sum = 0.0;

        // Below ~95 keV the RC-110 spectrum is dominated by detector X-rays and
        // continuum artefacts, so those lines are ignored except for Am-241,
        // whose 59.5 keV line is a standard check-source signature.
        let low_energy_ok = isotope.id == "Am-241";
        for line in isotope.lines.iter().filter(|line| {
            line.intensity >= 1.0 && (line.energy_kev >= 95.0 || (low_energy_ok && line.energy_kev >= 55.0))
        }) {
            total += 1;
            total_intensity += line.intensity;
            let tolerance = (0.35 * fwhm_kev(line.energy_kev)).clamp(2.0, 30.0);

            match best_peak(peaks, line.energy_kev, tolerance) {
                Some(peak) => {
                    matched += 1;
                    matched_intensity += line.intensity;
                    let delta = peak.energy_kev - line.energy_kev;
                    let quality = (1.0 - 0.6 * delta.abs() / tolerance).clamp(0.4, 1.0);
                    quality_sum += quality;
                    evidence.push(Evidence {
                        line_energy_kev: line.energy_kev,
                        intensity: line.intensity,
                        matched: true,
                        peak_channel: Some(peak.channel),
                        peak_energy_kev: Some(peak.energy_kev),
                        delta_kev: Some(delta),
                        significance: Some(peak.significance),
                    });
                }
                None => evidence.push(Evidence {
                    line_energy_kev: line.energy_kev,
                    intensity: line.intensity,
                    matched: false,
                    peak_channel: None,
                    peak_energy_kev: None,
                    delta_kev: None,
                    significance: None,
                }),
            }
        }

        if matched == 0 || total_intensity <= 0.0 {
            continue;
        }

        let coverage = matched_intensity / total_intensity;
        let mean_quality = quality_sum / matched as f64;
        let score = coverage * mean_quality;

        if score < 0.25 {
            continue;
        }

        evidence.sort_by(|a, b| {
            b.matched.cmp(&a.matched).then(
                b.intensity
                    .partial_cmp(&a.intensity)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        evidence.truncate(8);

        candidates.push(Candidate {
            id: isotope.id.clone(),
            name: isotope.name.clone(),
            half_life: isotope.half_life.clone(),
            category: isotope.category.clone(),
            confidence: confidence(matched, coverage, score, &evidence).to_string(),
            score,
            coverage,
            matched_lines: matched,
            total_lines: total,
            evidence,
        });
    }

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.matched_lines.cmp(&a.matched_lines))
    });
    candidates.truncate(6);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(peaks: &[(f64, f64)]) -> Spectrum {
        let mut counts = vec![40u32; 1024];
        for (energy, amplitude) in peaks {
            let channel = (energy / 3.0) as isize;
            for offset in -20..=20 {
                let index = channel + offset;
                if index < 0 || index >= counts.len() as isize {
                    continue;
                }
                let sigma = 6.0;
                let value = amplitude * (-0.5 * (offset as f64 / sigma).powi(2)).exp();
                counts[index as usize] += value as u32;
            }
        }
        Spectrum {
            duration_s: 600,
            a0: 0.0,
            a1: 3.0,
            a2: 0.0,
            counts,
        }
    }

    #[test]
    fn library_is_loaded() {
        let lib = library();
        assert!(lib.isotopes.len() >= 30);
        let cs = lib.isotopes.iter().find(|i| i.id == "Cs-137").unwrap();
        assert!((cs.lines[0].energy_kev - 661.657).abs() < 0.01);
    }

    #[test]
    fn identifies_cs137_and_k40() {
        let spectrum = synthetic(&[(661.7, 3000.0), (1460.8, 700.0)]);
        let analysis = analyze(&spectrum);
        assert!(!analysis.peaks.is_empty());
        let ids: Vec<&str> = analysis.candidates.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"Cs-137"), "candidates: {ids:?}");
        assert!(ids.contains(&"K-40"), "candidates: {ids:?}");

        let cs = analysis.candidates.iter().find(|c| c.id == "Cs-137").unwrap();
        assert_eq!(cs.confidence, "medium");
        assert!(analysis.candidates.iter().all(|c| c.confidence != "high"));
    }

    #[test]
    fn two_strong_lines_reach_high_confidence() {
        let spectrum = synthetic(&[(1173.2, 2600.0), (1332.5, 2500.0)]);
        let analysis = analyze(&spectrum);
        let co = analysis
            .candidates
            .iter()
            .find(|c| c.id == "Co-60")
            .expect("Co-60 should be detected");
        assert_eq!(co.confidence, "high");
    }

    #[test]
    fn ignores_low_energy_only_isotopes() {
        // The RC-110 has a large low-energy artefact around 78 keV; it must not
        // be reported as Xe-133 (81 keV is its only line).
        let spectrum = synthetic(&[(78.0, 5000.0)]);
        let analysis = analyze(&spectrum);
        assert!(
            !analysis.candidates.iter().any(|c| c.id == "Xe-133"),
            "candidates: {:?}",
            analysis.candidates.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn real_device_fixture_finds_k40() {
        let spectrum: Spectrum =
            serde_json::from_str(include_str!("../tests/fixtures/rc110-spectrum.json")).unwrap();
        let analysis = analyze(&spectrum);
        assert!(!analysis.peaks.is_empty());
        assert!(
            analysis.candidates.iter().any(|c| c.id == "K-40"),
            "candidates: {:?}",
            analysis
                .candidates
                .iter()
                .map(|c| (&c.id, c.score))
                .collect::<Vec<_>>()
        );
    }
}
