//! Transport abstraction: USB and BLE both carry the same framed protocol.

use async_trait::async_trait;

use crate::error::Result;

#[async_trait]
pub trait Transport: Send {
    /// Send one complete frame (length prefix included) and return the reply
    /// body (header and length prefix stripped).
    async fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>>;

    /// Human readable description, e.g. `usb 0483:f123 RC-10x-123456`.
    fn label(&self) -> String;

    /// Release the underlying connection.
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
