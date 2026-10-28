//! Protocol constants, framing and a small cursor for decoding replies.

use crate::error::{Error, Result};

/// STMicroelectronics VID used by RadiaCode devices.
pub const USB_VID: u16 = 0x0483;
/// Product ID used by RadiaCode 10x devices.
pub const USB_PID: u16 = 0xF123;
/// Bulk-IN endpoint (device -> host).
pub const USB_EP_IN: u8 = 0x81;
/// Bulk-OUT endpoint (host -> device).
pub const USB_EP_OUT: u8 = 0x01;

/// GATT characteristic used to write requests.
pub const BLE_WRITE_UUID: &str = "e63215e6-7003-49d8-96b0-b024798fb901";
/// GATT characteristic used to receive notifications.
pub const BLE_NOTIFY_UUID: &str = "e63215e7-7003-49d8-96b0-b024798fb901";

/// Maximum ATT payload we put in a single write (default MTU 23 minus 3).
pub const BLE_CHUNK: usize = 18;

pub mod cmd {
    pub const GET_STATUS: u16 = 0x0005;
    pub const SET_EXCHANGE: u16 = 0x0007;
    pub const GET_VERSION: u16 = 0x000A;
    pub const GET_SERIAL: u16 = 0x000B;
    pub const FW_SIGNATURE: u16 = 0x0101;
    pub const RD_VIRT_SFR: u16 = 0x0824;
    pub const WR_VIRT_SFR: u16 = 0x0825;
    pub const RD_VIRT_STRING: u16 = 0x0826;
    pub const WR_VIRT_STRING: u16 = 0x0827;
    pub const RD_VIRT_SFR_BATCH: u16 = 0x082A;
    pub const SET_TIME: u16 = 0x0A04;
}

pub mod vs {
    pub const CONFIGURATION: u32 = 0x0002;
    pub const FW_DESCRIPTOR: u32 = 0x0003;
    pub const SERIAL_NUMBER: u32 = 0x0008;
    pub const TEXT_MESSAGE: u32 = 0x000F;
    pub const DATA_BUF: u32 = 0x0100;
    pub const SFR_FILE: u32 = 0x0101;
    pub const SPECTRUM: u32 = 0x0200;
    pub const ENERGY_CALIB: u32 = 0x0202;
    pub const SPEC_ACCUM: u32 = 0x0205;
}

pub mod vsfr {
    pub const DEVICE_LANG: u32 = 0x0502;
    pub const DEVICE_ON: u32 = 0x0503;
    pub const DEVICE_TIME: u32 = 0x0504;
    pub const DISP_BRT: u32 = 0x0511;
    pub const DISP_OFF_TIME: u32 = 0x0513;
    pub const DISP_DIR: u32 = 0x0515;
    pub const SOUND_ON: u32 = 0x0522;
    pub const VIBRO_ON: u32 = 0x0531;
    pub const BLE_TX_PWR: u32 = 0x0700;

    pub const DS_UNITS: u32 = 0x8004;
    pub const USE_NSV_H: u32 = 0x800C;
    pub const CR_UNITS: u32 = 0x8013;
    pub const CPS: u32 = 0x8020;
    pub const DR_UR_H: u32 = 0x8021;
    pub const DS_UR: u32 = 0x8022;
    pub const TEMP_DEGC: u32 = 0x8024;
    pub const CHN_TO_KEV_A0: u32 = 0x8010;
    pub const CHN_TO_KEV_A1: u32 = 0x8011;
    pub const CHN_TO_KEV_A2: u32 = 0x8012;

    pub const DR_LEV1_UR_H: u32 = 0x8000;
    pub const DR_LEV2_UR_H: u32 = 0x8001;
    pub const DS_LEV1_UR: u32 = 0x8014;
    pub const DS_LEV2_UR: u32 = 0x8015;
    pub const CR_LEV1_CP10S: u32 = 0x8008;
    pub const CR_LEV2_CP10S: u32 = 0x8009;
    pub const DOSE_RESET: u32 = 0x8007;
}

/// Build a full frame (length prefix included) for the given command and payload.
pub fn frame(cmd: u16, seq: u8, args: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + args.len());
    payload.extend_from_slice(&cmd.to_le_bytes());
    payload.push(0);
    payload.push(0x80 | (seq & 0x1F));
    payload.extend_from_slice(args);

    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Header bytes the device must echo for a given frame.
pub fn header_of(frame: &[u8]) -> [u8; 4] {
    [frame[4], frame[5], frame[6], frame[7]]
}

/// Parse a reply, verifying the echoed header, and return the body.
pub fn check_reply(reply: &[u8], request_header: [u8; 4]) -> Result<&[u8]> {
    if reply.len() < 4 {
        return Err(Error::Protocol(format!(
            "short reply: {} bytes (expected at least 4)",
            reply.len()
        )));
    }
    if reply[..4] != request_header {
        return Err(Error::Protocol(format!(
            "header mismatch: request {:02x?}, reply {:02x?}",
            request_header,
            &reply[..4]
        )));
    }
    Ok(&reply[4..])
}

/// Sequential little-endian reader with bounds checks.
pub struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn rest(&self) -> &'a [u8] {
        &self.data[self.pos.min(self.data.len())..]
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(Error::Protocol(format!(
                "need {} bytes, only {} left",
                n,
                self.remaining()
            )));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }

    /// Read a `u8`-length-prefixed ASCII string.
    pub fn string(&mut self) -> Result<String> {
        let len = self.u8()? as usize;
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).trim_end_matches('\0').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_layout() {
        let f = frame(cmd::RD_VIRT_STRING, 0, &5u32.to_le_bytes());
        assert_eq!(&f[..4], &8u32.to_le_bytes());
        assert_eq!(&f[4..8], &[0x26, 0x08, 0x00, 0x80]);
        assert_eq!(&f[8..], &5u32.to_le_bytes());
    }

    #[test]
    fn sequence_wraps_and_sets_flag() {
        let f = frame(cmd::GET_STATUS, 31, &[]);
        assert_eq!(f[7], 0x80 | 31);
    }

    #[test]
    fn reply_verification() {
        let f = frame(cmd::GET_STATUS, 3, &[]);
        let mut reply = f[4..].to_vec();
        reply.extend_from_slice(&[1, 2, 3]);
        let body = check_reply(&reply, header_of(&f)).unwrap();
        assert_eq!(body, &[1, 2, 3]);
        assert!(check_reply(&reply, [0, 0, 0, 0]).is_err());
    }
}
