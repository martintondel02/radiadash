//! USB transport using the device's vendor bulk endpoints.
//!
//! Unlike a serial port, the RadiaCode exposes a custom interface: bulk OUT
//! endpoint `0x01` and bulk IN endpoint `0x81`, with a 32-bit little-endian
//! length prefix in front of every frame.

use std::time::Duration;

use async_trait::async_trait;
use nusb::transfer::{Buffer, Bulk, In, Out};
use nusb::{Device, Endpoint, Interface};

use crate::error::{Error, Result};
use crate::protocol::{USB_EP_IN, USB_EP_OUT, USB_PID, USB_VID};
use crate::transport::Transport;

const IO_TIMEOUT: Duration = Duration::from_millis(3000);

/// Summary of a connected RadiaCode device, for discovery UIs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    pub bus: String,
}

/// List connected RadiaCode devices without opening them.
pub async fn list() -> Result<Vec<UsbDeviceInfo>> {
    let devices = nusb::list_devices()
        .await
        .map_err(|e| Error::Usb(format!("enumerate: {e}")))?;
    Ok(devices
        .filter(|d| d.vendor_id() == USB_VID && d.product_id() == USB_PID)
        .map(|d| UsbDeviceInfo {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            serial: d.serial_number().map(str::to_string),
            bus: d.bus_id().to_string(),
        })
        .collect())
}

pub struct UsbTransport {
    _device: Device,
    _interface: Interface,
    ep_in: Endpoint<Bulk, In>,
    ep_out: Endpoint<Bulk, Out>,
    label: String,
}

impl UsbTransport {
    /// Open the first matching device, optionally filtered by USB serial number.
    pub async fn open(serial: Option<&str>) -> Result<Self> {
        let devices = nusb::list_devices()
            .await
            .map_err(|e| Error::Usb(format!("enumerate: {e}")))?;

        let mut selected = None;
        for info in devices {
            if info.vendor_id() != USB_VID || info.product_id() != USB_PID {
                continue;
            }
            if let Some(want) = serial
                && info.serial_number() != Some(want)
            {
                continue;
            }
            selected = Some(info);
            break;
        }

        let info = selected.ok_or(Error::DeviceNotFound)?;
        let label = format!(
            "usb {:04x}:{:04x}{}",
            info.vendor_id(),
            info.product_id(),
            info.serial_number().map(|s| format!(" {s}")).unwrap_or_default()
        );

        let device = info.open().await.map_err(|e| Error::Usb(format!("open: {e}")))?;
        let interface = device
            .claim_interface(0)
            .await
            .map_err(|e| Error::Usb(format!("claim interface: {e}")))?;
        let ep_in = interface
            .endpoint::<Bulk, In>(USB_EP_IN)
            .map_err(|e| Error::Usb(format!("bulk in endpoint: {e}")))?;
        let ep_out = interface
            .endpoint::<Bulk, Out>(USB_EP_OUT)
            .map_err(|e| Error::Usb(format!("bulk out endpoint: {e}")))?;

        let mut transport = Self {
            _device: device,
            _interface: interface,
            ep_in,
            ep_out,
            label,
        };
        transport.drain();
        Ok(transport)
    }

    /// Discard stale bytes left over from a previous session.
    fn drain(&mut self) {
        for _ in 0..64 {
            let done = self
                .ep_in
                .transfer_blocking(Buffer::new(256), Duration::from_millis(100));
            if done.status.is_err() || done.actual_len == 0 {
                break;
            }
        }
    }

    fn read_chunk(&mut self, out: &mut Vec<u8>, timeout: Duration) -> Result<usize> {
        let done = self.ep_in.transfer_blocking(Buffer::new(256), timeout);
        if let Err(err) = done.status {
            return Err(Error::Usb(format!("bulk read: {err:?}")));
        }
        let n = done.actual_len;
        out.extend_from_slice(&done.buffer.into_vec()[..n]);
        Ok(n)
    }
}

#[async_trait]
impl Transport for UsbTransport {
    async fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        let done = self
            .ep_out
            .transfer_blocking(Buffer::from(frame.to_vec()), IO_TIMEOUT);
        if let Err(err) = done.status {
            return Err(Error::Usb(format!("bulk write: {err:?}")));
        }
        if done.actual_len != frame.len() {
            return Err(Error::Usb(format!(
                "short write: {} of {} bytes",
                done.actual_len,
                frame.len()
            )));
        }

        let mut data = Vec::with_capacity(512);
        let mut spins = 0;
        while data.len() < 4 {
            self.read_chunk(&mut data, IO_TIMEOUT)?;
            spins += 1;
            if spins > 8 {
                return Err(Error::Protocol("no reply length prefix".into()));
            }
        }

        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if len > 1 << 20 {
            return Err(Error::Protocol(format!("unreasonable reply length {len}")));
        }
        while data.len() < 4 + len {
            self.read_chunk(&mut data, IO_TIMEOUT)?;
        }

        Ok(data[4..4 + len].to_vec())
    }

    fn label(&self) -> String {
        self.label.clone()
    }
}
