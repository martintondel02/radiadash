//! Synthetic spectra and records, used for development without hardware and
//! for deterministic UI tests.

use chrono::{DateTime, Duration, Local};
use radiadash_core::types::{DeviceIdentity, FirmwareVersion, RareData, RealTimeData, Record, Spectrum};

/// Small deterministic PRNG (splitmix64) so demos and tests are reproducible.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    /// Approximate normal sample via the sum of uniforms.
    pub fn normal(&mut self) -> f64 {
        let sum: f64 = (0..12).map(|_| self.uniform()).sum();
        sum - 6.0
    }
}

pub struct PeakSpec {
    pub energy_kev: f64,
    pub amplitude: f64,
    pub fwhm_kev: f64,
}

/// A plausible CsI(Tl) spectrum: NORM background plus a few common sources.
pub fn demo_spectrum(duration_s: u64, seed: u64) -> Spectrum {
    let a0 = 0.0f32;
    let a1 = 3.0f32; // 3 keV per channel, 1024 channels -> 3072 keV
    let a2 = 0.0f32;
    let channels = 1024usize;
    let mut rng = Rng::new(seed);

    let peaks = [
        PeakSpec {
            energy_kev: 59.5,
            amplitude: 140.0,
            fwhm_kev: 12.0,
        }, // Am-241
        PeakSpec {
            energy_kev: 351.9,
            amplitude: 60.0,
            fwhm_kev: 20.0,
        }, // Pb-214
        PeakSpec {
            energy_kev: 609.3,
            amplitude: 70.0,
            fwhm_kev: 27.0,
        }, // Bi-214
        PeakSpec {
            energy_kev: 661.7,
            amplitude: 900.0,
            fwhm_kev: 55.0,
        }, // Cs-137
        PeakSpec {
            energy_kev: 1120.3,
            amplitude: 45.0,
            fwhm_kev: 42.0,
        }, // Bi-214
        PeakSpec {
            energy_kev: 1460.8,
            amplitude: 260.0,
            fwhm_kev: 60.0,
        }, // K-40
        PeakSpec {
            energy_kev: 1764.5,
            amplitude: 40.0,
            fwhm_kev: 70.0,
        }, // Bi-214
    ];

    let mut counts = Vec::with_capacity(channels);
    for channel in 0..channels {
        let energy = a0 as f64 + a1 as f64 * channel as f64 + a2 as f64 * (channel * channel) as f64;
        // Background: a broad Compton continuum falling off with energy.
        let mut expected = 40.0 * (-energy / 900.0).exp() + 2.0;
        for peak in &peaks {
            let sigma = peak.fwhm_kev / 2.355;
            let x = (energy - peak.energy_kev) / sigma;
            expected += peak.amplitude * (-0.5 * x * x).exp();
        }
        expected *= duration_s as f64 / 60.0;

        let noisy = expected + expected.sqrt() * rng.normal();
        counts.push(noisy.max(0.0).round() as u32);
    }

    Spectrum {
        duration_s,
        a0,
        a1,
        a2,
        counts,
    }
}

/// One measurement per second with realistic jitter.
pub fn demo_records(
    start: DateTime<Local>,
    seconds: u64,
    count_rate: f64,
    dose_rate_usv_h: f64,
    seed: u64,
) -> Vec<Record> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(seconds as usize);
    for i in 0..seconds {
        let dt = start + Duration::seconds(i as i64);
        let cps = count_rate + count_rate.sqrt() * rng.normal();
        out.push(Record::RealTime(RealTimeData {
            dt,
            count_rate: cps.max(0.0) as f32,
            count_rate_err: (100.0 / cps.max(1.0)) as f32,
            // Raw scale is µR/h, i.e. 100x the µSv/h value.
            dose_rate: (dose_rate_usv_h * 100.0) as f32,
            dose_rate_err: 3.0,
            flags: 0,
            real_time_flags: 0,
        }));

        if i % 30 == 0 {
            let dose_r = (i as f64 * dose_rate_usv_h) / 1e4;
            out.push(Record::Rare(RareData {
                dt,
                duration: i as u32,
                dose: (dose_r as f32).to_bits(),
                dose_float: dose_r as f32,
                temperature_c: 24.5,
                charge_percent: 87.0,
                flags: 0,
            }));
        }
    }
    out
}

pub fn demo_identity() -> DeviceIdentity {
    let mut config = std::collections::BTreeMap::new();
    config.insert("DeviceName".into(), "RC-110 (simulated)".into());
    config.insert("SpecFormatVersion".into(), "1".into());
    DeviceIdentity {
        transport: "simulator".into(),
        serial: "RC-110-SIM-0001".into(),
        hw_serial: "00000000-00000000".into(),
        firmware: FirmwareVersion {
            major: 4,
            minor: 12,
            date: "2026-01-01".into(),
        },
        boot: FirmwareVersion {
            major: 1,
            minor: 2,
            date: "2025-11-01".into(),
        },
        model: Some("RC-110".into()),
        spectrum_format: 1,
        config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_spectrum_has_a_cs137_peak() {
        let s = demo_spectrum(120, 7);
        assert_eq!(s.counts.len(), 1024);
        let ch = (661.7_f64 / 3.0).round() as usize;
        let peak = s.counts[ch - 2..ch + 3].iter().copied().max().unwrap();
        let side = s.counts[ch + 40];
        assert!(peak > side * 5, "peak {peak} vs side {side}");
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        assert_eq!(a.next_u64(), b.next_u64());
    }
}
