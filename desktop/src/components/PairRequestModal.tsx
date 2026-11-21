import React from 'react';
import { Smartphone, Check, X } from 'lucide-react';

interface PairRequestModalProps {
  request: {
    from_device_id: string;
    from_device_name: string;
    from_fingerprint: string;
  } | null;
  onAllow: () => void;
  onDeny: () => void;
  isResponding?: boolean;
}

export const PairRequestModal: React.FC<PairRequestModalProps> = ({
  request,
  onAllow,
  onDeny,
  isResponding = false,
}) => {
  if (!request) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.65)',
        backdropFilter: 'blur(4px)',
        zIndex: 9999,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '16px',
        animation: 'fadeIn 0.2s ease-out',
      }}
    >
      <div
        className="card"
        style={{
          maxWidth: '420px',
          width: '100%',
          backgroundColor: 'var(--bg-surface)',
          borderRadius: '12px',
          padding: '24px',
          boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.4), 0 8px 10px -6px rgba(0, 0, 0, 0.3)',
          border: '1px solid var(--border-strong)',
          textAlign: 'center',
        }}
      >
        {/* Top Icon */}
        <div
          style={{
            width: '56px',
            height: '56px',
            borderRadius: '50%',
            backgroundColor: 'var(--accent-subtle)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            margin: '0 auto 16px auto',
            border: '2px solid var(--accent)',
          }}
        >
          <Smartphone size={28} color="var(--accent)" />
        </div>

        <h3 style={{ fontSize: '18px', fontWeight: 700, marginBottom: '8px' }}>
          Pairing Request
        </h3>

        <p style={{ fontSize: '14px', color: 'var(--text-main)', marginBottom: '12px', lineHeight: 1.5 }}>
          <strong style={{ color: 'var(--accent)', fontSize: '15px' }}>{request.from_device_name}</strong>
          {' '}wants to pair with this device.
        </p>

        <p style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '24px', lineHeight: 1.5 }}>
          Allowing will display your QR code and 6-digit verification code so both devices can establish an encrypted connection.
        </p>

        {/* Buttons */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: '12px',
          }}
        >
          <button
            className="secondary"
            onClick={onDeny}
            disabled={isResponding}
            style={{
              padding: '10px 16px',
              fontSize: '14px',
              fontWeight: 600,
              color: 'var(--status-danger)',
              borderColor: 'var(--border-subtle)',
            }}
          >
            <X size={16} />
            <span>Deny</span>
          </button>

          <button
            className="primary"
            onClick={onAllow}
            disabled={isResponding}
            style={{
              padding: '10px 16px',
              fontSize: '14px',
              fontWeight: 600,
            }}
          >
            <Check size={16} />
            <span>{isResponding ? 'Starting...' : 'Allow'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
