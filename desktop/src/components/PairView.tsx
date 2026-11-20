import React, { useState, useEffect } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { ShieldCheck, Clock, XCircle, ArrowRight, AlertCircle, Copy, Check } from 'lucide-react';
import type { DiscoveredDevice, QrPairingPayload, SasConfirmation, TrustedDevice } from '../types';
import { cancelPairing, initiatePairing, startPairing } from '../api';

interface PairViewProps {
  initialTargetDevice?: DiscoveredDevice | null;
  onPairingCompleted: (peer: TrustedDevice) => void;
}

export const PairView: React.FC<PairViewProps> = ({
  initialTargetDevice,
  onPairingCompleted,
}) => {
  const [mode, setMode] = useState<'advertise' | 'connect'>(
    initialTargetDevice ? 'connect' : 'advertise'
  );

  // Advertise state
  const [payload, setPayload] = useState<QrPairingPayload | null>(null);
  const [secondsRemaining, setSecondsRemaining] = useState<number>(120);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string>('');

  // Connect state
  const [targetIp, setTargetIp] = useState<string>(
    initialTargetDevice?.addresses[0] || ''
  );
  const [targetPort, setTargetPort] = useState<string>(
    initialTargetDevice ? String(initialTargetDevice.port) : '52480'
  );
  const [authMethod, setAuthMethod] = useState<'code' | 'qr'>('code');
  const [secretOrCode, setSecretOrCode] = useState<string>('');
  const [isPairingConnecting, setIsPairingConnecting] = useState<boolean>(false);

  // Confirmation state
  const [sasConfirmation, setSasConfirmation] = useState<{
    peer: TrustedDevice;
    sas: SasConfirmation;
  } | null>(null);
  const [copiedCode, setCopiedCode] = useState<boolean>(false);

  const startHostingPairing = async () => {
    setIsLoading(true);
    setErrorMsg('');
    try {
      const p = await startPairing();
      setPayload(p);
      setSecondsRemaining(120);
    } catch (err: any) {
      setErrorMsg(typeof err === 'string' ? err : 'Failed to start pairing session');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    if (mode === 'advertise') {
      startHostingPairing();
    }
    return () => {
      cancelPairing().catch(() => {});
    };
  }, [mode]);

  // Countdown timer for 120s expiry
  useEffect(() => {
    if (mode !== 'advertise' || !payload || secondsRemaining <= 0) return;
    const timer = setInterval(() => {
      setSecondsRemaining((prev) => {
        if (prev <= 1) {
          clearInterval(timer);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(timer);
  }, [mode, payload, secondsRemaining]);

  const handleCancel = async () => {
    await cancelPairing();
    setPayload(null);
    setSecondsRemaining(0);
  };

  const handleConnectPeer = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!targetIp || !targetPort || !secretOrCode) {
      setErrorMsg('Please fill in IP address, port, and code or secret.');
      return;
    }
    setIsPairingConnecting(true);
    setErrorMsg('');

    try {
      const peerAddr = `${targetIp.trim()}:${targetPort.trim()}`;
      const res = await initiatePairing(
        peerAddr,
        authMethod,
        secretOrCode.replace(/\s+/g, '')
      );
      setSasConfirmation(res);
      onPairingCompleted(res.peer);
    } catch (err: any) {
      setErrorMsg(
        typeof err === 'string'
          ? err
          : 'Pairing failed. Check secret code or network reachability.'
      );
    } finally {
      setIsPairingConnecting(false);
    }
  };

  const copyCode = () => {
    if (payload?.code) {
      navigator.clipboard.writeText(payload.code);
      setCopiedCode(true);
      setTimeout(() => setCopiedCode(false), 2000);
    }
  };

  const formatTimer = (s: number) => {
    const mins = Math.floor(s / 60);
    const secs = s % 60;
    return `${mins}:${secs < 10 ? '0' : ''}${secs}`;
  };

  // If pairing succeeded, show SAS confirmation dialog
  if (sasConfirmation) {
    return (
      <div style={{ maxWidth: '580px', margin: '40px auto', padding: '0 20px' }}>
        <div className="card" style={{ padding: '32px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', color: 'var(--status-online)', marginBottom: '16px' }}>
            <ShieldCheck size={28} />
            <h2 style={{ fontSize: '18px', fontWeight: 600, color: 'var(--text-main)' }}>
              Pairing Succeeded
            </h2>
          </div>

          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '24px', lineHeight: 1.6 }}>
            Both devices have proven possession of the pairing secret and exchanged long-term identity keys. To prevent active man-in-the-middle attacks on the local Wi-Fi, verify that the following authentication string matches on both screens:
          </p>

          {/* SAS Code Display */}
          <div style={{
            backgroundColor: 'var(--bg-surface-subtle)',
            border: '1px solid var(--border-subtle)',
            borderRadius: 'var(--radius-md)',
            padding: '24px',
            textAlign: 'center',
            marginBottom: '24px',
          }}>
            <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '1px', color: 'var(--text-muted)', marginBottom: '8px' }}>
              Short Authentication String (SAS)
            </div>

            <div className="mono" style={{ fontSize: '32px', fontWeight: 700, letterSpacing: '4px', marginBottom: '12px' }}>
              {sasConfirmation.sas.numeric}
            </div>

            <div style={{ fontSize: '28px', display: 'flex', justifyContent: 'center', gap: '12px' }}>
              {sasConfirmation.sas.emoji.map((em, idx) => (
                <span key={idx}>{em}</span>
              ))}
            </div>
          </div>

          <div style={{
            fontSize: '12px',
            color: 'var(--text-muted)',
            marginBottom: '24px',
            padding: '12px',
            backgroundColor: 'var(--bg-app)',
            borderRadius: 'var(--radius-sm)',
          }}>
            <div><strong>Paired Device:</strong> {sasConfirmation.peer.device_name}</div>
            <div className="mono" style={{ marginTop: '4px' }}>
              <strong>Device ID:</strong> {sasConfirmation.peer.device_id}
            </div>
            <div className="mono" style={{ marginTop: '2px' }}>
              <strong>Key Fingerprint:</strong> {sasConfirmation.peer.fingerprint}
            </div>
          </div>

          <button
            className="primary"
            style={{ width: '100%', padding: '10px' }}
            onClick={() => setSasConfirmation(null)}
          >
            Confirmed — Device is Trusted
          </button>
        </div>
      </div>
    );
  }

  return (
    <div style={{ maxWidth: '680px', margin: '24px auto', padding: '0 20px' }}>
      {/* Mode Switcher */}
      <div style={{
        display: 'flex',
        borderBottom: '1px solid var(--border-subtle)',
        marginBottom: '24px',
        gap: '16px',
      }}>
        <button
          onClick={() => setMode('advertise')}
          style={{
            background: 'none',
            border: 'none',
            borderBottom: mode === 'advertise' ? '2px solid var(--accent)' : '2px solid transparent',
            color: mode === 'advertise' ? 'var(--text-main)' : 'var(--text-muted)',
            fontWeight: 600,
            borderRadius: 0,
            padding: '10px 4px',
          }}
        >
          Show QR / Pairing Code
        </button>

        <button
          onClick={() => setMode('connect')}
          style={{
            background: 'none',
            border: 'none',
            borderBottom: mode === 'connect' ? '2px solid var(--accent)' : '2px solid transparent',
            color: mode === 'connect' ? 'var(--text-main)' : 'var(--text-muted)',
            fontWeight: 600,
            borderRadius: 0,
            padding: '10px 4px',
          }}
        >
          Enter Peer Code
        </button>
      </div>

      {errorMsg && (
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          padding: '10px 14px',
          backgroundColor: 'var(--status-danger-bg)',
          color: 'var(--status-danger)',
          borderRadius: 'var(--radius-sm)',
          fontSize: '13px',
          marginBottom: '20px',
        }}>
          <AlertCircle size={16} />
          <span>{errorMsg}</span>
        </div>
      )}

      {mode === 'advertise' ? (
        <div className="card" style={{ padding: '28px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '20px' }}>
            <div>
              <h3 style={{ fontSize: '16px', fontWeight: 600 }}>Pairing Mode Active</h3>
              <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
                Scan this QR code with the LocalShare mobile app, or enter the 6-digit numeric fallback code.
              </p>
            </div>

            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              fontSize: '13px',
              fontWeight: 600,
              color: secondsRemaining < 30 ? 'var(--status-danger)' : 'var(--text-main)',
              backgroundColor: 'var(--bg-surface-subtle)',
              padding: '4px 10px',
              borderRadius: 'var(--radius-sm)',
            }}>
              <Clock size={14} />
              <span>{formatTimer(secondsRemaining)}</span>
            </div>
          </div>

          {secondsRemaining <= 0 ? (
            <div style={{ textAlign: 'center', padding: '36px 0' }}>
              <XCircle size={36} color="var(--status-offline)" style={{ margin: '0 auto 12px auto' }} />
              <h4 style={{ fontSize: '15px', fontWeight: 500, marginBottom: '6px' }}>Pairing code expired</h4>
              <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '16px' }}>
                Pairing secrets expire in 2 minutes for security. Generate a new code to continue.
              </p>
              <button className="primary" onClick={startHostingPairing} disabled={isLoading}>
                Generate New Code
              </button>
            </div>
          ) : payload ? (
            <div style={{ display: 'flex', gap: '32px', alignItems: 'center' }}>
              {/* QR Code */}
              <div style={{
                padding: '16px',
                backgroundColor: '#ffffff',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-subtle)',
                boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
                display: 'inline-block',
              }}>
                <QRCodeSVG
                  value={JSON.stringify(payload)}
                  size={190}
                  level="M"
                  includeMargin={false}
                />
              </div>

              {/* Code Info & Details */}
              <div style={{ flex: 1 }}>
                <div style={{ marginBottom: '16px' }}>
                  <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.8px', color: 'var(--text-muted)', marginBottom: '4px' }}>
                    6-Digit Fallback Code
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <span className="mono" style={{ fontSize: '26px', fontWeight: 700, letterSpacing: '3px' }}>
                      {payload.code.slice(0, 3)} {payload.code.slice(3, 6)}
                    </span>
                    <button className="secondary" onClick={copyCode} style={{ padding: '4px 8px' }}>
                      {copiedCode ? <Check size={14} color="var(--status-online)" /> : <Copy size={14} />}
                    </button>
                  </div>
                </div>

                <div style={{ fontSize: '12px', color: 'var(--text-muted)', display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '20px' }}>
                  <div><strong>IP:</strong> {payload.addrs.join(', ')}</div>
                  <div><strong>Port:</strong> {payload.port}</div>
                  <div><strong>Security:</strong> 256-bit Noise PSK, single-use</div>
                </div>

                <button className="danger" onClick={handleCancel}>
                  Cancel Pairing
                </button>
              </div>
            </div>
          ) : (
            <div style={{ textAlign: 'center', padding: '36px' }}>
              Generating secure pairing session...
            </div>
          )}
        </div>
      ) : (
        /* Connect to Peer Tab */
        <div className="card" style={{ padding: '28px' }}>
          <h3 style={{ fontSize: '16px', fontWeight: 600, marginBottom: '4px' }}>
            Pair with Another Device
          </h3>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '20px' }}>
            Enter the peer device's IP address and 6-digit pairing code shown on their screen.
          </p>

          <form onSubmit={handleConnectPeer}>
            <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: '12px', marginBottom: '14px' }}>
              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 500, marginBottom: '6px' }}>
                  Peer IP Address
                </label>
                <input
                  type="text"
                  placeholder="192.168.1.100"
                  value={targetIp}
                  onChange={(e) => setTargetIp(e.target.value)}
                  style={{ width: '100%' }}
                  required
                />
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 500, marginBottom: '6px' }}>
                  Port
                </label>
                <input
                  type="text"
                  placeholder="52480"
                  value={targetPort}
                  onChange={(e) => setTargetPort(e.target.value)}
                  style={{ width: '100%' }}
                  required
                />
              </div>
            </div>

            <div style={{ marginBottom: '20px' }}>
              <div style={{ display: 'flex', gap: '12px', marginBottom: '8px' }}>
                <label style={{ fontSize: '12px', display: 'flex', alignItems: 'center', gap: '4px', cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="authMethod"
                    checked={authMethod === 'code'}
                    onChange={() => setAuthMethod('code')}
                  />
                  <span>6-Digit Code</span>
                </label>
                <label style={{ fontSize: '12px', display: 'flex', alignItems: 'center', gap: '4px', cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="authMethod"
                    checked={authMethod === 'qr'}
                    onChange={() => setAuthMethod('qr')}
                  />
                  <span>256-bit Hex Secret</span>
                </label>
              </div>

              <input
                type="text"
                placeholder={authMethod === 'code' ? 'e.g. 749 102' : '64-character hex secret'}
                value={secretOrCode}
                onChange={(e) => setSecretOrCode(e.target.value)}
                style={{ width: '100%', fontSize: '14px' }}
                required
              />
            </div>

            <button
              type="submit"
              className="primary"
              disabled={isPairingConnecting}
              style={{ width: '100%', padding: '10px' }}
            >
              {isPairingConnecting ? (
                'Running Authenticated Key Exchange...'
              ) : (
                <>
                  <span>Pair with Device</span>
                  <ArrowRight size={15} />
                </>
              )}
            </button>
          </form>
        </div>
      )}
    </div>
  );
};
