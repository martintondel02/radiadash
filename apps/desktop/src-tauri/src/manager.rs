//! Device session manager: owns the transport, polls the device, writes to
//! SQLite and emits events to the UI.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use radiadash_core::protocol::vsfr;
use radiadash_core::types::{DeviceIdentity, Record, Spectrum, UnitFlags};
use radiadash_core::{ble, usb, RadiaCode, Transport};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};

use crate::store::Db;

pub const DEFAULT_SPECTRUM_INTERVAL_S: u64 = 60;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportChoice {
    Auto,
    Ble { address: Option<String> },
    Usb { serial: Option<String> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectRequest {
    pub transport: TransportChoice,
    #[serde(default = "default_scan_secs")]
    pub scan_secs: u64,
}

fn default_scan_secs() -> u64 {
    8
}

pub enum Cmd {
    Connect(ConnectRequest),
    Disconnect,
    SetSpectrumInterval(u64),
    ResetDose,
}

/// How the device encodes dose values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoseScale {
    /// `RealTimeData.dose_rate` in R/h, `RareData.dose` an f32 in R.
    Roentgen,
    /// Values already in µSv/h and µSv (older RC-10x firmware).
    MicroSievert,
}

impl DoseScale {
    fn dose_rate_usvh(self, raw: f32) -> f64 {
        match self {
            DoseScale::Roentgen => raw as f64 * 1e4,
            DoseScale::MicroSievert => raw as f64,
        }
    }

    fn dose_usv(self, raw_f32: f32) -> f64 {
        match self {
            DoseScale::Roentgen => raw_f32 as f64 * 1e4,
            DoseScale::MicroSievert => raw_f32 as f64,
        }
    }
}

/// RC-110 firmware writes roentgen-based config keys; use them to pick the scale.
pub fn detect_scale(config: &BTreeMap<String, String>) -> DoseScale {
    let roentgen = config.keys().any(|key| {
        key.contains("AlarmMax_R")
            || key.contains("AlarmMin_R")
            || key.starts_with("DR_") && key.ends_with("_R_h")
    });
    if roentgen {
        DoseScale::Roentgen
    } else {
        DoseScale::MicroSievert
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Telemetry {
    pub ts: i64,
    pub cps: f64,
    pub cps_err: f64,
    pub dose_rate_usvh: f64,
    pub dose_rate_err: f64,
    pub dose_usv: Option<f64>,
    pub temp_c: Option<f32>,
    pub charge: Option<f32>,
    pub flags: u16,
    pub session_id: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Snapshot {
    pub status: String,
    pub message: Option<String>,
    pub identity: Option<DeviceIdentity>,
    pub latest: Option<Telemetry>,
    pub session_id: Option<i64>,
    pub started_at: Option<i64>,
    pub spectrum_interval_s: u64,
    pub dose_scale: Option<DoseScale>,
    pub unit_flags: Option<UnitFlags>,
    pub spectrum: Option<SpectrumPayload>,
    pub analysis: Option<radiadash_core::isotope::Analysis>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpectrumPayload {
    pub ts: i64,
    pub duration_s: u64,
    pub a0: f32,
    pub a1: f32,
    pub a2: f32,
    pub counts: Vec<u32>,
}

impl From<&Spectrum> for SpectrumPayload {
    fn from(s: &Spectrum) -> Self {
        Self {
            ts: chrono::Utc::now().timestamp_millis(),
            duration_s: s.duration_s,
            a0: s.a0,
            a1: s.a1,
            a2: s.a2,
            counts: s.counts.clone(),
        }
    }
}

pub struct Shared {
    pub snapshot: RwLock<Snapshot>,
    pub tx: mpsc::Sender<Cmd>,
    pub db_path: std::path::PathBuf,
}

fn emit_connection(app: &AppHandle, snapshot: &Snapshot) {
    let _ = app.emit("connection", snapshot);
}

async fn open_transport(request: &ConnectRequest) -> anyhow::Result<Box<dyn Transport>> {
    match &request.transport {
        TransportChoice::Ble { address } => {
            let transport = ble::BleTransport::open(address.as_deref(), "radia", request.scan_secs).await?;
            Ok(Box::new(transport))
        }
        TransportChoice::Usb { serial } => {
            let transport = usb::UsbTransport::open(serial.as_deref()).await?;
            Ok(Box::new(transport))
        }
        TransportChoice::Auto => match ble::BleTransport::open(None, "radia", request.scan_secs).await {
            Ok(transport) => Ok(Box::new(transport)),
            Err(ble_err) => match usb::UsbTransport::open(None).await {
                Ok(transport) => Ok(Box::new(transport)),
                Err(usb_err) => anyhow::bail!("bluetooth: {ble_err}; usb: {usb_err}"),
            },
        },
    }
}

pub async fn run(app: AppHandle, shared: Arc<Shared>, mut rx: mpsc::Receiver<Cmd>, db: Arc<Mutex<Db>>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Cmd::Connect(request) => run_connection(&app, &shared, &mut rx, &db, request).await,
            Cmd::Disconnect => {}
            Cmd::SetSpectrumInterval(_) => {}
            Cmd::ResetDose => {}
        }
    }
}

async fn run_connection(
    app: &AppHandle,
    shared: &Arc<Shared>,
    rx: &mut mpsc::Receiver<Cmd>,
    db: &Arc<Mutex<Db>>,
    request: ConnectRequest,
) {
    {
        let mut snapshot = shared.snapshot.write().await;
        snapshot.status = "connecting".into();
        snapshot.message = Some("opening transport".into());
        emit_connection(app, &snapshot);
    }

    let opened = tokio::time::timeout(Duration::from_secs(30), async {
        let transport = open_transport(&request).await?;
        let label = transport.label();
        let device = RadiaCode::open(transport).await?;
        anyhow::Ok((device, label))
    })
    .await;

    let (mut device, transport_label) = match opened {
        Ok(Ok(pair)) => pair,
        Ok(Err(err)) => {
            fail(app, shared, format!("{err:#}")).await;
            return;
        }
        Err(_) => {
            fail(app, shared, "connection timed out".into()).await;
            return;
        }
    };

    let identity = match device.identify().await {
        Ok(identity) => identity,
        Err(err) => {
            fail(app, shared, format!("identify failed: {err}")).await;
            return;
        }
    };
    let unit_flags = device.read_units().await.ok();
    let scale = detect_scale(&identity.config);
    let session_id = match with_db(db, |db| db.start_session(&identity.serial, &transport_label)) {
        Ok(id) => id,
        Err(err) => {
            fail(app, shared, format!("database: {err}")).await;
            return;
        }
    };

    {
        let mut snapshot = shared.snapshot.write().await;
        snapshot.status = "connected".into();
        snapshot.message = None;
        snapshot.identity = Some(identity.clone());
        snapshot.session_id = Some(session_id);
        snapshot.started_at = Some(chrono::Utc::now().timestamp_millis());
        snapshot.dose_scale = Some(scale);
        snapshot.unit_flags = unit_flags;
        snapshot.spectrum_interval_s = DEFAULT_SPECTRUM_INTERVAL_S;
        emit_connection(app, &snapshot);
    }

    // Rare records (dose, battery, temperature) arrive infrequently, so seed
    // them from the database and from whatever the device still buffers.
    let mut seed = Seed::default();
    if let Ok(Some(stored)) = with_db(db, |db| db.latest_rare()) {
        seed.dose_usv = Some(stored.dose_usv);
        seed.temp_c = Some(stored.temp_c);
        seed.charge = Some(stored.charge);
    }
    if let Ok(Some(raw)) = device.read_register_opt(vsfr::TEMP_DEGC).await {
        seed.temp_c = Some(f32::from_bits(raw));
    }
    if let Some(backlog) = seed_from_backlog(&mut device, scale).await {
        seed.dose_usv = backlog.dose_usv.or(seed.dose_usv);
        seed.temp_c = backlog.temp_c.or(seed.temp_c);
        seed.charge = backlog.charge.or(seed.charge);
    }

    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut spectrum_interval = DEFAULT_SPECTRUM_INTERVAL_S;
    let mut ticks_until_spectrum = spectrum_interval;
    let mut latest: Option<Telemetry> = None;

    // Drop the backlog accumulated while disconnected.
    let _ = device.data_buf().await;

    loop {
        tokio::select! {
            cmd = rx.recv() => match cmd {
                Some(Cmd::Disconnect) | None => break,
                Some(Cmd::SetSpectrumInterval(secs)) => {
                    spectrum_interval = secs.clamp(3, 600);
                    ticks_until_spectrum = spectrum_interval;
                    shared.snapshot.write().await.spectrum_interval_s = spectrum_interval;
                }
                Some(Cmd::ResetDose) => {
                    if let Err(err) = device.dose_reset().await {
                        let _ = app.emit("device-error", err.to_string());
                    }
                }
                Some(Cmd::Connect(_)) => {}
            },
            _ = ticker.tick() => {
                match device.data_buf().await {
                    Ok(records) => {
                        if let Some(telemetry) = process_records(app, db, session_id, scale, &seed, &records, &mut latest) {
                            shared.snapshot.write().await.latest = Some(telemetry.clone());
                            let _ = app.emit("telemetry", telemetry);
                        }
                    }
                    Err(err) => {
                        fail(app, shared, format!("device read failed: {err}")).await;
                        break;
                    }
                }

                ticks_until_spectrum = ticks_until_spectrum.saturating_sub(1);
                if ticks_until_spectrum == 0 {
                    ticks_until_spectrum = spectrum_interval;
                    match device.spectrum().await {
                        Ok(spectrum) => {
                            let payload = SpectrumPayload::from(&spectrum);
                            if let Err(err) = with_db(db, |db| db.insert_spectrum(Some(session_id), payload.ts, &spectrum)) {
                                let _ = app.emit("device-error", format!("store spectrum: {err}"));
                            }
                            let analysis = radiadash_core::isotope::analyze(&spectrum);
                            {
                                let mut snapshot = shared.snapshot.write().await;
                                snapshot.spectrum = Some(payload.clone());
                                snapshot.analysis = Some(analysis.clone());
                            }
                            let _ = app.emit("spectrum", payload);
                            let _ = app.emit("analysis", analysis);
                        }
                        Err(err) => {
                            let _ = app.emit("device-error", format!("spectrum read failed: {err}"));
                        }
                    }
                }
            }
        }
    }

    let _ = with_db(db, |db| db.end_session(session_id));
    {
        let mut snapshot = shared.snapshot.write().await;
        snapshot.status = "disconnected".into();
        snapshot.message = None;
        snapshot.session_id = None;
        snapshot.identity = None;
        snapshot.latest = None;
        emit_connection(app, &snapshot);
    }
}

/// Convert and persist incoming records; returns the telemetry to publish.
#[derive(Debug, Clone, Copy, Default)]
struct Seed {
    dose_usv: Option<f64>,
    temp_c: Option<f32>,
    charge: Option<f32>,
}

async fn seed_from_backlog(device: &mut RadiaCode, scale: DoseScale) -> Option<Seed> {
    let mut seed = None;
    for _ in 0..3 {
        let Ok(records) = device.data_buf().await else {
            break;
        };
        for record in &records {
            if let Record::Rare(data) = record {
                seed = Some(Seed {
                    dose_usv: Some(scale.dose_usv(data.dose_float)),
                    temp_c: Some(data.temperature_c),
                    charge: Some(data.charge_percent),
                });
            }
        }
        if seed.is_some() {
            break;
        }
    }
    seed
}

fn process_records(
    app: &AppHandle,
    db: &Arc<Mutex<Db>>,
    session_id: i64,
    scale: DoseScale,
    seed: &Seed,
    records: &[Record],
    latest: &mut Option<Telemetry>,
) -> Option<Telemetry> {
    let mut changed = false;

    for record in records {
        match record {
            Record::RealTime(data) => {
                let telemetry = Telemetry {
                    ts: chrono::Utc::now().timestamp_millis(),
                    cps: data.count_rate as f64,
                    cps_err: data.count_rate_err as f64,
                    dose_rate_usvh: scale.dose_rate_usvh(data.dose_rate),
                    dose_rate_err: data.dose_rate_err as f64,
                    dose_usv: latest.as_ref().and_then(|t| t.dose_usv).or(seed.dose_usv),
                    temp_c: latest.as_ref().and_then(|t| t.temp_c).or(seed.temp_c),
                    charge: latest.as_ref().and_then(|t| t.charge).or(seed.charge),
                    flags: data.flags,
                    session_id,
                };
                if let Err(err) = with_db(db, |db| {
                    db.insert_sample(crate::store::Sample {
                        session_id,
                        ts: telemetry.ts,
                        cps: telemetry.cps,
                        cps_err: telemetry.cps_err,
                        dose_rate_usvh: telemetry.dose_rate_usvh,
                        dose_rate_err: telemetry.dose_rate_err,
                        flags: telemetry.flags,
                    })
                }) {
                    let _ = app.emit("device-error", format!("store sample: {err}"));
                }
                *latest = Some(telemetry);
                changed = true;
            }
            Record::Rare(data) => {
                let dose_usv = scale.dose_usv(data.dose_float);
                let telemetry = match latest.take() {
                    Some(mut t) => {
                        t.dose_usv = Some(dose_usv);
                        t.temp_c = Some(data.temperature_c);
                        t.charge = Some(data.charge_percent);
                        t
                    }
                    None => Telemetry {
                        ts: chrono::Utc::now().timestamp_millis(),
                        cps: 0.0,
                        cps_err: 0.0,
                        dose_rate_usvh: 0.0,
                        dose_rate_err: 0.0,
                        dose_usv: Some(dose_usv),
                        temp_c: Some(data.temperature_c),
                        charge: Some(data.charge_percent),
                        flags: data.flags,
                        session_id,
                    },
                };
                if let Err(err) = with_db(db, |db| {
                    db.insert_rare(
                        session_id,
                        telemetry.ts,
                        dose_usv,
                        data.temperature_c,
                        data.charge_percent,
                        data.flags,
                    )
                }) {
                    let _ = app.emit("device-error", format!("store rare: {err}"));
                }
                *latest = Some(telemetry);
                changed = true;
            }
            _ => {}
        }
    }

    if changed {
        latest.clone()
    } else {
        None
    }
}

async fn fail(app: &AppHandle, shared: &Arc<Shared>, message: String) {
    let mut snapshot = shared.snapshot.write().await;
    snapshot.status = "error".into();
    snapshot.message = Some(message);
    snapshot.identity = None;
    snapshot.session_id = None;
    emit_connection(app, &snapshot);
}

fn with_db<T>(db: &Arc<Mutex<Db>>, f: impl FnOnce(&Db) -> anyhow::Result<T>) -> anyhow::Result<T> {
    let guard = db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&guard)
}
