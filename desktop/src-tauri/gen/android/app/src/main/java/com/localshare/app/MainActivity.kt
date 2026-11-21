package com.localshare.app

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Bundle

class MainActivity : TauriActivity() {
    private var multicastLock: WifiManager.MulticastLock? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        // Acquire multicast lock BEFORE Tauri/Rust initializes so mDNS (mdns-sd) can bind
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
        if (wifi != null) {
            multicastLock = wifi.createMulticastLock("localshare_mdns").also { lock ->
                lock.setReferenceCounted(true)
                lock.acquire()
            }
        }
        super.onCreate(savedInstanceState)
    }

    override fun onDestroy() {
        multicastLock?.let { if (it.isHeld) it.release() }
        super.onDestroy()
    }
}
