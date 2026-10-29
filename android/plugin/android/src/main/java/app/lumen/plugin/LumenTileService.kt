package app.lumen.plugin

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** Quick Settings toggle. It reuses the last plan, so it works without opening Lumen. */
class LumenTileService : TileService() {
    private val main = Handler(Looper.getMainLooper())
    // The service reports from its worker thread; the tile updates on the main one.
    private val listener: (VpnState.Snapshot) -> Unit = { state -> main.post { render(state) } }

    override fun onStartListening() {
        VpnState.addListener(listener)
        render(VpnState.current)
    }

    override fun onStopListening() {
        VpnState.removeListener(listener)
    }

    override fun onClick() {
        val state = VpnState.current
        if (state.phase == VpnState.Phase.CONNECTED || state.phase == VpnState.Phase.CONNECTING) {
            LumenVpnService.stop(this)
            return
        }
        if (VpnService.prepare(this) != null || !VpnPlan.exists(this)) {
            openApp()
            return
        }
        try {
            VpnState.request(state.serverName)
            LumenVpnService.start(this)
        } catch (_: Exception) {
            // Android may refuse a foreground start from the tile; Lumen can start it.
            VpnState.idle()
            openApp()
        }
    }

    private fun render(state: VpnState.Snapshot) {
        val tile = qsTile ?: return
        tile.state = when (state.phase) {
            VpnState.Phase.CONNECTED, VpnState.Phase.CONNECTING -> Tile.STATE_ACTIVE
            else -> Tile.STATE_INACTIVE
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.subtitle = when (state.phase) {
                VpnState.Phase.CONNECTED -> state.serverName
                VpnState.Phase.CONNECTING -> getString(R.string.lumen_notification_connecting)
                else -> null
            }
        }
        tile.updateTile()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    private fun openApp() {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_IMMUTABLE))
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(launch)
        }
    }
}
