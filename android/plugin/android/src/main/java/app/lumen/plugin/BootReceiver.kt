package app.lumen.plugin

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.util.Log

/**
 * Reconnects after a reboot or an app update when the user turned on
 * auto-connect. System Always-on VPN does the same without this receiver.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED && intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        val plan = VpnPlan.load(context) ?: return
        if (!plan.autoConnect || VpnService.prepare(context) != null) return
        try {
            VpnState.request(plan.serverName)
            LumenVpnService.start(context)
        } catch (error: Exception) {
            VpnState.idle()
            Log.w("LumenBoot", "Auto-connect was not allowed to start", error)
        }
    }
}
