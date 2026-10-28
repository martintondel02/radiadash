use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("device not found")]
    DeviceNotFound,

    #[error("usb: {0}")]
    Usb(String),

    #[error("bluetooth: {0}")]
    Bluetooth(String),

    #[error("protocol: {0}")]
    Protocol(String),

    #[error("unsupported message or register: {0}")]
    Unsupported(String),

    #[error("timed out waiting for the device")]
    Timeout,

    #[error("connection closed")]
    Disconnected,

    #[error("firmware {major}.{minor} is older than 4.8 and may not be supported")]
    FirmwareUnsupported { major: u16, minor: u16 },

    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
