//! Regression tests against a real RadiaCode 110 capture.

use radiadash_core::spectrum::find_peaks;
use radiadash_core::types::{Record, Spectrum};

#[test]
fn rc110_spectrum_has_expected_shape_and_k40_peak() {
    let spectrum: Spectrum =
        serde_json::from_str(include_str!("fixtures/rc110-spectrum.json")).expect("valid spectrum fixture");

    assert_eq!(spectrum.counts.len(), 1024);
    assert!(spectrum.duration_s > 0);
    assert!(spectrum.total_counts() > 0);
    assert!(spectrum.a1 > 1.0 && spectrum.a1 < 5.0, "keV/channel looks sane");

    let peaks = find_peaks(&spectrum, 15.0, 20);
    assert!(
        peaks.iter().any(|p| (p.energy_kev - 1460.8).abs() < 40.0),
        "K-40 line should be visible, got {:?}",
        peaks.iter().map(|p| p.energy_kev).collect::<Vec<_>>()
    );
}

#[test]
fn rc110_records_decode_with_plausible_values() {
    let mut realtime = 0;
    let mut rare = 0;

    for line in include_str!("fixtures/rc110-records.jsonl").lines() {
        let record: Record = serde_json::from_str(line).expect("valid record line");
        match record {
            Record::RealTime(data) => {
                realtime += 1;
                assert!(data.count_rate >= 0.0 && data.count_rate < 10_000.0);
                // Dose rate is R/h on this firmware; background stays below 1 mR/h.
                assert!(data.dose_rate >= 0.0 && data.dose_rate < 1e-3);
            }
            Record::Rare(data) => {
                rare += 1;
                // The word is a float in roentgen: well below 1 R.
                assert!(data.dose_float > 0.0 && data.dose_float < 1.0);
                assert!(data.temperature_c > -40.0 && data.temperature_c < 80.0);
                assert!((0.0..=100.0).contains(&data.charge_percent));
            }
            _ => {}
        }
    }

    assert!(realtime > 0, "fixture should contain real-time records");
    assert!(rare > 0, "fixture should contain rare records");
}
