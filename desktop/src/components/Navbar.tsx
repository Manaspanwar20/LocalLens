import React, { useState, useEffect } from 'react';
import { Wifi, ShieldCheck, QrCode, Settings, RefreshCw } from 'lucide-react';
import type { IdentityDto } from '../types';

const checkIsMobile = (): boolean => {
  if (typeof window === 'undefined') return false;
  return /android|iphone|ipad|ipod/i.test(navigator.userAgent) || window.innerWidth <= 768;
};

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
  const [isMobile, setIsMobile] = useState<boolean>(checkIsMobile());

  useEffect(() => {
    const handleResize = () => {
      setIsMobile(checkIsMobile());
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);
  return (
    <>
      {/* ── Top Header (Responsive) ────────────────────────────────── */}
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '10px 16px',
          borderBottom: '1px solid var(--border-subtle)',
          backgroundColor: 'var(--bg-surface)',
          position: 'sticky',
          top: 0,
          zIndex: 50,
          width: '100%',
        }}
      >
        {/* Brand */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <div
            style={{
              width: '9px',
              height: '9px',
              borderRadius: '50%',
              backgroundColor: 'var(--status-online)',
            }}
          />
          <span style={{ fontWeight: 600, fontSize: '15px', letterSpacing: '-0.2px' }}>
            LocalShare
          </span>
          {identity && (
            <span
              className="badge desktop-only"
              style={{ fontSize: '11px', maxWidth: '140px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              {identity.device_name}
            </span>
          )}
        </div>

        {/* Desktop Navigation Tabs */}
        {!isMobile && (
          <nav className="desktop-only" style={{ gap: '4px' }}>
            <button
              className={currentTab === 'home' ? 'primary' : 'secondary'}
              onClick={() => onSelectTab('home')}
              style={{ padding: '6px 12px' }}
            >
              <Wifi size={15} />
              <span>Nearby</span>
              {discoveredCount > 0 && (
                <span
                  style={{
                    fontSize: '11px',
                    padding: '1px 6px',
                    borderRadius: '10px',
                    backgroundColor: currentTab === 'home' ? 'rgba(255,255,255,0.2)' : 'var(--bg-surface-subtle)',
                    color: currentTab === 'home' ? '#ffffff' : 'var(--text-muted)',
                  }}
                >
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
                <span
                  style={{
                    fontSize: '11px',
                    padding: '1px 6px',
                    borderRadius: '10px',
                    backgroundColor: currentTab === 'trusted' ? 'rgba(255,255,255,0.2)' : 'var(--bg-surface-subtle)',
                    color: currentTab === 'trusted' ? '#ffffff' : 'var(--text-muted)',
                  }}
                >
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
        )}

        {/* Action button */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            className="primary"
            onClick={onConnect}
            disabled={isScanning}
            style={{
              padding: '6px 12px',
              fontSize: '12px',
            }}
          >
            <RefreshCw
              size={13}
              style={{
                animation: isScanning ? 'spin 1s linear infinite' : 'none',
              }}
            />
            <span>{isScanning ? 'Scanning...' : 'Scan Wi-Fi'}</span>
          </button>
        </div>
      </header>

      {/* ── Mobile Bottom Navigation Bar (Fixed) ────────────────────── */}
      {isMobile && (
        <nav
          className="mobile-only"
          style={{
            position: 'fixed',
            bottom: 0,
            left: 0,
            right: 0,
            height: '60px',
            backgroundColor: 'var(--bg-surface)',
            borderTop: '1px solid var(--border-subtle)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-around',
            zIndex: 100,
            paddingBottom: 'env(safe-area-inset-bottom, 0px)',
          }}
        >
        <button
          onClick={() => onSelectTab('home')}
          style={{
            flex: 1,
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '3px',
            background: 'none',
            border: 'none',
            borderRadius: 0,
            padding: 0,
            color: currentTab === 'home' ? 'var(--accent)' : 'var(--text-muted)',
            fontSize: '11px',
            position: 'relative',
          }}
        >
          <Wifi size={18} />
          <span>Nearby</span>
          {discoveredCount > 0 && (
            <span
              style={{
                position: 'absolute',
                top: '6px',
                right: 'calc(50% - 18px)',
                backgroundColor: 'var(--accent)',
                color: '#fff',
                fontSize: '9px',
                padding: '1px 4px',
                borderRadius: '8px',
              }}
            >
              {discoveredCount}
            </span>
          )}
        </button>

        <button
          onClick={() => onSelectTab('pair')}
          style={{
            flex: 1,
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '3px',
            background: 'none',
            border: 'none',
            borderRadius: 0,
            padding: 0,
            color: currentTab === 'pair' ? 'var(--accent)' : 'var(--text-muted)',
            fontSize: '11px',
          }}
        >
          <QrCode size={18} />
          <span>Pair</span>
        </button>

        <button
          onClick={() => onSelectTab('trusted')}
          style={{
            flex: 1,
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '3px',
            background: 'none',
            border: 'none',
            borderRadius: 0,
            padding: 0,
            color: currentTab === 'trusted' ? 'var(--accent)' : 'var(--text-muted)',
            fontSize: '11px',
            position: 'relative',
          }}
        >
          <ShieldCheck size={18} />
          <span>Trusted</span>
          {trustedCount > 0 && (
            <span
              style={{
                position: 'absolute',
                top: '6px',
                right: 'calc(50% - 18px)',
                backgroundColor: 'var(--accent)',
                color: '#fff',
                fontSize: '9px',
                padding: '1px 4px',
                borderRadius: '8px',
              }}
            >
              {trustedCount}
            </span>
          )}
        </button>

        <button
          onClick={() => onSelectTab('settings')}
          style={{
            flex: 1,
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '3px',
            background: 'none',
            border: 'none',
            borderRadius: 0,
            padding: 0,
            color: currentTab === 'settings' ? 'var(--accent)' : 'var(--text-muted)',
            fontSize: '11px',
          }}
        >
          <Settings size={18} />
          <span>Settings</span>
        </button>
      </nav>
      )}

      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </>
  );
};
