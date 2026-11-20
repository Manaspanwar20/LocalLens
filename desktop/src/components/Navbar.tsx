import React from 'react';
import { Wifi, ShieldCheck, QrCode, Settings, RefreshCw } from 'lucide-react';
import type { IdentityDto } from '../types';

interface NavbarProps {
  currentTab: 'home' | 'pair' | 'trusted' | 'settings';
  onSelectTab: (tab: 'home' | 'pair' | 'trusted' | 'settings') => void;
  identity: IdentityDto | null;
  discoveredCount: number;
  trustedCount: number;
  isScanning: boolean;
  onConnect: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  currentTab,
  onSelectTab,
  identity,
  discoveredCount,
  trustedCount,
  isScanning,
  onConnect,
}) => {
  return (
    <header style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '12px 20px',
      borderBottom: '1px solid var(--border-subtle)',
      backgroundColor: 'var(--bg-surface)',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <div style={{
            width: '10px',
            height: '10px',
            borderRadius: '50%',
            backgroundColor: 'var(--status-online)',
          }} />
          <span style={{ fontWeight: 600, fontSize: '15px', letterSpacing: '-0.2px' }}>
            LocalShare
          </span>
        </div>

        <nav style={{ display: 'flex', gap: '4px' }}>
          <button
            className={currentTab === 'home' ? 'primary' : 'secondary'}
            onClick={() => onSelectTab('home')}
            style={{ padding: '6px 12px' }}
          >
            <Wifi size={15} />
            <span>Nearby</span>
            {discoveredCount > 0 && (
              <span style={{
                fontSize: '11px',
                padding: '1px 6px',
                borderRadius: '10px',
                backgroundColor: currentTab === 'home' ? 'rgba(255,255,255,0.2)' : 'var(--bg-surface-subtle)',
                color: currentTab === 'home' ? '#ffffff' : 'var(--text-muted)',
              }}>
                {discoveredCount}
              </span>
            )}
          </button>

          <button
            className={currentTab === 'pair' ? 'primary' : 'secondary'}
            onClick={() => onSelectTab('pair')}
            style={{ padding: '6px 12px' }}
          >
            <QrCode size={15} />
            <span>Pair Device</span>
          </button>

          <button
            className={currentTab === 'trusted' ? 'primary' : 'secondary'}
            onClick={() => onSelectTab('trusted')}
            style={{ padding: '6px 12px' }}
          >
            <ShieldCheck size={15} />
            <span>Trusted</span>
            {trustedCount > 0 && (
              <span style={{
                fontSize: '11px',
                padding: '1px 6px',
                borderRadius: '10px',
                backgroundColor: currentTab === 'trusted' ? 'rgba(255,255,255,0.2)' : 'var(--bg-surface-subtle)',
                color: currentTab === 'trusted' ? '#ffffff' : 'var(--text-muted)',
              }}>
                {trustedCount}
              </span>
            )}
          </button>

          <button
            className={currentTab === 'settings' ? 'primary' : 'secondary'}
            onClick={() => onSelectTab('settings')}
            style={{ padding: '6px 12px' }}
          >
            <Settings size={15} />
            <span>Settings</span>
          </button>
        </nav>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
        {identity && (
          <div style={{ textAlign: 'right' }}>
            <div style={{ fontSize: '13px', fontWeight: 500 }}>{identity.device_name}</div>
            <div className="mono" style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
              {identity.fingerprint.slice(0, 8)}...{identity.fingerprint.slice(-6)}
            </div>
          </div>
        )}

        <button
          className="primary"
          onClick={onConnect}
          disabled={isScanning}
          style={{ minWidth: '100px' }}
        >
          <RefreshCw size={14} className={isScanning ? 'spin' : ''} style={{
            animation: isScanning ? 'spin 1s linear infinite' : 'none'
          }} />
          <span>{isScanning ? 'Scanning...' : 'Connect'}</span>
        </button>
      </div>

      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </header>
  );
};
