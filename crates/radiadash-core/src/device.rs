//! High-level device session: initialization, polling and configuration.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local};

use crate::decode;
use crate::error::{Error, Result};
use crate::protocol::{self, Cursor, cmd, vs, vsfr};
use crate::transport::Transport;
use crate::types::*;

const FIRMWARE_MIN_MAJOR: u16 = 4;
const FIRMWARE_MIN_MINOR: u16 = 8;

/// Time offset applied to buffered records; the device's clock lags the host
/// slightly after the exchange/time sync handshake.
const BASE_TIME_OFFSET_SECS: i64 = 128;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// Continue even if the firmware is older than 4.8.
    pub allow_old_firmware: bool,
}

pub struct RadiaCode {
    transport: Box<dyn Transport>,
    seq: u8,
    base_time: DateTime<Local>,
    pub spectrum_format: u8,
    pub firmware: FirmwareVersion,
    pub boot: FirmwareVersion,
    pub config: BTreeMap<String, String>,
    pub fw_supported: bool,
    identity: Option<DeviceIdentity>,
}

impl RadiaCode {
    pub async fn open(transport: Box<dyn Transport>) -> Result<Self> {
        Self::open_with(transport, OpenOptions::default()).await
    }

    pub async fn open_with(transport: Box<dyn Transport>, options: OpenOptions) -> Result<Self> {
        let mut device = Self {
            transport,
            seq: 0,
            base_time: Local::now(),
            spectrum_format: 0,
            firmware: FirmwareVersion {
                major: 0,
                minor: 0,
                date: String::new(),
            },
            boot: FirmwareVersion {
                major: 0,
                minor: 0,
                date: String::new(),
            },
            config: BTreeMap::new(),
            fw_supported: false,
            identity: None,
        };

        // Handshake, then align the device clock with the host.
        device
            .execute(cmd::SET_EXCHANGE, &[0x01, 0xFF, 0x12, 0xFF])
            .await?;
        device.set_local_time(Local::now()).await?;
        device
            .write_register(vsfr::DEVICE_TIME, Some(&0u32.to_le_bytes()))
            .await?;

        device.base_time = Local::now() + Duration::seconds(BASE_TIME_OFFSET_SECS);

        let (boot, firmware) = device.fw_version().await?;
        device.fw_supported = firmware.major > FIRMWARE_MIN_MAJOR
            || (firmware.major == FIRMWARE_MIN_MAJOR && firmware.minor >= FIRMWARE_MIN_MINOR);

        if !device.fw_supported && !options.allow_old_firmware {
            return Err(Error::FirmwareUnsupported {
                major: firmware.major,
                minor: firmware.minor,
            });
        }

        let configuration = device.configuration().await?;
        device.config = decode::parse_configuration(&configuration);
        device.spectrum_format = device
            .config
            .get("SpecFormatVersion")
            .and_then(|v| v.trim().parse::<u8>().ok())
            .unwrap_or(0);
        device.firmware = firmware;
        device.boot = boot;

        Ok(device)
    }

    pub fn transport_label(&self) -> String {
        self.transport.label()
    }

    pub fn base_time(&self) -> DateTime<Local> {
        self.base_time
    }

    /// Read identity fields from the device (serial, hardware serial, model).
    pub async fn identify(&mut self) -> Result<DeviceIdentity> {
        let serial = self.serial_number().await?;
        let hw_serial = self.hw_serial_number().await?;
        let model = ["DeviceName", "Model", "DeviceModel", "DeviceType"]
            .iter()
            .find_map(|key| self.config.get(*key).cloned());

        let identity = DeviceIdentity {
            transport: self.transport_label(),
            serial,
            hw_serial,
            firmware: self.firmware.clone(),
            boot: self.boot.clone(),
            model,
            spectrum_format: self.spectrum_format,
            config: self.config.clone(),
        };
        self.identity = Some(identity.clone());
        Ok(identity)
    }

    pub fn identity(&self) -> Option<&DeviceIdentity> {
        self.identity.as_ref()
    }

    async fn execute(&mut self, command: u16, args: &[u8]) -> Result<Vec<u8>> {
        let seq = self.seq;
        self.seq = (self.seq + 1) % 32;

        let frame = protocol::frame(command, seq, args);
        let header = protocol::header_of(&frame);
        let reply = self.transport.exchange(&frame).await?;
        Ok(protocol::check_reply(&reply, header)?.to_vec())
    }

    async fn read_virtual_string(&mut self, id: u32) -> Result<Vec<u8>> {
        let reply = self.execute(cmd::RD_VIRT_STRING, &id.to_le_bytes()).await?;
        let mut cursor = Cursor::new(&reply);
        let retcode = cursor.u32()?;
        let length = cursor.u32()? as usize;
        if retcode != 1 {
            return Err(Error::Protocol(format!(
                "read 0x{id:04x} returned code {retcode}"
            )));
        }

        let mut data = cursor.rest().to_vec();
        // Some firmware versions append a stray NUL byte.
        if data.len() == length + 1 && data.last() == Some(&0) {
            data.pop();
        }
        if data.len() != length {
            return Err(Error::Protocol(format!(
                "read 0x{id:04x}: expected {length} bytes, got {}",
                data.len()
            )));
        }
        Ok(data)
    }

