//! Bluetooth LE transport (Linux / BlueZ).
//!
//! The device exposes a vendor GATT service with two characteristics: one for
//! writing requests and one that notifies replies. Writes are chunked to fit
//! the default ATT payload size.

use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use bluer::gatt::remote::Characteristic;
use bluer::{Adapter, AdapterEvent, Address, Session, Uuid};
use futures::{Stream, StreamExt};

use crate::error::{Error, Result};
use crate::protocol::{BLE_CHUNK, BLE_NOTIFY_UUID, BLE_WRITE_UUID};
use crate::transport::Transport;

const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

pub struct BleTransport {
    write: Characteristic,
    notify: Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>,
    pending: Vec<u8>,
    label: String,
}

impl BleTransport {
    /// Connect to a device by address, or scan for one whose name contains
    /// `name_hint` (case-insensitive).
    pub async fn open(address: Option<&str>, name_hint: &str, scan_secs: u64) -> Result<Self> {
        let session = Session::new().await.map_err(ble_err("session"))?;
        let adapter = session.default_adapter().await.map_err(ble_err("adapter"))?;
        adapter.set_powered(true).await.map_err(ble_err("power on"))?;

        let addr = match address {
            Some(text) => text
                .parse::<Address>()
                .map_err(|_| Error::Bluetooth(format!("bad address {text}")))?,
            None => discover(&adapter, name_hint, scan_secs).await?,
        };

        let device = adapter.device(addr).map_err(ble_err("device"))?;
        let label = format!("ble {addr}");
        let _ = device.set_trusted(true).await;
        device.connect().await.map_err(ble_err("connect"))?;

        let services = device.services().await.map_err(ble_err("services"))?;
        let write_uuid = Uuid::parse_str(BLE_WRITE_UUID).expect("valid write uuid");
        let notify_uuid = Uuid::parse_str(BLE_NOTIFY_UUID).expect("valid notify uuid");

        let mut write = None;
        let mut notify_char = None;
        for service in services {
            for characteristic in service
                .characteristics()
                .await
                .map_err(ble_err("characteristics"))?
            {
                match characteristic.uuid().await {
                    Ok(uuid) if uuid == write_uuid => write = Some(characteristic),
                    Ok(uuid) if uuid == notify_uuid => notify_char = Some(characteristic),
                    _ => {}
                }
            }
        }

        let write = write.ok_or_else(|| Error::Bluetooth("write characteristic not found".into()))?;
        let notify_char =
            notify_char.ok_or_else(|| Error::Bluetooth("notify characteristic not found".into()))?;
        let stream = notify_char.notify().await.map_err(ble_err("subscribe"))?;

        Ok(Self {
            write,
            notify: Box::pin(stream),
            pending: Vec::new(),
            label,
        })
    }
}

async fn discover(adapter: &Adapter, name_hint: &str, scan_secs: u64) -> Result<Address> {
    let hint = name_hint.to_lowercase();
    let mut stream = adapter.discover_devices().await.map_err(ble_err("scan"))?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(scan_secs);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(Error::DeviceNotFound);
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(AdapterEvent::DeviceAdded(addr))) => {
                let Ok(device) = adapter.device(addr) else {
                    continue;
                };
                let name = device.name().await.ok().flatten().unwrap_or_default();
                let alias = device.alias().await.ok().unwrap_or_default();
                let haystack = format!("{name} {alias}").to_lowercase();
                if haystack.contains(&hint) {
                    return Ok(addr);
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => return Err(Error::DeviceNotFound),
            Err(_) => return Err(Error::DeviceNotFound),
        }
    }
}

fn ble_err(context: &'static str) -> impl Fn(bluer::Error) -> Error {
    move |err| Error::Bluetooth(format!("{context}: {err}"))
}

#[async_trait]
impl Transport for BleTransport {
    async fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        self.pending.clear();

        for chunk in frame.chunks(BLE_CHUNK) {
            self.write.write(chunk).await.map_err(ble_err("write"))?;
            tokio::time::sleep(Duration::from_millis(2)).await;
        }

        let deadline = tokio::time::Instant::now() + WRITE_TIMEOUT + REPLY_TIMEOUT;
        loop {
            if self.pending.len() >= 4 {
                let total = 4 + u32::from_le_bytes([
                    self.pending[0],
                    self.pending[1],
                    self.pending[2],
                    self.pending[3],
                ]) as usize;
                if self.pending.len() >= total {
                    let body = self.pending[4..total].to_vec();
                    self.pending.drain(..total);
                    return Ok(body);
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            match tokio::time::timeout(remaining, self.notify.next()).await {
                Ok(Some(chunk)) => self.pending.extend_from_slice(&chunk),
                Ok(None) => return Err(Error::Disconnected),
                Err(_) => return Err(Error::Timeout),
            }
        }
    }

    fn label(&self) -> String {
        self.label.clone()
    }
}

/// Scan for nearby BLE devices and return `(address, name)` pairs.
pub async fn scan(seconds: u64) -> Result<Vec<(String, String)>> {
    let session = Session::new().await.map_err(ble_err("session"))?;
    let adapter = session.default_adapter().await.map_err(ble_err("adapter"))?;
    let _ = adapter.set_powered(true).await;

    let mut stream = adapter.discover_devices().await.map_err(ble_err("scan"))?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let mut found: Vec<(String, String)> = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(AdapterEvent::DeviceAdded(address))) => {
                if found.iter().any(|(a, _)| a == &address.to_string()) {
                    continue;
                }
                let label = match adapter.device(address) {
                    Ok(device) => {
                        let alias = device.alias().await.ok();
                        let name = device.name().await.ok().flatten();
                        alias.or(name).unwrap_or_else(|| "(unknown)".into())
                    }
                    Err(_) => "(unknown)".into(),
                };
                found.push((address.to_string(), label));
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }

    found.sort();
    Ok(found)
}
