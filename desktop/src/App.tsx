import React, { useState, useEffect } from 'react';
import { Navbar } from './components/Navbar';
import { HomeView } from './components/HomeView';
import { PairView } from './components/PairView';
import { TrustedView } from './components/TrustedView';
import { SettingsView } from './components/SettingsView';
import { PairRequestModal } from './components/PairRequestModal';
import {
  getIdentity,
  getDiscoveredDevices,
  listTrustedDevices,
  startDiscovery,
  onDiscoveredDevices,
  onServiceEvent,
  respondPairRequest,
} from './api';
import type { DiscoveredDevice, IdentityDto, PairResponseMessage, QrPairingPayload, TrustedDevice } from './types';

export const App: React.FC = () => {
  const [currentTab, setCurrentTab] = useState<'home' | 'pair' | 'trusted' | 'settings'>('home');
  const [identity, setIdentity] = useState<IdentityDto | null>(null);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const [trustedDevices, setTrustedDevices] = useState<TrustedDevice[]>([]);
  const [isScanning, setIsScanning] = useState<boolean>(false);
  const [scanCountdown, setScanCountdown] = useState<number>(0);
  const [targetPairDevice, setTargetPairDevice] = useState<DiscoveredDevice | null>(null);
  const [activePairPayload, setActivePairPayload] = useState<QrPairingPayload | null>(null);

  // Incoming pairing request from remote device
  const [incomingPairRequest, setIncomingPairRequest] = useState<{
    from_device_id: string;
    from_device_name: string;
    from_fingerprint: string;
  } | null>(null);
  const [isRespondingPairRequest, setIsRespondingPairRequest] = useState<boolean>(false);

  const refreshIdentity = async () => {
    try {
      const id = await getIdentity();
      setIdentity(id);
    } catch (err) {
      console.error('Failed to get identity:', err);
    }
  };

  const refreshTrustedDevices = async () => {
    try {
      const list = await listTrustedDevices();
      setTrustedDevices(list);
    } catch (err) {
      console.error('Failed to list trusted devices:', err);
    }
  };

  const refreshDiscovered = async () => {
    try {
      const list = await getDiscoveredDevices();
      setDiscoveredDevices(list);
    } catch (err) {
      console.error('Failed to get discovered devices:', err);
    }
  };

  const handleConnect = async () => {
    if (isScanning) return;
    setIsScanning(true);
    setScanCountdown(10);
    try {
      await startDiscovery();
    } catch (err) {
      console.error('Failed to start discovery scan:', err);
      setIsScanning(false);
    }
  };

  // Countdown timer for scanning
  useEffect(() => {
    if (!isScanning) return;
    const interval = setInterval(() => {
      setScanCountdown((prev) => {
        if (prev <= 1) {
          setIsScanning(false);
          clearInterval(interval);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => clearInterval(interval);
  }, [isScanning]);

  // Initial load & event subscriptions
  useEffect(() => {
    refreshIdentity();
    refreshTrustedDevices();
    refreshDiscovered();

    let unlistenDisc: (() => void) | undefined;
    let unlistenSvc: (() => void) | undefined;

    onDiscoveredDevices((devices) => {
      setDiscoveredDevices(devices);
    }).then((un) => {
      unlistenDisc = un;
    });

    onServiceEvent((event) => {
      if (event && typeof event === 'object') {
        if ('PairingSuccess' in event) {
          refreshTrustedDevices();
        } else if ('TrustStoreUpdated' in event) {
          setTrustedDevices((event as any).TrustStoreUpdated);
        } else if ('DiscoveredDevicesUpdated' in event) {
          setDiscoveredDevices(event.DiscoveredDevicesUpdated);
        } else if ('IncomingPairRequest' in event) {
          setIncomingPairRequest((event as any).IncomingPairRequest);
        }
      }
    }).then((un) => {
      unlistenSvc = un;
    });

    // Also periodic background refresh for live status updates
    const syncInterval = setInterval(() => {
      refreshTrustedDevices();
      refreshDiscovered();
    }, 3000);

    return () => {
      if (unlistenDisc) unlistenDisc();
      if (unlistenSvc) unlistenSvc();
      clearInterval(syncInterval);
    };
  }, []);

  const handleAllowPairRequest = async () => {
    if (!incomingPairRequest) return;
    setIsRespondingPairRequest(true);
    try {
      const payload = await respondPairRequest(incomingPairRequest.from_device_id, true);
      if (payload) {
        setActivePairPayload(payload);
        setTargetPairDevice(null);
        setCurrentTab('pair');
      }
    } catch (err) {
      console.error('Failed to respond to pair request:', err);
    } finally {
      setIsRespondingPairRequest(false);
      setIncomingPairRequest(null);
    }
  };

  const handleDenyPairRequest = async () => {
    if (!incomingPairRequest) return;
    try {
      await respondPairRequest(incomingPairRequest.from_device_id, false);
    } catch (err) {
      console.error('Failed to deny pair request:', err);
    } finally {
      setIncomingPairRequest(null);
    }
  };

  const handleStartPairWith = (device: DiscoveredDevice, _pairResponse?: PairResponseMessage) => {
    setTargetPairDevice(device);
    setActivePairPayload(null);
    setCurrentTab('pair');
  };

  const handlePairingCompleted = (_peer: TrustedDevice) => {
    refreshTrustedDevices();
    refreshDiscovered();
  };

  return (
    <div style={{ minHeight: '100vh', display: 'flex', flexDirection: 'column' }}>
      <Navbar
        currentTab={currentTab}
        onSelectTab={(tab) => {
          if (tab !== 'pair') {
            setTargetPairDevice(null);
            setActivePairPayload(null);
          }
          setCurrentTab(tab);
        }}
        identity={identity}
        discoveredCount={discoveredDevices.length}
        trustedCount={trustedDevices.length}
        isScanning={isScanning}
        onConnect={handleConnect}
      />

      <main className="app-main" style={{ flex: 1, width: '100%' }}>
        {currentTab === 'home' && (
          <HomeView
            devices={discoveredDevices}
            trustedDevices={trustedDevices}
            isScanning={isScanning}
            scanCountdown={scanCountdown}
            onConnect={handleConnect}
            onStartPairWith={handleStartPairWith}
          />
        )}

        {currentTab === 'pair' && (
          <PairView
            initialTargetDevice={targetPairDevice}
            initialPayload={activePairPayload}
            onPairingCompleted={handlePairingCompleted}
            onCancel={() => {
              setActivePairPayload(null);
              setTargetPairDevice(null);
              setCurrentTab('home');
            }}
          />
        )}

        {currentTab === 'trusted' && (
          <TrustedView
            trustedDevices={trustedDevices}
            onRefresh={refreshTrustedDevices}
            onNavigatePair={() => {
              setTargetPairDevice(null);
              setActivePairPayload(null);
              setCurrentTab('pair');
            }}
          />
        )}

        {currentTab === 'settings' && (
          <SettingsView
            identity={identity}
            onIdentityUpdated={refreshIdentity}
          />
        )}
      </main>

      {/* AirDrop style Incoming Pairing Request Modal */}
      <PairRequestModal
        request={incomingPairRequest}
        onAllow={handleAllowPairRequest}
        onDeny={handleDenyPairRequest}
        isResponding={isRespondingPairRequest}
      />
    </div>
  );
};

export default App;
