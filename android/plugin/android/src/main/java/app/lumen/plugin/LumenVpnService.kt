package app.lumen.plugin

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import app.lumen.core.lumencore.Lumencore
import app.lumen.core.lumencore.Protector
import java.util.concurrent.Executors

/**
 * The tunnel: a foreground VpnService that hands its TUN descriptor to Xray
 * running in this process. Xray's own sockets are protected, and Lumen's package
 * stays outside the VPN, which is what the desktop `process: self/` rule does.
 */
class LumenVpnService : VpnService() {
    private val worker = Executors.newSingleThreadExecutor()
    private var tunnel: ParcelFileDescriptor? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    private val protector = object : Protector {
        override fun protect(fd: Long): Boolean = this@LumenVpnService.protect(fd.toInt())
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            VpnState.stopping()
            worker.execute { shutdown(null) }
            return START_NOT_STICKY
        }
        // ACTION_START, Always-on (android.net.VpnService) or a sticky restart.
        val plan = VpnPlan.load(this)
        promote(plan?.serverName)
        if (intent?.action != ACTION_START) {
            VpnState.connecting(plan?.serverName)
        }
        worker.execute { connect(plan) }
        return START_STICKY
    }

    override fun onRevoke() {
        // Another VPN took over, or the user revoked consent in system settings.
        worker.execute { shutdown(Failure("VPN_REVOKED", "Android VPN permission was revoked")) }
    }

    override fun onDestroy() {
        worker.execute { shutdown(null, stopService = false) }
        worker.shutdown()
        super.onDestroy()
    }

    private class Failure(val code: String, override val message: String) : Exception(message)

    private fun connect(plan: VpnPlan?) {
        try {
            if (plan == null) throw Failure("NO_CONNECTION_PLAN", "Open Lumen and connect once")
            if (prepare(this) != null) {
                throw Failure("VPN_PERMISSION_DENIED", "Lumen is not allowed to create a VPN")
            }
            val previous = tunnel
            val next = buildTunnel(plan)
                ?: throw Failure("VPN_PERMISSION_DENIED", "Android refused to create the VPN interface")
            Lumencore.setProtector(protector)
            Lumencore.setMemoryLimit(MEMORY_LIMIT_MB)
            val error = Lumencore.start(plan.config, next.fd.toLong(), plan.assetDir)
            // Start closed the previous instance, so its descriptor is free now.
            previous?.closeQuietly()
            if (error.isNotEmpty()) {
                next.closeQuietly()
                tunnel = null
                throw Failure("CORE_START_FAILED", error)
            }
            tunnel = next
            watchNetwork()
            VpnState.connected(plan.serverName)
            notifyStatus(plan.serverName)
        } catch (failure: Failure) {
            Log.w(TAG, "Tunnel failed: ${failure.code}")
            shutdown(failure)
        } catch (error: Exception) {
            Log.e(TAG, "Tunnel failed", error)
            shutdown(Failure("TUN_FAILED", error.message ?: error.javaClass.simpleName))
        }
    }

    private fun buildTunnel(plan: VpnPlan): ParcelFileDescriptor? {
        val builder = Builder()
            .setSession(plan.serverName.ifEmpty { "Lumen" })
            .setMtu(plan.mtu)
            .addAddress(TUN_ADDRESS_V4, 30)
            .addRoute("0.0.0.0", 0)
            .addDnsServer(plan.dns)
            .setBlocking(false)
        // IPv6 always enters the tunnel; without a route it would leave directly.
        builder.addAddress(TUN_ADDRESS_V6, 126).addRoute("::", 0)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            builder.setMetered(false)
        }
        if (plan.includedApps.isNotEmpty()) {
            for (name in plan.includedApps) {
                if (name != packageName) builder.allowQuietly(name, allow = true)
            }
        } else {
            builder.addDisallowedApplication(packageName)
            for (name in plan.excludedApps) {
                if (name != packageName) builder.allowQuietly(name, allow = false)
            }
        }
        openAppIntent()?.let { builder.setConfigureIntent(it) }
        return builder.establish()
    }

    private fun Builder.allowQuietly(name: String, allow: Boolean) {
        try {
            if (allow) addAllowedApplication(name) else addDisallowedApplication(name)
        } catch (_: PackageManager.NameNotFoundException) {
            // An uninstalled app in a rule is not an error.
        }
    }

    /** Keeps Android's view of the VPN's underlying network current (metered state, captive portals). */
    private fun watchNetwork() {
        if (networkCallback != null) return
        val connectivity = getSystemService(ConnectivityManager::class.java) ?: return
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                setUnderlyingNetworks(arrayOf(network))
            }

            override fun onLost(network: Network) {
                setUnderlyingNetworks(null)
            }
        }
        try {
            // Lumen's own package is outside the VPN, so its default network is the physical one.
            connectivity.registerDefaultNetworkCallback(callback)
            networkCallback = callback
        } catch (error: Exception) {
            Log.w(TAG, "Network watch unavailable", error)
        }
    }

    private fun shutdown(failure: Failure?, stopService: Boolean = true) {
        Lumencore.stop()
        tunnel?.closeQuietly()
        tunnel = null
        networkCallback?.let { callback ->
            try {
                getSystemService(ConnectivityManager::class.java)?.unregisterNetworkCallback(callback)
            } catch (_: Exception) {
            }
        }
        networkCallback = null
        if (failure != null) {
            VpnState.failed(failure.code, failure.message)
        } else {
            VpnState.idle()
        }
        if (stopService) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
    }

    private fun promote(serverName: String?) {
        ensureChannel(this)
        val notification = buildNotification(serverName, connected = false)
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
        } else {
            0
        }
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
    }

    private fun notifyStatus(serverName: String?) {
        val manager = getSystemService(NotificationManager::class.java) ?: return
        try {
            manager.notify(NOTIFICATION_ID, buildNotification(serverName, connected = true))
        } catch (_: SecurityException) {
            // POST_NOTIFICATIONS was declined; the service keeps running.
        }
    }

    private fun buildNotification(serverName: String?, connected: Boolean): Notification {
        val stop = PendingIntent.getService(
            this,
            1,
            Intent(this, LumenVpnService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val title = getString(if (connected) R.string.lumen_notification_connected else R.string.lumen_notification_connecting)
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_lumen_status)
            .setContentTitle(title)
            .setContentText(serverName?.takeIf { it.isNotEmpty() } ?: getString(R.string.lumen_app_name))
            .setContentIntent(openAppIntent())
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(connected)
            .setUsesChronometer(connected)
            .setWhen(VpnState.current.startedAt ?: System.currentTimeMillis())
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .addAction(0, getString(R.string.lumen_notification_disconnect), stop)
            .build()
    }

    private fun openAppIntent(): PendingIntent? {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return null
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        return PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    private fun ParcelFileDescriptor.closeQuietly() {
        try {
            close()
        } catch (_: Exception) {
        }
    }

    companion object {
        private const val TAG = "LumenVpn"
        const val ACTION_START = "app.lumen.plugin.action.START"
        const val ACTION_STOP = "app.lumen.plugin.action.STOP"
        private const val CHANNEL_ID = "lumen_vpn"
        private const val NOTIFICATION_ID = 0x4c55
        private const val TUN_ADDRESS_V4 = "172.31.255.1"
        private const val TUN_ADDRESS_V6 = "fd42:6c75:6d65::1"
        /** Large geosite categories keep ~120 MB live; see GO_RUNTIME_ENV on desktop. */
        private const val MEMORY_LIMIT_MB = 384L

        fun start(context: Context) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, LumenVpnService::class.java).setAction(ACTION_START),
            )
        }

        fun stop(context: Context) {
            try {
                context.startService(Intent(context, LumenVpnService::class.java).setAction(ACTION_STOP))
            } catch (error: IllegalStateException) {
                // Not running and not allowed to start from the background: nothing to stop.
                VpnState.idle()
            }
        }

        fun ensureChannel(context: Context) {
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            if (manager.getNotificationChannel(CHANNEL_ID) == null) {
                val channel = NotificationChannel(
                    CHANNEL_ID,
                    context.getString(R.string.lumen_channel_name),
                    NotificationManager.IMPORTANCE_LOW,
                )
                channel.setShowBadge(false)
                manager.createNotificationChannel(channel)
            }
        }
    }
}
