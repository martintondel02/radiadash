//! Tauri shell for Radiadash: IPC surface and app bootstrap.

mod manager;
mod store;
mod track;

use std::sync::Arc;

use manager::{Cmd, ConnectRequest, Shared, Snapshot, SpectrumPayload};
use radiadash_core::isotope::Analysis;
use serde::Serialize;
use store::{Db, Series};
use tauri::{Manager, State};
use tokio::sync::{mpsc, RwLock};
use track::TrackPoint;

struct AppState {
    shared: Arc<Shared>,
}

#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub core_version: &'static str,
}

#[derive(Serialize, Default)]
pub struct DeviceList {
    pub usb: Vec<radiadash_core::usb::UsbDeviceInfo>,
    pub ble: Vec<BleDevice>,
}

#[derive(Serialize)]
pub struct BleDevice {
    pub address: String,
    pub name: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Radiadash",
        version: env!("CARGO_PKG_VERSION"),
        core_version: "0.1.0",
    }
}

#[tauri::command]
async fn connect_device(state: State<'_, AppState>, request: ConnectRequest) -> Result<(), String> {
    state
        .shared
        .tx
        .send(Cmd::Connect(request))
        .await
        .map_err(|_| "device manager stopped".to_string())
}

#[tauri::command]
async fn disconnect_device(state: State<'_, AppState>) -> Result<(), String> {
    state
        .shared
        .tx
        .send(Cmd::Disconnect)
        .await
        .map_err(|_| "device manager stopped".to_string())
}

#[tauri::command]
async fn set_spectrum_interval(state: State<'_, AppState>, seconds: u64) -> Result<(), String> {
    state
        .shared
        .tx
        .send(Cmd::SetSpectrumInterval(seconds))
        .await
        .map_err(|_| "device manager stopped".to_string())
}

#[tauri::command]
async fn set_energy_calibration(state: State<'_, AppState>, coefficients: [f32; 3]) -> Result<(), String> {
    let [a0, a1, a2] = coefficients;
    let sane = a0.is_finite()
        && a1.is_finite()
        && a2.is_finite()
        && (-100.0..100.0).contains(&a0)
        && (0.1..20.0).contains(&a1)
        && a2.abs() < 0.01;
    if !sane {
        return Err("refusing to write implausible calibration coefficients".into());
    }
    state
        .shared
        .tx
        .send(Cmd::SetCalibration(coefficients))
        .await
        .map_err(|_| "device manager stopped".to_string())
}

#[tauri::command]
async fn reset_dose(state: State<'_, AppState>) -> Result<(), String> {
    state
        .shared
        .tx
        .send(Cmd::ResetDose)
        .await
        .map_err(|_| "device manager stopped".to_string())
}

#[tauri::command]
async fn get_snapshot(state: State<'_, AppState>) -> Result<Snapshot, String> {
    Ok(state.shared.snapshot.read().await.clone())
}

#[tauri::command]
async fn get_series(state: State<'_, AppState>, range_seconds: i64) -> Result<Series, String> {
    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    db.series(range_seconds.clamp(10, 90 * 86_400) * 1000)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_latest_spectrum(state: State<'_, AppState>) -> Result<Option<SpectrumPayload>, String> {
    let snapshot = state.shared.snapshot.read().await;
    if let Some(spectrum) = &snapshot.spectrum {
        return Ok(Some(spectrum.clone()));
    }
    drop(snapshot);

    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    match db.latest_spectrum().map_err(|e| e.to_string())? {
        Some(stored) => Ok(Some(SpectrumPayload {
            ts: stored.ts,
            duration_s: stored.duration_s,
            a0: stored.a0,
            a1: stored.a1,
            a2: stored.a2,
            counts: stored.counts,
        })),
        None => Ok(None),
    }
}

#[tauri::command]
async fn get_latest_analysis(state: State<'_, AppState>) -> Result<Option<Analysis>, String> {
    {
        let snapshot = state.shared.snapshot.read().await;
        if let Some(analysis) = &snapshot.analysis {
            return Ok(Some(analysis.clone()));
        }
    }

    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    if let Some(stored) = db.latest_spectrum().map_err(|e| e.to_string())? {
        let spectrum = radiadash_core::types::Spectrum {
            duration_s: stored.duration_s,
            a0: stored.a0,
            a1: stored.a1,
            a2: stored.a2,
            counts: stored.counts,
        };
        return Ok(Some(radiadash_core::isotope::analyze(&spectrum)));
    }
    Ok(None)
}

#[tauri::command]
async fn import_track(
    state: State<'_, AppState>,
    text: String,
    format: String,
    source: Option<String>,
) -> Result<usize, String> {
    let source = source.unwrap_or_else(|| "import".into());
    let points = match format.as_str() {
        "gpx" => track::parse_gpx(&text, &source),
        _ => track::parse_rctrk(&text, &source),
    }
    .map_err(|e| format!("parse failed: {e}"))?;

    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    db.insert_track(&points).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_track(state: State<'_, AppState>) -> Result<Vec<TrackPoint>, String> {
    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    db.track().map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_track(state: State<'_, AppState>) -> Result<(), String> {
    let db = Db::open(&state.shared.db_path).map_err(|e| e.to_string())?;
    db.clear_track().map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_devices() -> Result<DeviceList, String> {
    let usb = radiadash_core::usb::list().await.map_err(|e| e.to_string())?;
    let ble = radiadash_core::ble::scan(4)
        .await
        .map(|list| {
            list.into_iter()
                .map(|(address, name)| BleDevice { address, name })
                .collect()
        })
        .unwrap_or_default();
    Ok(DeviceList { usb, ble })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join("radiadash.db");
            let db = Arc::new(std::sync::Mutex::new(Db::open(&db_path)?));
            if let Ok(db) = db.lock() {
                let _ = db.close_stale_sessions();
            }

            let (tx, rx) = mpsc::channel(32);
            let snapshot = Snapshot {
                status: "disconnected".into(),
                spectrum_interval_s: manager::DEFAULT_SPECTRUM_INTERVAL_S,
                ..Default::default()
            };
            let shared = Arc::new(Shared {
                snapshot: RwLock::new(snapshot),
                tx,
                db_path,
            });
            app.manage(AppState {
                shared: shared.clone(),
            });

            let handle = app.handle().clone();
            let auto = shared.tx.clone();
            tauri::async_runtime::spawn(async move {
                manager::run(handle, shared, rx, db).await;
            });
            // Try to connect on launch; the UI can override with an explicit request.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                let _ = auto
                    .send(Cmd::Connect(ConnectRequest {
                        transport: manager::TransportChoice::Auto,
                        scan_secs: 6,
                    }))
                    .await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            connect_device,
            disconnect_device,
            set_spectrum_interval,
            set_energy_calibration,
            reset_dose,
            get_snapshot,
            get_series,
            get_latest_spectrum,
            get_latest_analysis,
            import_track,
            get_track,
            clear_track,
            list_devices
        ])
        .run(tauri::generate_context!())
        .expect("error while running Radiadash");
}
