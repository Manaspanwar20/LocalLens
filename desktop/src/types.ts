export interface IdentityDto {
  device_id: string;
  device_name: string;
  fingerprint: string;
  noise_public_key_hex: string;
}

export type DeviceStatus = 'Paired' | 'NotPaired' | 'Online' | 'Offline';

export interface DiscoveredDevice {
  device_id: string;
  device_name: string;
  protocol_version: string;
  pairing_mode: boolean;
  fingerprint: string;
  addresses: string[];
  port: number;
  status: DeviceStatus;
  last_seen: number;
}

export interface TrustedDevice {
  device_id: string;
  device_name: string;
  public_key_hex: string;
  noise_static_key_hex: string;
  fingerprint: string;
  paired_at: number;
  last_seen?: number;
}

export interface QrPairingPayload {
  version: number;
  device_id: string;
  device_name: string;
  fingerprint: string;
  addrs: string[];
  port: number;
  secret: string; // 64 hex chars
  code: string;   // 6 digits
}

export interface SasConfirmation {
  numeric: string;
  emoji: string[];
}

export interface PairingResult {
  peer: TrustedDevice;
  sas: SasConfirmation;
}