    /// Write a virtual register. `data` is the raw little-endian payload.
    pub async fn write_register(&mut self, id: u32, data: Option<&[u8]>) -> Result<()> {
        let mut args = id.to_le_bytes().to_vec();
        if let Some(data) = data {
            args.extend_from_slice(data);
        }
        let reply = self.execute(cmd::WR_VIRT_SFR, &args).await?;
        let mut cursor = Cursor::new(&reply);
        let retcode = cursor.u32()?;
        if retcode != 1 {
            return Err(Error::Protocol(format!(
                "write 0x{id:04x} returned code {retcode}"
            )));
        }
        Ok(())
    }

    /// Batch-read registers; returns the raw 32-bit value of each one.
    pub async fn batch_read(&mut self, ids: &[u32]) -> Result<Vec<u32>> {
        let mut args = (ids.len() as u32).to_le_bytes().to_vec();
        for id in ids {
            args.extend_from_slice(&id.to_le_bytes());
        }

        let reply = self.execute(cmd::RD_VIRT_SFR_BATCH, &args).await?;
        let mut cursor = Cursor::new(&reply);
        let valid = cursor.u32()?;
        let expected = if ids.is_empty() {
            0
        } else {
            (1u32 << ids.len()) - 1
        };
        if valid != expected {
            return Err(Error::Protocol(format!(
                "batch read validity {valid:08b} != {expected:08b}"
            )));
        }

        let mut values = Vec::with_capacity(ids.len());
        for _ in ids {
            values.push(cursor.u32()?);
        }
        Ok(values)
    }

    /// Read one register, returning `None` when the device reports it as invalid.
    pub async fn read_register_opt(&mut self, id: u32) -> Result<Option<u32>> {
        let mut args = 1u32.to_le_bytes().to_vec();
        args.extend_from_slice(&id.to_le_bytes());
        let reply = self.execute(cmd::RD_VIRT_SFR_BATCH, &args).await?;
        let mut cursor = Cursor::new(&reply);
        let valid = cursor.u32()?;
        if valid & 1 == 0 {
            return Ok(None);
        }
        Ok(Some(cursor.u32()?))
    }

    pub async fn firmware_signature(&mut self) -> Result<String> {
        let reply = self.execute(cmd::FW_SIGNATURE, &[]).await?;
        let mut cursor = Cursor::new(&reply);
        let signature = cursor.u32()?;
        let filename = cursor.string()?;
        let id = cursor.string()?;
        Ok(format!("{signature:08X} {filename} {id}"))
    }

    pub async fn fw_version(&mut self) -> Result<(FirmwareVersion, FirmwareVersion)> {
        let reply = self.execute(cmd::GET_VERSION, &[]).await?;
        decode::decode_fw_version(&reply)
    }

    pub async fn serial_number(&mut self) -> Result<String> {
        let data = self.read_virtual_string(vs::SERIAL_NUMBER).await?;
        Ok(String::from_utf8_lossy(&data)
            .trim_end_matches('\0')
            .trim()
            .to_string())
    }

    pub async fn hw_serial_number(&mut self) -> Result<String> {
        let reply = self.execute(cmd::GET_SERIAL, &[]).await?;
        decode::decode_hw_serial(&reply)
    }

    pub async fn configuration(&mut self) -> Result<String> {
        let data = self.read_virtual_string(vs::CONFIGURATION).await?;
        Ok(decode::decode_configuration(&data))
    }

    pub async fn text_message(&mut self) -> Result<String> {
        let data = self.read_virtual_string(vs::TEXT_MESSAGE).await?;
        Ok(String::from_utf8_lossy(&data).trim_end_matches('\0').to_string())
    }

    pub async fn status_flags(&mut self) -> Result<u32> {
        let reply = self.execute(cmd::GET_STATUS, &[]).await?;
        Cursor::new(&reply).u32()
    }

    pub async fn set_local_time(&mut self, time: DateTime<Local>) -> Result<()> {
        use chrono::{Datelike, Timelike};
        let payload = [
            time.day() as u8,
            time.month() as u8,
            (time.year() - 2000) as u8,
            0,
            time.second() as u8,
            time.minute() as u8,
            time.hour() as u8,
            0,
        ];
        self.execute(cmd::SET_TIME, &payload).await?;
        Ok(())
    }

    pub async fn data_buf(&mut self) -> Result<Vec<Record>> {
        let data = self.read_virtual_string(vs::DATA_BUF).await?;
        Ok(decode::decode_data_buf(&data, self.base_time))
    }

