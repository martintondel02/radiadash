//! Spectrum helpers: energy axis and simple peak search.

use serde::{Deserialize, Serialize};

use crate::types::Spectrum;

/// Convert a channel number to energy in keV.
pub fn channel_to_energy(channel: usize, a0: f32, a1: f32, a2: f32) -> f32 {
    let ch = channel as f32;
    a0 + a1 * ch + a2 * ch * ch
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Peak {
    pub channel: usize,
    pub energy_kev: f32,
    pub counts: u32,
    /// Height above the local valley on the lower side.
    pub prominence: f32,
    /// Approximate FWHM in channels.
    pub fwhm_channels: f32,
}

/// Naive peak search: local maxima with a prominence requirement.
///
/// Good enough for a probe / first look; M3 replaces this with a proper
/// background estimate and FWHM-aware fitting.
pub fn find_peaks(spectrum: &Spectrum, min_prominence: f32, max_peaks: usize) -> Vec<Peak> {
    let counts = &spectrum.counts;
    if counts.len() < 7 {
        return Vec::new();
    }

    const WINDOW: usize = 12;
    let mut peaks = Vec::new();

    for i in 2..counts.len() - 2 {
        let c = counts[i] as f32;
        let is_max = c > counts[i - 1] as f32
            && c >= counts[i + 1] as f32
            && c > counts[i - 2] as f32
            && c >= counts[i + 2] as f32;
        if !is_max || c < 5.0 {
            continue;
        }

        let lo = i.saturating_sub(WINDOW);
        let hi = (i + WINDOW).min(counts.len() - 1);
        let left = counts[lo..i].iter().copied().min().unwrap_or(0) as f32;
        let right = counts[i + 1..=hi].iter().copied().min().unwrap_or(0) as f32;
        let valley = left.max(right);
        let prominence = c - valley;

        if prominence < min_prominence {
            continue;
        }

        // Half-maximum width around the peak.
        let half = valley + prominence / 2.0;
        let mut left_edge = i;
        while left_edge > lo && counts[left_edge] as f32 > half {
            left_edge -= 1;
        }
        let mut right_edge = i;
        while right_edge < hi && counts[right_edge] as f32 > half {
            right_edge += 1;
        }

        peaks.push(Peak {
            channel: i,
            energy_kev: channel_to_energy(i, spectrum.a0, spectrum.a1, spectrum.a2),
            counts: counts[i],
            prominence,
            fwhm_channels: (right_edge - left_edge) as f32,
        });
    }

    peaks.sort_by(|a, b| {
        b.prominence
            .partial_cmp(&a.prominence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    peaks.truncate(max_peaks);
    peaks.sort_by_key(|p| p.channel);
    peaks
}

/// Render a compact ASCII spectrum for terminal use.
pub fn ascii_plot(spectrum: &Spectrum, width: usize, height: usize) -> String {
    let counts = &spectrum.counts;
    if counts.is_empty() || width == 0 || height == 0 {
        return String::new();
    }
    let step = counts.len() as f32 / width as f32;
    let mut bins = vec![0f32; width];
    for (i, &c) in counts.iter().enumerate() {
        let bin = ((i as f32) / step) as usize;
        if bin < width {
            bins[bin] += c as f32;
        }
    }
    // Log scale: spectra span several orders of magnitude, a linear plot hides
    // everything except the strongest peak.
    for bin in bins.iter_mut() {
        *bin = (*bin + 1.0).log10();
    }
    let max = bins.iter().copied().fold(0f32, f32::max).max(1.0);

    let mut out = String::new();
    for row in (0..height).rev() {
        let threshold = max * (row as f32 / height as f32);
        for &v in &bins {
            out.push(if v >= threshold { '█' } else { ' ' });
        }
        out.push('\n');
    }
    let axis = format!(
        "0 keV{:>width$}",
        format!(
            "{:.0} keV",
            channel_to_energy(
                spectrum.counts.len().saturating_sub(1),
                spectrum.a0,
                spectrum.a1,
                spectrum.a2
            )
        ),
        width = width.saturating_sub(6)
    );
    out.push_str(&axis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_axis() {
        assert!((channel_to_energy(662, 0.5, 1.0, 0.0) - 662.5).abs() < 1e-4);
    }

    #[test]
    fn finds_a_synthetic_peak() {
        let mut counts = vec![10u32; 256];
        for i in 0..12 {
            counts[100 + i] = 10 + (120.0 * (-((i as f32 - 6.0) / 2.0).powi(2) / 2.0).exp()) as u32;
        }
        let s = Spectrum {
            duration_s: 60,
            a0: 0.0,
            a1: 3.0,
            a2: 0.0,
            counts,
        };
        let peaks = find_peaks(&s, 20.0, 5);
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].channel, 106);
        assert!((peaks[0].energy_kev - 318.0).abs() < 1.0);
    }
}
