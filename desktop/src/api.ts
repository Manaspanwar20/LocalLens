import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  DiscoveredDevice,
  IdentityDto,
  PairingResult,
  PairResponseMessage,
  QrPairingPayload,
  TrustedDevice,
} from './types';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function getIdentity(): Promise<IdentityDto> {
  if (isTauri) {
    return await invoke<IdentityDto>('get_identity');
  }
  return {
    device_id: 'mock-device-id-1234',
    device_name: 'Windows Desktop (Web Preview)',
    fingerprint: 'a1b2c3d4e5f67890abcdef1234567890abcdef1234567890abcdef1234567890',
    noise_public_key_hex: '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff',
  };
}

export async function updateDeviceName(name: string): Promise<void> {
  if (isTauri) {
    await invoke('update_device_name', { name });
  }
}

export async function startDiscovery(): Promise<void> {
  if (isTauri) {
    await invoke('start_discovery');
  }
}

export async function getDiscoveredDevices(): Promise<DiscoveredDevice[]> {
  if (isTauri) {
    return await invoke<DiscoveredDevice[]>('get_discovered_devices');
  }
  return [];
}

export async function startPairing(): Promise<QrPairingPayload> {
  if (isTauri) {
    return await invoke<QrPairingPayload>('start_pairing');
  }
  return {
    version: 1,
    device_id: 'mock-device-id-1234',
    device_name: 'Windows Desktop (Web Preview)',
    fingerprint: 'a1b2c3d4e5f67890abcdef1234567890abcdef1234567890abcdef1234567890',
    addrs: ['192.168.1.105'],
    port: 52480,
    secret: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    code: '749102',
  };
}

export async function cancelPairing(): Promise<void> {
  if (isTauri) {
    await invoke('cancel_pairing');
  }
}

export async function sendPairRequest(peerAddr: string): Promise<PairResponseMessage> {
  if (isTauri) {
    return await invoke<PairResponseMessage>('send_pair_request', { peerAddr });
  }
  return {
    accepted: true,
    code: '123456',
  };
}

export async function respondPairRequest(
  fromDeviceId: string,
  accept: boolean
): Promise<QrPairingPayload | null> {
  if (isTauri) {
    return await invoke<QrPairingPayload | null>('respond_pair_request', {
      fromDeviceId,
      accept,
    });
  }
  if (accept) {
    return {
      version: 1,
      device_id: 'mock-peer-device',
      device_name: 'Mock Device',
      fingerprint: 'abcd1234abcd1234',
      addrs: ['192.168.1.100'],
      port: 52480,
      secret: 'mocksecret',
      code: '654321',
    };
  }
  return null;
}

export async function initiatePairing(
  peerAddr: string,
  mode: 'qr' | 'code',
  secretOrCode: string
): Promise<PairingResult> {
  if (isTauri) {
    return await invoke<PairingResult>('initiate_pairing', {
      peerAddr,
      mode,
      secretOrCode,
    });
  }
  return {
    peer: {
      device_id: 'mock-peer-5678',
      device_name: 'Android Phone (Mock)',
      public_key_hex: 'mockpubkeyhex',
      noise_static_key_hex: 'mocknoisedhhex',
      fingerprint: 'fedcba0987654321',
      paired_at: Date.now() / 1000,
    },
    sas: {
      numeric: '492 810',
      emoji: ['🦊', '🚀', '💎', '🛡️'],
    },
  };
}

export async function listTrustedDevices(): Promise<TrustedDevice[]> {
  if (isTauri) {
    return await invoke<TrustedDevice[]>('list_trusted_devices');
  }
  return [];
}

export async function renameTrustedDevice(deviceId: string, newName: string): Promise<TrustedDevice> {
  if (isTauri) {
    return await invoke<TrustedDevice>('rename_trusted_device', { deviceId, newName });
  }
  return {
    device_id: deviceId,
    device_name: newName,
    public_key_hex: '',
    noise_static_key_hex: '',
    fingerprint: '',
    paired_at: Date.now() / 1000,
  };
}

export async function revokeTrustedDevice(deviceId: string): Promise<TrustedDevice> {
  if (isTauri) {
    return await invoke<TrustedDevice>('revoke_trusted_device', { deviceId });
  }
  return {
    device_id: deviceId,
    device_name: 'Revoked',
    public_key_hex: '',
    noise_static_key_hex: '',
    fingerprint: '',
    paired_at: Date.now() / 1000,
  };
}

export async function pingPeer(peerAddr: string, deviceId: string): Promise<number> {
  if (isTauri) {
    return await invoke<number>('ping_peer', { peerAddr, deviceId });
  }
  return 14;
}

export async function onDiscoveredDevices(callback: (devices: DiscoveredDevice[]) => void): Promise<UnlistenFn> {
  if (isTauri) {
    return await listen<DiscoveredDevice[]>('discovered_devices', (event) => {
      callback(event.payload);
    });
  }
  return () => {};
}

export async function onServiceEvent(callback: (eventPayload: any) => void): Promise<UnlistenFn> {
  if (isTauri) {
    return await listen('service_event', (event) => {
      callback(event.payload);
    });
  }
  return () => {};
}
