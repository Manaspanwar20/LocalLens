pub mod error;
pub mod secure_store;
pub mod identity;
pub mod trust;
pub mod protocol;
pub mod pairing;
pub mod session;
pub mod discovery;
pub mod service;

pub use error::CoreError;
pub use identity::DeviceIdentity;
pub use trust::{TrustedDevice, TrustStore};
pub use discovery::{DiscoveredDevice, DeviceStatus, DiscoveryManager};
pub use pairing::{PairingManager, PairingAuthMode, QrPairingPayload, SasConfirmation};
pub use session::{SessionManager, EncryptedSession};
pub use service::{AppService, IdentityDto, ServiceEvent};
