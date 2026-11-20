import React, { useState } from 'react';
import { Wifi, Radio, Activity, Link2, CheckCircle2, AlertCircle } from 'lucide-react';
import type { DiscoveredDevice, TrustedDevice } from '../types';
import { pingPeer } from '../api';

interface HomeViewProps {
  devices: DiscoveredDevice[];
  trustedDevices: TrustedDevice[];
  isScanning: boolean;
  scanCountdown: number;
  onConnect: () => void;
  onStartPairWith: (device: DiscoveredDevice) => void;
}

export const HomeView: React.FC<HomeViewProps> = ({
  devices,
  trustedDevices,
  isScanning,
  scanCountdown,
  onConnect,
  onStartPairWith,
}) => {
  const [pingLatencies, setPingLatencies] = useState<Record<string, number>>({});
  const [pinging, setPinging] = useState<Record<string, boolean>>({});
  const [pingErrors, setPingErrors] = useState<Record<string, string>>({});

  const handlePing = async (device: DiscoveredDevice) => {
    if (device.addresses.length === 0) return;
    const addr = `${device.addresses[0]}:${device.port}`;
    setPinging((prev) => ({ ...prev, [device.device_id]: true }));
    setPingErrors((prev) => ({ ...prev, [device.device_id]: '' }));

    try {
      const ms = await pingPeer(addr, device.device_id);
      setPingLatencies((prev) => ({ ...prev, [device.device_id]: ms }));
    } catch (err: any) {
      setPingErrors((prev) => ({
        ...prev,
        [device.device_id]: typeof err === 'string' ? err : 'Connection dropped or rejected',
      }));
    } finally {
      setPinging((prev) => ({ ...prev, [device.device_id]: false }));
    }
  };

  return (
    <div style={{ maxWidth: '860px', margin: '0 auto', padding: '24px 20px' }}>
      {/* Header bar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        marginBottom: '20px',
      }}>
        <div>
          <h2 style={{ fontSize: '18px', fontWeight: 600 }}>Nearby Devices</h2>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
            Discovered over local Wi-Fi via mDNS (<code className="mono">_localshare._tcp</code>)
          </p>
        </div>

        {isScanning && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            fontSize: '12px',
            color: 'var(--text-muted)',
            backgroundColor: 'var(--bg-surface)',
            padding: '4px 10px',
            borderRadius: 'var(--radius-sm)',
            border: '1px solid var(--border-subtle)',
          }}>
            <Radio size={14} color="var(--accent)" />
            <span>Scanning Wi-Fi ({scanCountdown}s remaining)...</span>
          </div>
        )}
      </div>

      {/* Device List */}
      {devices.length === 0 ? (
        <div className="card" style={{
          textAlign: 'center',
          padding: '48px 24px',
          borderStyle: 'dashed',
        }}>
          <Wifi size={36} color="var(--border-strong)" style={{ margin: '0 auto 12px auto' }} />
          <h3 style={{ fontSize: '15px', fontWeight: 500, marginBottom: '6px' }}>
            No devices found on the local network
          </h3>
          <p style={{
            color: 'var(--text-muted)',
            fontSize: '13px',
            maxWidth: '440px',
            margin: '0 auto 16px auto',
            lineHeight: 1.6,
          }}>
            Click "Connect" above to scan for nearby peers running LocalShare. Make sure both devices are connected to the same Wi-Fi router and Windows Firewall has allowed the application.
          </p>
          <button className="primary" onClick={onConnect} disabled={isScanning}>
            {isScanning ? 'Scanning in progress...' : 'Start 10s Wi-Fi Scan'}
          </button>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {devices.map((device) => {
            const isPaired = trustedDevices.some((t) => t.device_id === device.device_id);
            const latency = pingLatencies[device.device_id];
            const isPinging = pinging[device.device_id];
            const pingErr = pingErrors[device.device_id];
            const primaryAddr = device.addresses[0] || 'Unknown IP';

            return (
              <div
                key={device.device_id}
                className="card"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '14px 18px',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
                  <div style={{
                    width: '36px',
                    height: '36px',
                    borderRadius: 'var(--radius-sm)',
                    backgroundColor: 'var(--bg-surface-subtle)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    border: '1px solid var(--border-subtle)',
                  }}>
                    <Wifi size={18} color="var(--text-muted)" />
                  </div>

                  <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <span style={{ fontWeight: 600, fontSize: '14px' }}>{device.device_name}</span>
                      
                      {isPaired ? (
                        <span className="badge online">
                          <CheckCircle2 size={12} /> Paired
                        </span>
                      ) : (
                        <span className="badge">
                          Not paired
                        </span>
                      )}

                      {device.pairing_mode && (
                        <span className="badge paired">
                          Ready to pair
                        </span>
                      )}
                    </div>

                    <div style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '12px',
                      marginTop: '4px',
                      color: 'var(--text-muted)',
                      fontSize: '12px',
                    }}>
                      <span className="mono">{primaryAddr}:{device.port}</span>
                      <span>•</span>
                      <span className="mono">FP: {device.fingerprint.slice(0, 10)}...</span>
                      <span>•</span>
                      <span>v{device.protocol_version}</span>
                    </div>

                    {latency !== undefined && (
                      <div style={{
                        marginTop: '6px',
                        fontSize: '12px',
                        color: 'var(--status-online)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px',
                        fontWeight: 500,
                      }}>
                        <Activity size={13} />
                        <span>Connected, latency {latency} ms</span>
                      </div>
                    )}

                    {pingErr && (
                      <div style={{
                        marginTop: '6px',
                        fontSize: '12px',
                        color: 'var(--status-danger)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px',
                      }}>
                        <AlertCircle size={13} />
                        <span>{pingErr}</span>
                      </div>
                    )}
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  {isPaired ? (
                    <button
                      className="secondary"
                      onClick={() => handlePing(device)}
                      disabled={isPinging}
                      style={{ minWidth: '85px' }}
                    >
                      <Activity size={14} />
                      <span>{isPinging ? 'Pinging...' : 'Ping'}</span>
                    </button>
                  ) : (
                    <button
                      className="primary"
                      onClick={() => onStartPairWith(device)}
                    >
                      <Link2 size={14} />
                      <span>Pair</span>
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
