use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

pub const PROTOCOL_VERSION: u8 = 1;
pub const PROTOCOL_MAGIC: &[u8; 4] = b"LSHR";

// Message Types
pub const MSG_TYPE_PAIR_HANDSHAKE: u8 = 0x01;
pub const MSG_TYPE_PAIR_INFO: u8 = 0x02;
pub const MSG_TYPE_PAIR_REQUEST: u8 = 0x03;
pub const MSG_TYPE_PAIR_RESPONSE: u8 = 0x04;
pub const MSG_TYPE_SESSION_HANDSHAKE: u8 = 0x10;
pub const MSG_TYPE_SESSION_APP: u8 = 0x11;

pub const MAX_FRAME_SIZE: usize = 65535;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairRequestMessage {
    pub from_device_id: String,
    pub from_device_name: String,
    pub from_fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairResponseMessage {
    pub accepted: bool,
    pub code: Option<String>,
    pub qr_payload: Option<crate::pairing::QrPairingPayload>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairingExchangeInfo {
    pub device_id: String,
    pub device_name: String,
    pub public_key_hex: String,
    pub noise_static_key_hex: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppMessage {
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Hello { device_id: String, device_name: String },
    Disconnect { reason: String },
    RevokeTrust { from_device_id: String },
}

pub async fn write_frame<W>(
    writer: &mut W,
    msg_type: u8,
    payload: &[u8],
) -> Result<(), CoreError>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_FRAME_SIZE {
        return Err(CoreError::Protocol(format!(
            "Payload length {} exceeds maximum frame size {}",
            payload.len(),
            MAX_FRAME_SIZE
        )));
    }

    let len = payload.len() as u16;
    let mut header = [0u8; 8];
    header[0..4].copy_from_slice(PROTOCOL_MAGIC);
    header[4] = PROTOCOL_VERSION;
    header[5] = msg_type;
    header[6..8].copy_from_slice(&len.to_be_bytes());

    writer.write_all(&header).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R>(reader: &mut R) -> Result<(u8, Vec<u8>), CoreError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; 8];
    reader.read_exact(&mut header).await.map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            CoreError::Protocol("Connection closed by peer".to_string())
        } else {
            CoreError::Io(e)
        }
    })?;

    if &header[0..4] != PROTOCOL_MAGIC {
        return Err(CoreError::Protocol("Invalid magic bytes in frame".to_string()));
    }

    let version = header[4];
    if version != PROTOCOL_VERSION {
        return Err(CoreError::Protocol(format!(
            "Unsupported protocol version: {}",
            version
        )));
    }

    let msg_type = header[5];
    let payload_len = u16::from_be_bytes([header[6], header[7]]) as usize;

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload).await?;

    Ok((msg_type, payload))
}