    pub async fn spectrum(&mut self) -> Result<Spectrum> {
        let data = self.read_virtual_string(vs::SPECTRUM).await?;
        decode::decode_spectrum(&data, self.spectrum_format)
    }

    pub async fn spectrum_accumulated(&mut self) -> Result<Spectrum> {
        let data = self.read_virtual_string(vs::SPEC_ACCUM).await?;
        decode::decode_spectrum(&data, self.spectrum_format)
    }

    pub async fn energy_calibration(&mut self) -> Result<[f32; 3]> {
        let data = self.read_virtual_string(vs::ENERGY_CALIB).await?;
        let mut cursor = Cursor::new(&data);
        Ok([cursor.f32()?, cursor.f32()?, cursor.f32()?])
    }

    pub async fn set_energy_calibration(&mut self, coefficients: [f32; 3]) -> Result<()> {
        for (id, value) in [
            (vsfr::CHN_TO_KEV_A0, coefficients[0]),
            (vsfr::CHN_TO_KEV_A1, coefficients[1]),
            (vsfr::CHN_TO_KEV_A2, coefficients[2]),
        ] {
            self.write_register(id, Some(&value.to_le_bytes())).await?;
        }
        Ok(())
    }

    pub async fn read_units(&mut self) -> Result<UnitFlags> {
        let values = self
            .batch_read(&[vsfr::DS_UNITS, vsfr::USE_NSV_H, vsfr::CR_UNITS])
            .await?;
        Ok(UnitFlags {
            dose_in_sievert: high_byte(values[0]) != 0,
            use_nsv_h: high_byte(values[1]) != 0,
            counts_per_minute: high_byte(values[2]) != 0,
        })
    }

    /// Convenience snapshot: CPS, µR/h, µR accumulated and temperature.
    ///
    /// Some firmware (including the RC-110 we tested) exposes only a subset of
    /// these registers, so missing values are reported as `None`.
    pub async fn live_registers(&mut self) -> Result<LiveRegisters> {
        Ok(LiveRegisters {
            cps: self.read_register_opt(vsfr::CPS).await?,
            dose_rate_ur_h: self.read_register_opt(vsfr::DR_UR_H).await?,
            dose_ur: self.read_register_opt(vsfr::DS_UR).await?,
            temperature_c: self.read_register_opt(vsfr::TEMP_DEGC).await?.map(f32::from_bits),
        })
    }

    pub async fn dose_reset(&mut self) -> Result<()> {
        self.write_register(vsfr::DOSE_RESET, None).await
    }

    pub async fn spectrum_reset(&mut self) -> Result<()> {
        let mut args = vs::SPECTRUM.to_le_bytes().to_vec();
        args.extend_from_slice(&0u32.to_le_bytes());
        let reply = self.execute(cmd::WR_VIRT_STRING, &args).await?;
        let retcode = Cursor::new(&reply).u32()?;
        if retcode != 1 {
            return Err(Error::Protocol(format!("spectrum reset returned {retcode}")));
        }
        Ok(())
    }

    pub async fn set_display_brightness(&mut self, level: u8) -> Result<()> {
        if level > 9 {
            return Err(Error::Unsupported(format!("brightness level {level}")));
        }
        self.write_register(vsfr::DISP_BRT, Some(&(level as u32).to_le_bytes()))
            .await
    }

    pub async fn set_display_off_time(&mut self, seconds: u8) -> Result<()> {
        let encoded = match seconds {
            5 => 0u32,
            10 => 1,
            15 => 2,
            30 => 3,
            other => return Err(Error::Unsupported(format!("display timeout {other}s"))),
        };
        self.write_register(vsfr::DISP_OFF_TIME, Some(&encoded.to_le_bytes()))
            .await
    }

    pub async fn set_sound(&mut self, on: bool) -> Result<()> {
        self.write_register(vsfr::SOUND_ON, Some(&u32::from(on).to_le_bytes()))
            .await
    }

    pub async fn set_vibration(&mut self, on: bool) -> Result<()> {
        self.write_register(vsfr::VIBRO_ON, Some(&u32::from(on).to_le_bytes()))
            .await
    }

    pub async fn set_language(&mut self, english: bool) -> Result<()> {
        self.write_register(vsfr::DEVICE_LANG, Some(&u32::from(english).to_le_bytes()))
            .await
    }

    pub async fn set_ble_tx_power(&mut self, level: u8) -> Result<()> {
        self.write_register(vsfr::BLE_TX_PWR, Some(&(level as u32).to_le_bytes()))
            .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveRegisters {
    pub cps: Option<u32>,
    pub dose_rate_ur_h: Option<u32>,
    pub dose_ur: Option<u32>,
    pub temperature_c: Option<f32>,
}

fn high_byte(value: u32) -> u8 {
    ((value >> 24) & 0xFF) as u8
}
