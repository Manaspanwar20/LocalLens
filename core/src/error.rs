use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Noise cryptographic error: {0}")]
    Noise(String),

    #[error("Cryptographic operation error: {0}")]
    Crypto(String),

    #[error("Secure store error: {0}")]
    SecureStore(String),

    #[error("mDNS discovery error: {0}")]
    Discovery(String),

    #[error("Pairing error: {0}")]
    Pairing(String),

    #[error("Pairing secret has expired")]
    PairingExpired,

    #[error("Pairing is locked due to too many failed attempts; try again in {0} seconds")]
    PairingRateLimited(u64),

    #[error("Pairing secret is invalid or single-use secret has already been consumed")]
    PairingSecretInvalid,

    #[error("Peer identity rejected: static key {0} is not in trusted store")]
    UntrustedPeer(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl From<snow::Error> for CoreError {
    fn from(err: snow::Error) -> Self {
        CoreError::Noise(err.to_string())
    }
}
