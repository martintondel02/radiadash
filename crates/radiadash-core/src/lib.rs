//! Core library for talking to RadiaCode radiation detectors.
//!
//! The wire protocol is a small request/response protocol carried over USB
//! (vendor bulk endpoints) or Bluetooth LE (custom GATT characteristics).
//! Frames look like:
//!
//! ```text
//! [u32 payload_len][u16 command][u8 zero][u8 sequence | 0x80][args...]
//! ```
//!
//! The device echoes the four header bytes back, followed by the payload.

#[cfg(all(feature = "ble", target_os = "linux"))]
pub mod ble;
pub mod decode;
pub mod device;
pub mod error;
pub mod isotope;
pub mod protocol;
pub mod spectrum;
pub mod transport;
pub mod types;
pub mod units;
#[cfg(feature = "usb")]
pub mod usb;

pub use device::{OpenOptions, RadiaCode};
pub use error::{Error, Result};
pub use transport::Transport;
pub use types::*;
