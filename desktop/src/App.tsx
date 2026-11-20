import React, { useState, useEffect } from 'react';
import { Navbar } from './components/Navbar';
import { HomeView } from './components/HomeView';
import { PairView } from './components/PairView';
import { TrustedView } from './components/TrustedView';
import { SettingsView } from './components/SettingsView';
import {
  getIdentity,
  getDiscoveredDevices,
  listTrustedDevices,
  startDiscovery,
  onDiscoveredDevices,
  onServiceEvent,
} from './api';
import type { DiscoveredDevice, IdentityDto, TrustedDevice } from './types';

export const App: React.FC = () => {
  const [currentTab, setCurrentTab] = useState<'home' | 'pair' | 'trusted' | 'settings'>('home');
  const [identity, setIdentity] = useState<IdentityDto | null>(null);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const [trustedDevices, setTrustedDevices] = useState<TrustedDevice[]>([]);
  const [isScanning, setIsScanning] = useState<boolean>(false);
  const [scanCountdown, setScanCountdown] = useState<number>(0);
  const [targetPairDevice, setTargetPairDevice] = useState<DiscoveredDevice | null>(null);

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
        } else if ('DiscoveredDevicesUpdated' in event) {
          setDiscoveredDevices(event.DiscoveredDevicesUpdated);
        }
      }
    }).then((un) => {
      unlistenSvc = un;
    });

    return () => {
      if (unlistenDisc) unlistenDisc();
      if (unlistenSvc) unlistenSvc();
    };
  }, []);

  const handleStartPairWith = (device: DiscoveredDevice) => {
    setTargetPairDevice(device);
    setCurrentTab('pair');
  };

  const handlePairingCompleted = (_peer: TrustedDevice) => {
    refreshTrustedDevices();
  };

  return (
    <div style={{ minHeight: '100vh', display: 'flex', flexDirection: 'column' }}>
      <Navbar
        currentTab={currentTab}
        onSelectTab={(tab) => {
          if (tab !== 'pair') setTargetPairDevice(null);
          setCurrentTab(tab);
        }}
        identity={identity}
        discoveredCount={discoveredDevices.length}
        trustedCount={trustedDevices.length}
        isScanning={isScanning}
        onConnect={handleConnect}
      />

      <main style={{ flex: 1 }}>
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
            onPairingCompleted={handlePairingCompleted}
          />
        )}

        {currentTab === 'trusted' && (
          <TrustedView
            trustedDevices={trustedDevices}
            onRefresh={refreshTrustedDevices}
            onNavigatePair={() => {
              setTargetPairDevice(null);
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
    </div>
  );
};

export default App;
