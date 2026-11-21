import React, { useState } from 'react';
import { Shield, Edit2, Trash2, Check, X } from 'lucide-react';
import type { TrustedDevice } from '../types';
import { renameTrustedDevice, revokeTrustedDevice } from '../api';

interface TrustedViewProps {
  trustedDevices: TrustedDevice[];
  onRefresh: () => void;
  onNavigatePair: () => void;
}

export const TrustedView: React.FC<TrustedViewProps> = ({
  trustedDevices,
  onRefresh,
  onNavigatePair,
}) => {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState<string>('');
  const [confirmRevokeId, setConfirmRevokeId] = useState<string | null>(null);

  const startEdit = (dev: TrustedDevice) => {
    setEditingId(dev.device_id);
    setEditName(dev.device_name);
  };

  const saveEdit = async (deviceId: string) => {
    if (!editName.trim()) return;
    try {
      await renameTrustedDevice(deviceId, editName.trim());
      setEditingId(null);
      onRefresh();
    } catch (err) {
      console.error(err);
    }
  };

  const handleRevoke = async (deviceId: string) => {
    try {
      await revokeTrustedDevice(deviceId);
      setConfirmRevokeId(null);
      onRefresh();
    } catch (err) {
      console.error(err);
    }
  };

  const formatDate = (timestampSecs: number) => {
    return new Date(timestampSecs * 1000).toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  return (
    <div className="content-container" style={{ maxWidth: '860px', margin: '0 auto', padding: '24px 20px', width: '100%' }}>
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        marginBottom: '20px',
      }}>
        <div>
          <h2 style={{ fontSize: '18px', fontWeight: 600 }}>Trusted Devices</h2>
          <p style={{ color: 'var(--text-muted)', fontSize: '13px', marginTop: '2px' }}>
            Devices whose public keys are pinned in the OS secure store (Windows DPAPI)
          </p>
        </div>

        <button className="primary" onClick={onNavigatePair}>
          Pair New Device
        </button>
      </div>

      {trustedDevices.length === 0 ? (
        <div className="card" style={{
          textAlign: 'center',
          padding: '48px 24px',
          borderStyle: 'dashed',
        }}>
          <Shield size={36} color="var(--border-strong)" style={{ margin: '0 auto 12px auto' }} />
          <h3 style={{ fontSize: '15px', fontWeight: 500, marginBottom: '6px' }}>
            No trusted devices yet
          </h3>
          <p style={{
            color: 'var(--text-muted)',
            fontSize: '13px',
            maxWidth: '440px',
            margin: '0 auto 16px auto',
            lineHeight: 1.6,
          }}>
            When you pair with another phone or desktop via QR code or numeric code, its cryptographic identity key is pinned here.
          </p>
          <button className="secondary" onClick={onNavigatePair}>
            Pair a Device Now
          </button>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {trustedDevices.map((dev) => {
            const isEditing = editingId === dev.device_id;
            const isRevoking = confirmRevokeId === dev.device_id;

            return (
              <div
                key={dev.device_id}
                className="card"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '16px 20px',
                }}
              >
                <div style={{ flex: 1, marginRight: '16px' }}>
                  {isEditing ? (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '6px' }}>
                      <input
                        type="text"
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        autoFocus
                      />
                      <button className="primary" onClick={() => saveEdit(dev.device_id)} style={{ padding: '4px 8px' }}>
                        <Check size={14} />
                      </button>
                      <button className="secondary" onClick={() => setEditingId(null)} style={{ padding: '4px 8px' }}>
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                      <span style={{ fontWeight: 600, fontSize: '15px' }}>{dev.device_name}</span>
                      <button
                        onClick={() => startEdit(dev)}
                        style={{ background: 'none', border: 'none', padding: '2px', color: 'var(--text-muted)' }}
                        title="Rename"
                      >
                        <Edit2 size={13} />
                      </button>
                    </div>
                  )}

                  <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '2px',
                    fontSize: '12px',
                    color: 'var(--text-muted)',
                  }}>
                    <div>
                      <span>Device ID: </span>
                      <span className="mono">{dev.device_id}</span>
                    </div>
                    <div>
                      <span>Key Fingerprint: </span>
                      <span className="mono">{dev.fingerprint}</span>
                    </div>
                    <div>
                      <span>Paired on: </span>
                      <span>{formatDate(dev.paired_at)}</span>
                    </div>
                  </div>
                </div>

                <div>
                  {isRevoking ? (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <span style={{ fontSize: '12px', color: 'var(--status-danger)' }}>Revoke key?</span>
                      <button
                        className="danger"
                        onClick={() => handleRevoke(dev.device_id)}
                        style={{ padding: '4px 8px' }}
                      >
                        Yes, Revoke
                      </button>
                      <button
                        className="secondary"
                        onClick={() => setConfirmRevokeId(null)}
                        style={{ padding: '4px 8px' }}
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      className="danger"
                      onClick={() => setConfirmRevokeId(dev.device_id)}
                      title="Revoke device trust"
                    >
                      <Trash2 size={14} />
                      <span>Revoke</span>
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
