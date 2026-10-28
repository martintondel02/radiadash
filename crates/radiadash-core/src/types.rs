//! Data types decoded from the device protocol.

use std::collections::BTreeMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// A real-time measurement record (about one per second while streaming).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealTimeData {
    pub dt: DateTime<Local>,
    pub count_rate: f32,
    /// Percentage error on `count_rate`.
    pub count_rate_err: f32,
    /// Raw dose rate in the device's internal scale (see `units`).
    pub dose_rate: f32,
    /// Percentage error on `dose_rate`.
    pub dose_rate_err: f32,
    pub flags: u16,
    pub real_time_flags: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawData {
    pub dt: DateTime<Local>,
    pub count_rate: f32,
    pub dose_rate: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoseRateDb {
    pub dt: DateTime<Local>,
    pub count: u32,
    pub count_rate: f32,
    pub dose_rate: f32,
    pub dose_rate_err: f32,
    pub flags: u16,
}

/// Slow-changing device state: accumulated dose, temperature, battery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RareData {
    pub dt: DateTime<Local>,
    pub duration: u32,
    /// Raw accumulated-dose word. On firmware 4.x (RC-110) this is actually a
    /// little-endian `f32` in roentgen; `dose_float` exposes that reading.
    pub dose: u32,
    /// The same 32 bits interpreted as `f32`.
    pub dose_float: f32,
    pub temperature_c: f32,
    pub charge_percent: f32,
    pub flags: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub dt: DateTime<Local>,
    pub event: u8,
    pub event_param1: u8,
    pub flags: u16,
}

/// Any record the device can return from its data buffer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    RealTime(RealTimeData),
    Raw(RawData),
    DoseRateDb(DoseRateDb),
    Rare(RareData),
    Event(Event),
}

impl Record {
    pub fn dt(&self) -> DateTime<Local> {
        match self {
            Record::RealTime(r) => r.dt,
            Record::Raw(r) => r.dt,
            Record::DoseRateDb(r) => r.dt,
            Record::Rare(r) => r.dt,
            Record::Event(r) => r.dt,
        }
    }
}

/// One spectrum measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spectrum {
    /// Live time in seconds.
    pub duration_s: u64,
    /// Energy calibration coefficients: `keV = a0 + a1*ch + a2*ch^2`.
    pub a0: f32,
    pub a1: f32,
    pub a2: f32,
    /// Counts per channel (usually 1024 channels).
    pub counts: Vec<u32>,
}

impl Spectrum {
    pub fn total_counts(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    pub fn cps(&self) -> f64 {
        if self.duration_s == 0 {
            0.0
        } else {
            self.total_counts() as f64 / self.duration_s as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirmwareVersion {
    pub major: u16,
    pub minor: u16,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub transport: String,
    pub serial: String,
    pub hw_serial: String,
    pub firmware: FirmwareVersion,
    pub boot: FirmwareVersion,
    pub model: Option<String>,
    pub spectrum_format: u8,
    pub config: BTreeMap<String, String>,
}

/// Values decoded from special-function registers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VsfrValue {
    U32(u32),
    I32(i32),
    F32(f32),
    U16(u16),
    I16(i16),
    Bool(bool),
}

impl VsfrValue {
    pub fn as_f64(self) -> f64 {
        match self {
            VsfrValue::U32(v) => v as f64,
            VsfrValue::I32(v) => v as f64,
            VsfrValue::F32(v) => v as f64,
            VsfrValue::U16(v) => v as f64,
            VsfrValue::I16(v) => v as f64,
            VsfrValue::Bool(v) => u8::from(v) as f64,
        }
    }

    pub fn as_bool(self) -> bool {
        match self {
            VsfrValue::Bool(v) => v,
            VsfrValue::U32(v) => v != 0,
            _ => self.as_f64() != 0.0,
        }
    }
}

/// How the device reports dose values internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawDoseScale {
    /// Value is already µSv (or µSv/h).
    MicroSievert,
    /// Value is µR (or µR/h); 100 µR == 1 µSv.
    MicroRoentgen,
    /// Value is nSv (or nSv/h).
    NanoSievert,
}

/// Unit configuration reported by the device.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitFlags {
    pub dose_in_sievert: bool,
    pub use_nsv_h: bool,
    pub counts_per_minute: bool,
}

impl Default for UnitFlags {
    fn default() -> Self {
        Self {
            dose_in_sievert: true,
            use_nsv_h: false,
            counts_per_minute: false,
        }
    }
}

impl UnitFlags {
    pub fn raw_scale(&self) -> RawDoseScale {
        if self.use_nsv_h {
            RawDoseScale::NanoSievert
        } else {
            RawDoseScale::MicroRoentgen
        }
    }
}
