import React, { useState } from 'react';
import { Copy, Check, Shield } from 'lucide-react';
import type { IdentityDto } from '../types';
import { updateDeviceName } from '../api';

interface SettingsViewProps {
  identity: IdentityDto | null;
  onIdentityUpdated: () => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  identity,
  onIdentityUpdated,
}) => {
  const [name, setName] = useState<string>(identity?.device_name || '');
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [savedSuccess, setSavedSuccess] = useState<boolean>(false);
  const [copiedFp, setCopiedFp] = useState<boolean>(false);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setIsSaving(true);
    try {
      await updateDeviceName(name.trim());
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2000);
      onIdentityUpdated();
    } catch (err) {
      console.error(err);
    } finally {
      setIsSaving(false);
    }
  };

  const copyFingerprint = () => {
    if (identity?.fingerprint) {
      navigator.clipboard.writeText(identity.fingerprint);
      setCopiedFp(true);
      setTimeout(() => setCopiedFp(false), 2000);
    }
  };

  return (
    <div className="content-container" style={{ maxWidth: '720px', margin: '24px auto', padding: '0 20px', width: '100%' }}>
      <div style={{ marginBottom: '24px' }}>
        <h2 style={{ fontSize: '18px', fontWeight: 600 }}>Device Settings</h2>
        <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
          Identity keys, device name, and security configuration
        </p>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        {/* Device Name Card */}
        <div className="card">
          <h3 style={{ fontSize: '14px', fontWeight: 600, marginBottom: '4px' }}>
            Device Friendly Name
          </h3>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '14px' }}>
            This name is broadcast over the local network via mDNS to nearby peers.
          </p>

          <form onSubmit={handleSave} style={{ display: 'flex', gap: '8px' }}>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              style={{ flex: 1 }}
              placeholder="e.g. Alice's ThinkPad"
              required
            />
            <button type="submit" className="primary" disabled={isSaving}>
              {savedSuccess ? 'Saved!' : isSaving ? 'Saving...' : 'Update Name'}
            </button>
          </form>
        </div>

        {/* Identity & Keys Card */}
        <div className="card">
          <h3 style={{ fontSize: '14px', fontWeight: 600, marginBottom: '4px' }}>
            Cryptographic Identity
          </h3>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginBottom: '16px' }}>
            Long-term identity keypair generated on first launch and protected by the OS secure store.
          </p>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <div>
              <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.8px', color: 'var(--text-muted)', marginBottom: '4px' }}>
                Device ID (UUIDv4)
              </div>
              <div className="mono" style={{
                backgroundColor: 'var(--bg-surface-subtle)',
                padding: '8px 12px',
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--border-subtle)',
              }}>
                {identity?.device_id || 'Loading...'}
              </div>
            </div>

            <div>
              <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.8px', color: 'var(--text-muted)', marginBottom: '4px' }}>
                Ed25519 Public Key Fingerprint (SHA-256)
              </div>
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <div className="mono" style={{
                  flex: 1,
                  backgroundColor: 'var(--bg-surface-subtle)',
                  padding: '8px 12px',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border-subtle)',
                  wordBreak: 'break-all',
                }}>
                  {identity?.fingerprint || 'Loading...'}
                </div>
                <button className="secondary" onClick={copyFingerprint} style={{ padding: '8px' }}>
                  {copiedFp ? <Check size={14} color="var(--status-online)" /> : <Copy size={14} />}
                </button>
              </div>
            </div>

            <div>
              <div style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.8px', color: 'var(--text-muted)', marginBottom: '4px' }}>
                Noise X25519 Public Key
              </div>
              <div className="mono" style={{
                backgroundColor: 'var(--bg-surface-subtle)',
                padding: '8px 12px',
                borderRadius: 'var(--radius-sm)',
                border: '1px solid var(--border-subtle)',
                wordBreak: 'break-all',
                fontSize: '11px',
              }}>
                {identity?.noise_public_key_hex || 'Loading...'}
              </div>
            </div>
          </div>
        </div>

        {/* Security Architecture Card */}
        <div className="card">
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <Shield size={16} color="var(--accent)" />
            <h3 style={{ fontSize: '14px', fontWeight: 600 }}>Security Architecture</h3>
          </div>

          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
            gap: '12px',
            fontSize: '12px',
            color: 'var(--text-muted)',
            marginTop: '12px',
          }}>
            <div style={{ padding: '10px', backgroundColor: 'var(--bg-surface-subtle)', borderRadius: 'var(--radius-sm)' }}>
              <strong>Secure Key Storage:</strong> Windows DPAPI (<code className="mono">CryptProtectData</code>). Private keys are encrypted at rest with user login session keys.
            </div>
            <div style={{ padding: '10px', backgroundColor: 'var(--bg-surface-subtle)', borderRadius: 'var(--radius-sm)' }}>
              <strong>Pairing Protocol:</strong> <code className="mono">Noise_XXpsk3</code> with single-use 256-bit secret and 6-digit numeric fallback.
            </div>
            <div style={{ padding: '10px', backgroundColor: 'var(--bg-surface-subtle)', borderRadius: 'var(--radius-sm)' }}>
              <strong>Session Security:</strong> <code className="mono">Noise_XX</code> mutual authentication with ChaCha20Poly1305 AEAD framing.
            </div>
            <div style={{ padding: '10px', backgroundColor: 'var(--bg-surface-subtle)', borderRadius: 'var(--radius-sm)' }}>
              <strong>Zero Leakage:</strong> mDNS TXT records contain only device metadata and public fingerprints. Secrets are never broadcast.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
