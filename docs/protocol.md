# LocalShare Protocol Specification (v1)

## 1. Overview
LocalShare is a local-first, peer-to-peer device discovery and pairing protocol designed for zero-configuration, secure local network communications. It establishes mutual trust between devices without reliance on external cloud servers, certificate authorities (CAs), or centralized registries.

---

## 2. Discovery Layer (mDNS / DNS-SD)

### Service Registration
Devices advertise and browse using standard Multicast DNS (RFC 6762) and DNS-Based Service Discovery (RFC 6763).
- **Service Type**: `_localshare._tcp.local.`
- **Transport**: UDP port 5353 (multicast group `224.0.0.251` or `[FF02::FB]`).
- **Instance Name**: `<sanitized_device_name>-<short_device_id>`

### TXT Record Attributes
The DNS-SD TXT record conveys public presence information strictly without secrets or key material:
| Key | Type | Description |
| :--- | :--- | :--- |
| `device_id` | String | Unique persistent UUIDv4 of the advertising device. |
| `device_name` | String | Friendly device display name (e.g. "Alice's Laptop"). |
| `protocol_version` | String | Protocol version string, currently `"1"`. |
| `pairing_mode` | String | `"1"` if device is in pairing mode, `"0"` otherwise. |
| `fingerprint` | String | SHA-256 hex hash of the device's public Ed25519 identity key. |

---

## 3. Cryptographic Primitives & Key Management

- **Long-term Identity Keypair**: Ed25519 (`ed25519-dalek`). Generated on first application launch.
- **Diffie-Hellman Keypair**: X25519 (`x25519-dalek`) derived deterministically via HKDF-SHA256 from the master seed:
  `HKDF-Expand(PRK, info = "localshare-identity-v1-x25519", len = 32)`
- **Key Derivation Function**: HKDF-SHA256 (RFC 5869).
- **AEAD Cipher**: ChaCha20-Poly1305 (RFC 8439).
- **Hash Function**: BLAKE2s / SHA-256.
- **OS Secure Store**:
  - **Windows**: Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`). Encrypted at rest using user logon credentials.
  - **Android**: Android Keystore hardware-backed master key wrapping AES-256-GCM.
  - **Zero plain text**: Private keys are never stored in unencrypted files.

---

## 4. Framing & Wire Envelope

All communication over the TCP port uses length-delimited binary framing:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      MAGIC ("LSHR")                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    VERSION    |   MSG_TYPE    |          PAYLOAD_LEN          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                            PAYLOAD                            |
|                              ...                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **MAGIC** (4 bytes): `0x4C 0x53 0x48 0x52` (`"LSHR"`)
- **VERSION** (1 byte): `0x01`
- **MSG_TYPE** (1 byte):
  - `0x01`: `PairHandshake` (Noise_XXpsk3 handshake tokens)
  - `0x02`: `PairInfo` (Encrypted post-handshake identity exchange)
  - `0x10`: `SessionHandshake` (Noise_XX mutual authentication handshake)
  - `0x11`: `SessionApp` (Encrypted application payloads: Ping, Pong, Hello, Disconnect)
- **PAYLOAD_LEN** (2 bytes, Big-Endian): 16-bit payload length (max 65,535 bytes).
- **PAYLOAD**: Message bytes or AEAD encrypted ciphertext.

---

## 5. Pairing Flow (`Noise_XXpsk3`)

When a user taps "Pair Device" on Desktop:
1. **Secret Generation**:
   - 256-bit cryptographic random secret ($S_{256}$).
   - 6-digit numeric fallback code ($C_6$, $100000 \le C_6 \le 999999$).
   - Both expire in **120 seconds** and are **single-use**.
2. **PSK Derivation**:
   - QR mode: $PSK = \text{HKDF-Expand}(\text{salt}=\text{"localshare-pairing-qr-v1"}, IKM=S_{256}, \text{info}=\text{"pairing-psk-32"})$
   - Numeric Code mode: $PSK = \text{HKDF-Expand}(\text{salt}=\text{"localshare-pairing-code-v1"}, IKM=C_6, \text{info}=\text{"pairing-psk-32"})$
3. **Handshake Sequence**:
   ```
   Initiator (Connecting Device)                    Responder (Displaying QR)
               |                                               |
               | ---- (1) [Mode Byte | Noise e] -------------> |
               |                                               |
               | <--- (2) [Noise e, ee, s, es] --------------- |
               |                                               |
               | ---- (3) [Noise s, se, psk] ----------------> |
               |                                               |
      [Noise Transport Mode Established]             [Noise Transport Mode Established]
               |                                               |
               | <--- (4) Encrypted PairingExchangeInfo ------ |
               |                                               |
               | ---- (5) Encrypted PairingExchangeInfo -----> |
   ```
4. **Rate Limiting & Lockout**:
   - If message (3) decryption fails (invalid secret/code), failure counter increments.
   - After **5 consecutive failures**, pairing is locked for **60 seconds**.
   - Single-use: upon successful completion, the pairing session is invalidated.
5. **Short Authentication String (SAS) Verification**:
   - Both peers extract `handshake_hash = handshake.get_handshake_hash()`.
   - 6-digit SAS: `(u32::from_be_bytes(hash[0..4]) % 900000) + 100000`
   - 4-emoji SAS: `[emoji_table[hash[4] % 64], emoji_table[hash[5] % 64], emoji_table[hash[6] % 64], emoji_table[hash[7] % 64]]`
   - Both users compare the SAS visually to verify against active MITM tampering.
6. **Key Pinning**:
   - Both peers add the other's public key + device_id + friendly name to their local `TrustStore`, encrypted at rest via the OS secure store.

---

## 6. Authenticated Sessions (`Noise_XX`)

Subsequent sessions between paired devices establish a mutually authenticated, forward-secret channel:

```
Initiator (Paired Peer)                          Responder (Listening Peer)
            |                                               |
            | ---- (1) [Noise e] -------------------------> |
            |                                               |
            | <--- (2) [Noise e, ee, s, es] --------------- |
            |      [Initiator checks if s in TrustStore]    |
            |      [If unknown -> Abort connection]         |
            |                                               |
            | ---- (3) [Noise s, se] ---------------------> |
            |      [Responder checks if s in TrustStore]    |
            |      [If unknown -> Drop socket before data]  |
            |                                               |
   [Secure Transport Active]                       [Secure Transport Active]
            |                                               |
            | ---- Ping { timestamp } --------------------> |
            | <--- Pong { timestamp } --------------------- |
