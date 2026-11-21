# LocalLens - Secure cross-platform file access and sharing, searchable by what they contain

> **Milestone 1**: Secure Device Discovery, Authenticated Pairing, and Encrypted Peer-to-Peer Sessions.

---

## 1. Project Structure & Dependency Choices

```
LocalFirstFileSharing/
├── core/                        # Pure Rust headless core library
│   ├── src/
│   │   ├── error.rs             # Typed domain errors via thiserror
│   │   ├── secure_store.rs      # OS secure store integration (Windows DPAPI)
│   │   ├── identity.rs          # Ed25519 & X25519 identity key management
│   │   ├── trust.rs             # Encrypted trust store (peer key pinning)
│   │   ├── protocol.rs          # Wire framing and length-delimited envelopes
│   │   ├── pairing.rs           # Noise_XXpsk3 handshake, SAS, rate limiting
│   │   ├── session.rs           # Noise_XX mutual authentication, ping/pong
│   │   ├── discovery.rs         # mDNS / DNS-SD responder and querier
│   │   └── service.rs           # High-level orchestrator & TCP service
│   ├── tests/
│   │   └── integration_tests.rs # Complete integration and unit test suite
│   └── Cargo.toml
├── desktop/                     # Desktop app (Tauri 2 + React + TypeScript)
│   ├── src-tauri/               # Tauri 2 Rust harness calling localshare-core
│   │   ├── capabilities/        # Tauri 2 ACL permission policies
│   │   ├── icons/               # Platform icons (ICO, ICNS, PNG)
│   │   ├── src/
│   │   │   ├── main.rs          # Windows executable entry point
│   │   │   └── lib.rs           # Tauri IPC command registrations & event emitters
│   │   └── Cargo.toml
│   ├── src/                     # Utilitarian React/TypeScript UI
│   │   ├── components/
│   │   │   ├── Navbar.tsx       # System navigation header
│   │   │   ├── HomeView.tsx     # Discovered devices & Connect scanner
│   │   │   ├── PairView.tsx     # QR code, 6-digit code, SAS confirmation
│   │   │   ├── TrustedView.tsx  # Pinned keys, rename, revoke
│   │   │   └── SettingsView.tsx # Identity fingerprint & configuration
│   │   ├── api.ts               # IPC invoke bindings
│   │   ├── types.ts             # Domain models
│   │   ├── index.css            # Clean, flat, system-font styling (light/dark)
│   │   └── App.tsx              # Main state machine
│   └── package.json
├── mobile/                      # Android / React Native scaffold
│   ├── android/                 # Native Kotlin Keystore & MulticastLock module
│   └── package.json
├── docs/
│   └── protocol.md              # Detailed protocol specification & threat model
├── Cargo.toml                   # Root Cargo workspace manifest
└── README.md
```

### Dependency Choices with One-Line Reasons
- **`snow` (0.9.6)**: The audited, standard pure-Rust implementation of the Noise Protocol Framework for both pairing PSK handshakes and mutual sessions.
- **`ed25519-dalek` (2.2.0)**: High-performance Ed25519 digital signature algorithm used for persistent device identity fingerprints.
- **`x25519-dalek` (2.0.1)**: Elliptic curve Diffie-Hellman implementation providing forward secrecy in Noise key exchanges.
- **`mdns-sd` (0.13.11)**: Pure-Rust mDNS responder and querier with zero external system daemon dependencies (works without Bonjour or Avahi).
- **`windows-sys` (0.59.0)**: Windows native DPAPI (`CryptProtectData`/`CryptUnprotectData`) API for OS-level encryption of keys at rest.
- **`tokio` (1.54)**: Production asynchronous runtime for non-blocking TCP socket management, timers, and channels.
- **`thiserror` (2.0)**: Idiomatic, typed domain errors without boilerplate.
- **`serde` & `serde_json` (1.0)**: Type-safe serialization for secure payloads and configuration.
- **`qrcode.react` (3.1)**: Lightweight SVG QR code renderer for the desktop pairing interface.
- **`lucide-react`**: Clean, functional, non-decorative system icons.

