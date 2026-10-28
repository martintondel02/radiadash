//! Unit conversions.
//!
//! The device stores dose values in an internal scale: raw registers are in
//! micro-roentgen (µR) and micro-roentgen per hour. The canonical unit used
//! inside Radiadash is the micro-sievert (µSv, µSv/h); display units are
//! applied only at the UI boundary.
//!
//! Conversion assumes the conventional 1 Sv = 100 rem equivalence and
//! 1 R ≈ 0.01 Sv (photon exposure, air). These are approximations and are
//! documented in the UI wherever they are used.

use serde::{Deserialize, Serialize};

/// Micro-roentgen per micro-sievert.
pub const UR_PER_USV: f64 = 100.0;

/// Roentgen -> µSv. 1 R is treated as 0.01 Sv (photon exposure in air).
pub fn roentgen_to_usv(roentgen: f64) -> f64 {
    roentgen * 1e4
}

pub fn usv_to_roentgen(usv: f64) -> f64 {
    usv / 1e4
}

/// Raw device value -> µSv (dose) or µSv/h (dose rate).
pub fn raw_to_usv(raw: f64, scale: crate::types::RawDoseScale) -> f64 {
    use crate::types::RawDoseScale::*;
    match scale {
        MicroSievert => raw,
        MicroRoentgen => raw / UR_PER_USV,
        NanoSievert => raw / 1000.0,
    }
}

pub fn usv_to_raw(usv: f64, scale: crate::types::RawDoseScale) -> f64 {
    use crate::types::RawDoseScale::*;
    match scale {
        MicroSievert => usv,
        MicroRoentgen => usv * UR_PER_USV,
        NanoSievert => usv * 1000.0,
    }
}

/// Display unit for dose rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoseRateUnit {
    SvPerHour,
    MSvPerHour,
    USvPerHour,
    NSvPerHour,
    RemPerHour,
    MRemPerHour,
    URPerHour,
    MRPerHour,
    RPerHour,
}

impl DoseRateUnit {
    /// Convert a canonical µSv/h value into this display unit.
    pub fn from_usv_h(self, usv_h: f64) -> f64 {
        match self {
            DoseRateUnit::SvPerHour => usv_h * 1e-6,
            DoseRateUnit::MSvPerHour => usv_h * 1e-3,
            DoseRateUnit::USvPerHour => usv_h,
            DoseRateUnit::NSvPerHour => usv_h * 1e3,
            DoseRateUnit::RemPerHour => usv_h * 1e-4,
            DoseRateUnit::MRemPerHour => usv_h * 1e-1,
            DoseRateUnit::URPerHour => usv_h * 1e2,
            DoseRateUnit::MRPerHour => usv_h * 1e-1,
            DoseRateUnit::RPerHour => usv_h * 1e-4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DoseRateUnit::SvPerHour => "Sv/h",
            DoseRateUnit::MSvPerHour => "mSv/h",
            DoseRateUnit::USvPerHour => "µSv/h",
            DoseRateUnit::NSvPerHour => "nSv/h",
            DoseRateUnit::RemPerHour => "rem/h",
            DoseRateUnit::MRemPerHour => "mrem/h",
            DoseRateUnit::URPerHour => "µR/h",
            DoseRateUnit::MRPerHour => "mR/h",
            DoseRateUnit::RPerHour => "R/h",
        }
    }
}

/// Display unit for accumulated dose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoseUnit {
    Sv,
    MSv,
    USv,
    Rem,
    MRem,
    Gy,
    MGy,
    Rad,
    MR,
    UR,
}

impl DoseUnit {
    pub fn from_usv(self, usv: f64) -> f64 {
        match self {
            DoseUnit::Sv => usv * 1e-6,
            DoseUnit::MSv => usv * 1e-3,
            DoseUnit::USv => usv,
            DoseUnit::Rem => usv * 1e-4,
            DoseUnit::MRem => usv * 1e-1,
            DoseUnit::Gy => usv * 1e-6,
            DoseUnit::MGy => usv * 1e-3,
            DoseUnit::Rad => usv * 1e-4,
            DoseUnit::MR => usv * 1e-4,
            DoseUnit::UR => usv * 1e2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DoseUnit::Sv => "Sv",
            DoseUnit::MSv => "mSv",
            DoseUnit::USv => "µSv",
            DoseUnit::Rem => "rem",
            DoseUnit::MRem => "mrem",
            DoseUnit::Gy => "Gy",
            DoseUnit::MGy => "mGy",
            DoseUnit::Rad => "rad",
            DoseUnit::MR => "mR",
            DoseUnit::UR => "µR",
        }
    }
}

/// Count-rate display unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountRateUnit {
    Cps,
    Cpm,
}

impl CountRateUnit {
    pub fn from_cps(self, cps: f64) -> f64 {
        match self {
            CountRateUnit::Cps => cps,
            CountRateUnit::Cpm => cps * 60.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CountRateUnit::Cps => "cps",
            CountRateUnit::Cpm => "cpm",
        }
    }
}

/// Energy display unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnergyUnit {
    KeV,
    MeV,
}

impl EnergyUnit {
    pub fn from_kev(self, kev: f64) -> f64 {
        match self {
            EnergyUnit::KeV => kev,
            EnergyUnit::MeV => kev / 1000.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EnergyUnit::KeV => "keV",
            EnergyUnit::MeV => "MeV",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawDoseScale;

    #[test]
    fn roentgen_is_one_hundredth_of_sievert() {
        assert!((raw_to_usv(100.0, RawDoseScale::MicroRoentgen) - 1.0).abs() < 1e-9);
        assert!((raw_to_usv(58.0, RawDoseScale::MicroRoentgen) - 0.58).abs() < 1e-9);
        assert!((usv_to_raw(0.58, RawDoseScale::MicroRoentgen) - 58.0).abs() < 1e-9);
    }

    #[test]
    fn dose_rate_units_round_trip() {
        for unit in [
            DoseRateUnit::SvPerHour,
            DoseRateUnit::USvPerHour,
            DoseRateUnit::RemPerHour,
            DoseRateUnit::URPerHour,
            DoseRateUnit::RPerHour,
        ] {
            let v = unit.from_usv_h(0.58);
            assert!(v.is_finite());
        }
        assert!((DoseRateUnit::URPerHour.from_usv_h(0.58) - 58.0).abs() < 1e-9);
        assert!((DoseRateUnit::MRemPerHour.from_usv_h(1.0) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn roentgen_conversions() {
        assert!((roentgen_to_usv(0.00472) - 47.2).abs() < 1e-9);
        assert!((usv_to_roentgen(47.2) - 0.00472).abs() < 1e-12);
    }

    #[test]
    fn count_rate_units() {
        assert!((CountRateUnit::Cpm.from_cps(1.5) - 90.0).abs() < 1e-9);
    }
}
