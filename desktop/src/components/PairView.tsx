import React, { useState, useEffect } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import {
  ShieldCheck,
  Clock,
  XCircle,
  ArrowRight,
  AlertCircle,
  Copy,
  Check,
  Camera,
  Settings as SettingsIcon,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import type { DiscoveredDevice, QrPairingPayload, SasConfirmation, TrustedDevice } from '../types';
import { cancelPairing, initiatePairing, startPairing } from '../api';

const isAndroid = (): boolean => {
  if (typeof window === 'undefined') return false;
  return /android/i.test(navigator.userAgent);
};

interface PairViewProps {
  initialTargetDevice?: DiscoveredDevice | null;
  initialPayload?: QrPairingPayload | null;
  initialMode?: 'advertise' | 'connect';
  onPairingCompleted: (peer: TrustedDevice) => void;
  onCancel?: () => void;
}

export const PairView: React.FC<PairViewProps> = ({
  initialTargetDevice,
  initialPayload,
  initialMode,
  onPairingCompleted,
  onCancel,
}) => {
  const [mode, setMode] = useState<'advertise' | 'connect'>(() => {
    if (initialMode) return initialMode;
    if (initialPayload) return 'advertise';
    if (initialTargetDevice) return 'connect';
    return isAndroid() ? 'connect' : 'advertise';
  });

  // Advertise state
  const [payload, setPayload] = useState<QrPairingPayload | null>(initialPayload || null);
  const [secondsRemaining, setSecondsRemaining] = useState<number>(120);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string>('');

  // Connect state
  const [targetIp, setTargetIp] = useState<string>(
    initialTargetDevice?.addresses[0] || ''
  );
  const [targetPort, setTargetPort] = useState<string>(
    initialTargetDevice ? String(initialTargetDevice.port) : ''
  );
  const [secretOrCode, setSecretOrCode] = useState<string>('');
  const [showManualAddress, setShowManualAddress] = useState<boolean>(!initialTargetDevice);
  const [isPairingConnecting, setIsPairingConnecting] = useState<boolean>(false);
  const [isScanning, setIsScanning] = useState<boolean>(false);
  const [permissionDenied, setPermissionDenied] = useState<boolean>(false);

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
    if (initialPayload) {
      setPayload(initialPayload);
      setSecondsRemaining(120);
      setMode('advertise');
    }
  }, [initialPayload]);

  useEffect(() => {
    if (initialTargetDevice) {
      setTargetIp(initialTargetDevice.addresses[0] || '');
      setTargetPort(String(initialTargetDevice.port || ''));
      setMode('connect');
    }
  }, [initialTargetDevice]);

  useEffect(() => {
    if (mode === 'advertise' && !payload) {
      startHostingPairing();
    }
    return () => {
      if (mode === 'advertise') {
        cancelPairing().catch(() => {});
      }
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
    if (onCancel) onCancel();
  };

  // ── Open App Settings for camera permission ───────────────────────────
  const handleOpenSettings = async () => {
    try {
      const { openAppSettings } = await import('@tauri-apps/plugin-barcode-scanner');
      await openAppSettings();
    } catch (e) {
      console.error('Failed to open app settings:', e);
    }
  };

  // ── QR Scanner Handler (Mobile only) ──────────────────────────────────
  const handleScanQr = async () => {
    setIsScanning(true);
    setErrorMsg('');
    setPermissionDenied(false);

    try {
      const { scan, checkPermissions, requestPermissions, Format } = await import(
        '@tauri-apps/plugin-barcode-scanner'
      );

      // Check and request camera permission
      let perm = await checkPermissions();
      if (perm !== 'granted') {
        perm = await requestPermissions();
      }

      if (perm !== 'granted') {
        setPermissionDenied(true);
        setErrorMsg('Camera permission is required to scan QR codes.');
        setIsScanning(false);
        return;
      }

      // Open camera scanner
      const result = await scan({
        cameraDirection: 'back',
        formats: [Format.QRCode],
      });

      if (!result || !result.content) {
        setIsScanning(false);
        return;
      }

      let parsed: QrPairingPayload;
      try {
        parsed = JSON.parse(result.content) as QrPairingPayload;
      } catch {
        setErrorMsg('Scanned QR code is not a valid LocalShare pairing code.');
        setIsScanning(false);
        return;
      }

      if (!parsed.addrs || parsed.addrs.length === 0 || !parsed.port || !parsed.secret) {
        setErrorMsg('QR code is missing connection details.');
        setIsScanning(false);
        return;
      }

      // Try connecting to the peer using addresses from the QR payload
      setIsPairingConnecting(true);
      let lastErr: any = null;
      let successResult: { peer: TrustedDevice; sas: SasConfirmation } | null = null;

      for (const ip of parsed.addrs) {
        try {
          const peerAddr = `${ip}:${parsed.port}`;
          const res = await initiatePairing(peerAddr, 'qr', parsed.secret);
          successResult = res;
          break;
        } catch (e) {
          lastErr = e;
        }
      }

      if (successResult) {
        setSasConfirmation(successResult);
        onPairingCompleted(successResult.peer);
      } else {
        const msg = typeof lastErr === 'string' ? lastErr : lastErr?.message || 'Could not connect to peer';
        setErrorMsg(`Connection failed: ${msg}. Make sure both devices are on the same Wi-Fi.`);
      }
    } catch (err: any) {
      console.error('Scan error:', err);
      const msg = typeof err === 'string' ? err : err?.message || 'Scanner failed';
      if (/permission/i.test(msg)) {
        setPermissionDenied(true);
      }
      setErrorMsg(msg);
    } finally {
      setIsScanning(false);
      setIsPairingConnecting(false);
    }
  };

  // ── Manual Connection (6-Digit Code) ──────────────────────────────────
  const handleConnectPeer = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg('');

    let ip = targetIp.trim();
    let port = targetPort.trim();
    const code = secretOrCode.trim().replace(/\s+/g, '');

    // Strip http:// or https:// if pasted
    ip = ip.replace(/^https?:\/\//i, '').replace(/\/.*$/, '');

    // Parse IP:port if together
    if (ip.includes(':')) {
      const parts = ip.split(':');
      ip = parts[0].trim();
      port = parts[1].trim();
    }

    if (!ip) {
      setErrorMsg('No IP address found. Please select a device from the Nearby list or enter IP manually.');
      return;
    }

    if (!port || isNaN(Number(port))) {
      setErrorMsg('Invalid port number.');
      return;
    }

    if (!code || code.length < 4) {
      setErrorMsg('Please enter the 6-digit code shown on the other device.');
      return;
    }

    setIsPairingConnecting(true);

    try {
      const peerAddr = `${ip}:${port}`;
      const res = await initiatePairing(peerAddr, 'code', code);
      setSasConfirmation(res);
      onPairingCompleted(res.peer);
    } catch (err: any) {
      const rawMsg = typeof err === 'string' ? err : err?.message || 'Pairing failed';
      if (/failed to lookup address|no information associated/i.test(rawMsg)) {
        setErrorMsg(`Could not reach "${ip}". Make sure both devices are on the same Wi-Fi.`);
      } else if (/connection refused/i.test(rawMsg)) {
        setErrorMsg(`Connection refused. Ensure the other device accepted pairing and is in pairing mode.`);
      } else {
        setErrorMsg(`Pairing failed: ${rawMsg}`);
      }
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

  // ── SAS Confirmation Screen ───────────────────────────────────────────
  if (sasConfirmation) {
    return (
      <div className="content-container" style={{ maxWidth: '580px', margin: '20px auto', padding: '0 16px', width: '100%' }}>
        <div className="card" style={{ padding: '24px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', color: 'var(--status-online)', marginBottom: '16px' }}>
            <ShieldCheck size={28} />
            <h2 style={{ fontSize: '18px', fontWeight: 600, color: 'var(--text-main)' }}>
              Pairing Succeeded!
            </h2>
          </div>

          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '20px', lineHeight: 1.6 }}>
            Both devices verified the cryptographic handshake. Compare this authentication string with your other device:
          </p>

          <div
            style={{
              backgroundColor: 'var(--bg-surface-subtle)',
              border: '1px solid var(--border-subtle)',
              borderRadius: 'var(--radius-md)',
              padding: '20px',
              textAlign: 'center',
              marginBottom: '20px',
            }}
          >
            <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '1px', color: 'var(--text-muted)', marginBottom: '8px' }}>
              Short Authentication String (SAS)
            </div>
            <div className="mono" style={{ fontSize: '30px', fontWeight: 700, letterSpacing: '4px', marginBottom: '10px' }}>
              {sasConfirmation.sas.numeric}
            </div>
            <div style={{ fontSize: '26px', display: 'flex', justifyContent: 'center', gap: '12px' }}>
              {sasConfirmation.sas.emoji.map((em, idx) => (
                <span key={idx}>{em}</span>
              ))}
            </div>
          </div>

          <div
            style={{
              fontSize: '12px',
              color: 'var(--text-muted)',
              marginBottom: '20px',
              padding: '12px',
              backgroundColor: 'var(--bg-app)',
              borderRadius: 'var(--radius-sm)',
              wordBreak: 'break-all',
            }}
          >
            <div><strong>Device:</strong> {sasConfirmation.peer.device_name}</div>
            <div className="mono" style={{ marginTop: '4px' }}>
              <strong>Fingerprint:</strong> {sasConfirmation.peer.fingerprint}
            </div>
          </div>

          <button
            className="primary"
            style={{ width: '100%', padding: '12px' }}
            onClick={() => setSasConfirmation(null)}
          >
            Done — Device is Trusted
          </button>
        </div>
      </div>
    );
  }

  // ── Main Pair View ────────────────────────────────────────────────────
  return (
    <div className="content-container" style={{ maxWidth: '680px', margin: '20px auto', padding: '0 16px', width: '100%' }}>
      {/* Mode Switcher */}
      <div
        style={{
          display: 'flex',
          borderBottom: '1px solid var(--border-subtle)',
          marginBottom: '20px',
          gap: '12px',
        }}
      >
        <button
          onClick={() => setMode('advertise')}
          style={{
            background: 'none',
            border: 'none',
            borderBottom: mode === 'advertise' ? '2px solid var(--accent)' : '2px solid transparent',
            color: mode === 'advertise' ? 'var(--text-main)' : 'var(--text-muted)',
            fontWeight: 600,
            borderRadius: 0,
            padding: '10px 8px',
          }}
        >
          Show QR & Code
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
            padding: '10px 8px',
          }}
        >
          {isAndroid() ? 'Scan / Enter Code' : 'Enter 6-Digit Code'}
        </button>
      </div>

      {errorMsg && (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
            padding: '12px 14px',
            backgroundColor: 'var(--status-danger-bg)',
            color: 'var(--status-danger)',
            borderRadius: 'var(--radius-sm)',
            fontSize: '13px',
            marginBottom: '16px',
            lineHeight: 1.5,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'flex-start', gap: '8px' }}>
            <AlertCircle size={16} style={{ flexShrink: 0, marginTop: '2px' }} />
            <span>{errorMsg}</span>
          </div>

          {permissionDenied && (
            <button
              className="secondary"
              onClick={handleOpenSettings}
              style={{
                alignSelf: 'flex-start',
                fontSize: '12px',
                padding: '4px 10px',
                marginTop: '4px',
                display: 'inline-flex',
                alignItems: 'center',
                gap: '6px',
              }}
            >
              <SettingsIcon size={13} />
              <span>Open Android Settings</span>
            </button>
          )}
        </div>
      )}

      {mode === 'advertise' ? (
        // ── Show QR & 6-Digit Code (Receiver / Host) ───────────────────
        <div className="card" style={{ padding: '24px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '16px', flexWrap: 'wrap', gap: '8px' }}>
            <div>
              <h3 style={{ fontSize: '16px', fontWeight: 600 }}>Pairing Mode Active</h3>
              <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
                {isAndroid()
                  ? 'Enter this 6-digit code on your laptop to pair.'
                  : 'Scan this QR code with your mobile app, or enter the 6-digit code.'}
              </p>
            </div>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                fontSize: '13px',
                fontWeight: 600,
                color: secondsRemaining < 30 ? 'var(--status-danger)' : 'var(--text-main)',
                backgroundColor: 'var(--bg-surface-subtle)',
                padding: '4px 10px',
                borderRadius: 'var(--radius-sm)',
              }}
            >
              <Clock size={14} />
              <span>{formatTimer(secondsRemaining)}</span>
            </div>
          </div>

          {secondsRemaining <= 0 ? (
            <div style={{ textAlign: 'center', padding: '36px 0' }}>
              <XCircle size={36} color="var(--status-offline)" style={{ margin: '0 auto 12px auto' }} />
              <h4 style={{ fontSize: '15px', fontWeight: 500, marginBottom: '6px' }}>Pairing code expired</h4>
              <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '16px' }}>
                Pairing codes expire after 2 minutes for security.
              </p>
              <button className="primary" onClick={startHostingPairing} disabled={isLoading}>
                Generate New Code
              </button>
            </div>
          ) : payload ? (
            <div style={{ display: 'flex', gap: '24px', alignItems: 'center', flexWrap: 'wrap', justifyContent: 'center' }}>
              {/* QR Code */}
              <div
                style={{
                  padding: '16px',
                  backgroundColor: '#ffffff',
                  borderRadius: 'var(--radius-md)',
                  border: '1px solid var(--border-subtle)',
                  display: 'inline-block',
                }}
              >
                <QRCodeSVG value={JSON.stringify(payload)} size={200} level="M" includeMargin={false} />
              </div>

              {/* Code */}
              <div style={{ flex: '1 1 240px', minWidth: '220px', textAlign: 'center' }}>
                <div style={{ marginBottom: '20px' }}>
                  <div style={{ fontSize: '12px', textTransform: 'uppercase', letterSpacing: '0.8px', color: 'var(--text-muted)', marginBottom: '8px' }}>
                    6-Digit Verification Code
                  </div>
                  <div style={{ display: 'inline-flex', alignItems: 'center', gap: '10px', backgroundColor: 'var(--bg-surface-subtle)', padding: '8px 16px', borderRadius: '8px', border: '1px solid var(--border-subtle)' }}>
                    <span className="mono" style={{ fontSize: '32px', fontWeight: 700, letterSpacing: '4px', color: 'var(--accent)' }}>
                      {payload.code.slice(0, 3)} {payload.code.slice(3, 6)}
                    </span>
                    <button className="secondary" onClick={copyCode} style={{ padding: '6px 8px' }} title="Copy Code">
                      {copiedCode ? <Check size={16} color="var(--status-online)" /> : <Copy size={16} />}
                    </button>
                  </div>
                </div>

                <div style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '20px', lineHeight: 1.5 }}>
                  Waiting for other device to complete pairing...
                </div>

                <button className="danger" onClick={handleCancel} style={{ margin: '0 auto' }}>
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
        // ── Connect Mode (Initiator) ───────────────────────────────────
        <div className="card" style={{ padding: '24px' }}>
          {initialTargetDevice && (
            <div
              style={{
                backgroundColor: 'var(--accent-subtle)',
                border: '1px solid var(--accent)',
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '20px',
                display: 'flex',
                alignItems: 'center',
                gap: '10px',
              }}
            >
              <div style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: 'var(--accent)',
              }} />
              <span style={{ fontSize: '14px', fontWeight: 600 }}>
                Pairing with {initialTargetDevice.device_name}
              </span>
            </div>
          )}

          {/* Android camera QR scanner button (Mobile only) */}
          {isAndroid() && (
            <div style={{ marginBottom: '20px' }}>
              <button
                className="primary"
                style={{
                  width: '100%',
                  padding: '14px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '8px',
                  fontSize: '15px',
                  fontWeight: 600,
                  borderRadius: '8px',
                }}
                onClick={handleScanQr}
                disabled={isScanning || isPairingConnecting}
              >
                <Camera size={20} />
                <span>{isScanning ? 'Opening Camera...' : 'Scan QR Code on Laptop'}</span>
              </button>

              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', margin: '20px 0 16px 0' }}>
                <div style={{ flex: 1, height: '1px', backgroundColor: 'var(--border-subtle)' }} />
                <span style={{ fontSize: '12px', color: 'var(--text-muted)', fontWeight: 500 }}>
                  or enter 6-digit code
                </span>
                <div style={{ flex: 1, height: '1px', backgroundColor: 'var(--border-subtle)' }} />
              </div>
            </div>
          )}

          {/* Code input form */}
          <form onSubmit={handleConnectPeer}>
            <div style={{ marginBottom: '20px', textAlign: 'center' }}>
              <label style={{ display: 'block', fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>
                Enter the 6-Digit Code
              </label>
              <input
                type="text"
                placeholder="• • • • • •"
                maxLength={7}
                autoFocus
                value={secretOrCode}
                onChange={(e) => setSecretOrCode(e.target.value)}
                style={{
                  width: '100%',
                  maxWidth: '280px',
                  padding: '12px',
                  fontSize: '26px',
                  letterSpacing: '6px',
                  textAlign: 'center',
                  fontWeight: 700,
                  margin: '0 auto',
                  borderRadius: '8px',
                }}
                required
              />
              <p style={{ fontSize: '12px', color: 'var(--text-muted)', marginTop: '6px' }}>
                Look at the 6-digit code displayed on the other device.
              </p>
            </div>

            {/* Collapsible manual IP/Port for edge cases */}
            {!initialTargetDevice && (
              <div style={{ marginBottom: '16px' }}>
                <button
                  type="button"
                  onClick={() => setShowManualAddress(!showManualAddress)}
                  style={{
                    background: 'none',
                    border: 'none',
                    color: 'var(--text-muted)',
                    fontSize: '12px',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '4px',
                    margin: '0 auto',
                    padding: '4px 8px',
                    cursor: 'pointer',
                  }}
                >
                  <span>Manual IP & Port</span>
                  {showManualAddress ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                </button>

                {showManualAddress && (
                  <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: '8px', marginTop: '8px' }}>
                    <input
                      type="text"
                      placeholder="IP Address (e.g. 192.168.1.250)"
                      value={targetIp}
                      onChange={(e) => setTargetIp(e.target.value)}
                      style={{ padding: '8px', fontSize: '13px' }}
                    />
                    <input
                      type="text"
                      placeholder="Port"
                      value={targetPort}
                      onChange={(e) => setTargetPort(e.target.value)}
                      style={{ padding: '8px', fontSize: '13px' }}
                    />
                  </div>
                )}
              </div>
            )}

            <button
              type="submit"
              className="primary"
              disabled={isPairingConnecting || !secretOrCode.trim()}
              style={{ width: '100%', padding: '12px', fontSize: '15px', fontWeight: 600, borderRadius: '8px' }}
            >
              {isPairingConnecting ? (
                'Pairing & Verifying...'
              ) : (
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}>
                  <span>Pair Device</span>
                  <ArrowRight size={16} />
                </div>
              )}
            </button>
          </form>
        </div>
      )}
    </div>
  );
};