```

- **Drop Before Data**: Any connection where the remote static public key is not in the `TrustStore` is closed immediately with a security log. No application data is processed.
- **Revocation**: When a device is revoked in the UI, its key is deleted from `TrustStore` and any active TCP socket associated with its `device_id` is aborted immediately.

---

## 7. Threat Model Analysis

| Threat | Attack Vector | Mitigation in LocalShare |
| :--- | :--- | :--- |
| **Attacker on Same Wi-Fi** | Malicious device on the open/shared local Wi-Fi | All communications are encrypted with ChaCha20-Poly1305 AEAD. Untrusted devices cannot establish sessions. |
| **Passive Sniffing** | Wi-Fi packet capture (Wireshark, monitor mode) | Ephemeral Diffie-Hellman keys provide forward secrecy. Pre-shared keys are never transmitted over the wire. mDNS records contain only public fingerprints. |
| **Active Man-in-the-Middle (MITM)** | ARP spoofing or rogue Wi-Fi AP intercepting handshake | During pairing: PSK mixed into Noise pattern ensures only devices with the pairing secret can authenticate; SAS visual comparison (6 digits + 4 emoji) detects any mismatch. During sessions: Static public keys are pinned in the local `TrustStore`. |
| **Replay Attacks** | Capturing and replaying handshake frames | Handshake messages use freshly generated ephemeral keys ($e$) on every connection. AEAD nonces are strictly incremented per frame. Replayed messages fail decryption. |
| **Brute-forcing 6-Digit Code** | Automated rapid connection attempts guessing codes | Rate limiting locks pairing for 60 seconds after 5 failed attempts. Codes expire in 120 seconds. An attacker can make at most 5–10 guesses ($p \le 0.001\%$). |
| **Stolen Paired Device** | Physical compromise of a device | Identity seeds and trusted peer keys are encrypted at rest with Windows DPAPI / Android Keystore. The compromised device can be immediately revoked from other peers, terminating all access. |

### What is Protected
- Confidentiality and integrity of all message payloads.
- Mutual peer identity verification before application payload handling.
- Protection against unauthorized local network peers connecting or impersonating devices.
- Protection against at-rest plaintext key theft.

### What is NOT Protected (Out of Scope for Milestone 1)
- Metadata leakage of device presence: mDNS broadcasts reveal that a device with a given name is on the local subnet.
- Denial of Service (DoS): an attacker on the local network could flood the mDNS or TCP port with garbage packets.
- Endpoint malware: if malicious code is running with root/SYSTEM privileges on the host machine, it could theoretically interact with DPAPI in the user's active login session.
