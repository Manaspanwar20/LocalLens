import React, { useState } from 'react';
import { Wifi, Radio, Activity, Link2, CheckCircle2, AlertCircle, Loader2 } from 'lucide-react';
import type { DiscoveredDevice, PairResponseMessage, TrustedDevice } from '../types';
import { pingPeer, sendPairRequest } from '../api';

interface HomeViewProps {
  devices: DiscoveredDevice[];
  trustedDevices: TrustedDevice[];
  isScanning: boolean;
  scanCountdown: number;
  onConnect: () => void;
  onStartPairWith: (device: DiscoveredDevice, pairResponse?: PairResponseMessage) => void;
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

  // Pairing request state
  const [pairingWithId, setPairingWithId] = useState<string | null>(null);
  const [pairError, setPairError] = useState<string | null>(null);

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

  const handlePairClick = async (device: DiscoveredDevice) => {
    if (device.addresses.length === 0) {
      setPairError(`No reachable network address for ${device.device_name}`);
      return;
    }

    setPairingWithId(device.device_id);
    setPairError(null);

    const peerAddr = `${device.addresses[0]}:${device.port}`;
    try {
      const response = await sendPairRequest(peerAddr);
      if (response.accepted) {
        onStartPairWith(device, response);
      } else {
        setPairError(`${device.device_name} declined the pairing request.`);
      }
    } catch (err: any) {
      const msg = typeof err === 'string' ? err : err?.message || 'Could not send pairing request';
      setPairError(msg);
    } finally {
      setPairingWithId(null);
    }
  };

  return (
    <div className="content-container" style={{ maxWidth: '860px', margin: '0 auto', padding: '24px 20px', width: '100%' }}>
      {/* Header bar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        marginBottom: '20px',
        flexWrap: 'wrap',
        gap: '12px',
      }}>
        <div>
          <h2 style={{ fontSize: '18px', fontWeight: 600 }}>Nearby Devices</h2>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
            Discovered over local Wi-Fi
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

      {/* Global Pair Error banner */}
      {pairError && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: '8px',
            padding: '12px 14px',
            backgroundColor: 'var(--status-danger-bg)',
            color: 'var(--status-danger)',
            borderRadius: 'var(--radius-sm)',
            fontSize: '13px',
            marginBottom: '16px',
            border: '1px solid rgba(220, 38, 38, 0.2)',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <AlertCircle size={16} style={{ flexShrink: 0 }} />
            <span>{pairError}</span>
          </div>
          <button
            onClick={() => setPairError(null)}
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--status-danger)',
              cursor: 'pointer',
              padding: '2px 6px',
              fontSize: '12px',
              fontWeight: 600,
            }}
          >
            Dismiss
          </button>
        </div>
      )}

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
            Click "Scan Wi-Fi" to discover nearby devices running LocalShare. Make sure both devices are on the same Wi-Fi network.
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
            const isPairingThis = pairingWithId === device.device_id;

            return (
              <div
                key={device.device_id}
                className="card"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '12px 16px',
                  gap: '12px',
                  flexWrap: 'wrap',
                }}
              >
                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  minWidth: 0,
                  flex: '1 1 200px',
                }}>
                  <div style={{
                    width: '38px',
                    height: '38px',
                    borderRadius: '8px',
                    backgroundColor: 'var(--bg-surface-subtle)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    border: '1px solid var(--border-subtle)',
                    flexShrink: 0,
                  }}>
                    <Wifi size={18} color="var(--accent)" />
                  </div>

                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      flexWrap: 'wrap',
                    }}>
                      <span style={{
                        fontWeight: 600,
                        fontSize: '14px',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}>
                        {device.device_name}
                      </span>
                      
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

                    {latency !== undefined && (
                      <div style={{
                        marginTop: '4px',
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
                        marginTop: '4px',
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

                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  flexShrink: 0,
                  marginLeft: 'auto',
                }}>
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
                      onClick={() => handlePairClick(device)}
                      disabled={isPairingThis || pairingWithId !== null}
                      style={{ minWidth: '85px' }}
                    >
                      {isPairingThis ? (
                        <>
                          <Loader2 size={14} style={{ animation: 'spin 1s linear infinite' }} />
                          <span>Requesting...</span>
                        </>
                      ) : (
                        <>
                          <Link2 size={14} />
                          <span>Pair</span>
                        </>
                      )}
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
