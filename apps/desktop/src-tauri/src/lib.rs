//! Tauri shell for Radiadash.
//!
//! M0 only exposes an `app_info` command so the frontend can prove the IPC
//! bridge works. The device session and data pipeline are wired in M2.

use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub core_version: &'static str,
    pub stage: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Radiadash",
        version: env!("CARGO_PKG_VERSION"),
        core_version: "0.1.0",
        stage: "M0 scaffold",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Radiadash");
}