---

## 2. Threat Model

| Threat | Description | Mitigation |
| :--- | :--- | :--- |
| **Attacker on Same Wi-Fi** | Rogue device listening or injecting packets on an open Wi-Fi network | All payloads are authenticated and encrypted using ChaCha20-Poly1305 AEAD. Untrusted connections are dropped before any application data is exchanged. |
| **Passive Sniffing** | Wi-Fi packet capture via Wireshark or monitor mode | Diffie-Hellman ephemeral keys provide forward secrecy; pairing secrets are never sent in plaintext over the wire; mDNS TXT records contain zero secrets beyond public key fingerprints. |
| **Active MITM** | ARP poisoning or rogue access point altering packets | Handshake uses pre-shared secret in `Noise_XXpsk3`; post-handshake Short Authentication String (SAS, 6 digits + 4 emoji) is compared on both screens to detect tampering; paired sessions enforce strict public-key pinning. |
| **Replay Attacks** | Replaying recorded handshake or data frames | Fresh ephemeral DH keys ($e$) are generated for every connection; AEAD nonces increment strictly per frame; duplicate or out-of-order packets fail decryption. |
| **Brute-forcing Pairing Code** | Automated rapid attempts to guess the 6-digit numeric fallback code | Rate limiting locks pairing for 60 seconds after 5 failed attempts; the code automatically expires and self-destructs after 120 seconds ($p \le 0.001\%$). |
| **Stolen Paired Device** | Physical device compromise or disk extraction | Identity seeds and trusted peer keys are encrypted at rest using OS secure credentials (Windows DPAPI); revoking the device on any peer permanently drops all sessions. |

---

## 3. Instructions to Run and Test

### Prerequisites
- Rust 1.77+ (`rustc`, `cargo`)
- Node.js 18+ and `npm`

### 1. Run Unit and Integration Tests
To run the automated test suite (including trust store CRUD, secret expiry, rate limiting, and two in-process peers over live TCP):
```powershell
cargo test --manifest-path core/Cargo.toml
```
*Expected result:* 6 tests passed, including `test_in_process_network_peers_full_flow`.

### 2. Build and Run the Desktop App (Development Mode)
```powershell
cd desktop
npm install
npm run tauri dev
```
This starts the Vite React frontend and launches the native Tauri 2 desktop window with the local-first Rust core active.

### 3. Build the Desktop App (Production Bundle)
```powershell
cd desktop
npm run build
npx @tauri-apps/cli build
```

---

## 4. Known Limitations & Roadmap for Milestone 2

### Known Limitations in Milestone 1
- **Local Subnet Only**: mDNS operates via link-local multicast (`224.0.0.251:5353`), which by design does not cross WAN routers or isolated VLANs unless mDNS repeating is enabled on the network.
- **Firewall Prompt**: On Windows first launch, Windows Firewall will prompt to allow incoming TCP/UDP connections for private networks.

### Foundations for Milestone 2 (File Browsing & Transfer)
The architecture in Milestone 1 is deliberately structured to support Milestone 2 cleanly:
1. **Multiplexed Framing**: The 8-byte binary framing in `protocol.rs` (`[MAGIC][VERSION][MSG_TYPE][LENGTH][PAYLOAD]`) is ready for new message types:
   - `MSG_TYPE_FILE_CATALOG = 0x20`
   - `MSG_TYPE_FILE_REQUEST = 0x21`
   - `MSG_TYPE_FILE_CHUNK = 0x22`
2. **Streaming Transfer**: `EncryptedSession` holds a `snow::TransportState` with ChaCha20-Poly1305, allowing chunked binary streams (e.g. 64KB AEAD blocks with SHA-256 integrity verification) to stream directly over the authenticated session.
3. **Capability Authorization**: The `TrustedDevice` record can store folder access permissions (read-only, read-write) for granular file sharing between paired peers.
