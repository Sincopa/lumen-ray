package app.lumen.plugin

import android.content.Context
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * The last tunnel the Rust core asked for. The service starts from it alone, so
 * Always-on VPN, the tile and boot start work while the UI is not running. It
 * holds server credentials and is sealed with [SecretBox].
 */
internal data class VpnPlan(
    val config: String,
    val assetDir: String,
    val excludedApps: List<String>,
    val includedApps: List<String>,
    val serverName: String,
    val dns: String,
    val ipv6: Boolean,
    val mtu: Int,
    val autoConnect: Boolean,
) {
    fun save(context: Context) {
        val json = JSONObject()
            .put("config", config)
            .put("assetDir", assetDir)
            .put("excludedApps", JSONArray(excludedApps))
            .put("includedApps", JSONArray(includedApps))
            .put("serverName", serverName)
            .put("dns", dns)
            .put("ipv6", ipv6)
            .put("mtu", mtu)
            .put("autoConnect", autoConnect)
        SecretBox.writeAtomically(file(context), SecretBox.seal(json.toString().toByteArray()))
    }

    companion object {
        private const val TAG = "LumenVpnPlan"

        private fun file(context: Context) = File(context.noBackupFilesDir, "vpn-plan.bin")

        fun exists(context: Context) = file(context).isFile

        fun load(context: Context): VpnPlan? {
            val file = file(context)
            if (!file.isFile) return null
            return try {
                val json = JSONObject(String(SecretBox.open(file.readBytes())))
                VpnPlan(
                    config = json.getString("config"),
                    assetDir = json.getString("assetDir"),
                    excludedApps = json.optJSONArray("excludedApps").strings(),
                    includedApps = json.optJSONArray("includedApps").strings(),
                    serverName = json.optString("serverName"),
                    dns = json.optString("dns", "172.31.255.2"),
                    ipv6 = json.optBoolean("ipv6", true),
                    mtu = json.optInt("mtu", 1500),
                    autoConnect = json.optBoolean("autoConnect", false),
                )
            } catch (error: Exception) {
                // A plan from another Keystore key (restored data) cannot be read.
                Log.w(TAG, "Stored VPN plan is unreadable; it is discarded")
                file.delete()
                null
            }
        }

        private fun JSONArray?.strings(): List<String> {
            if (this == null) return emptyList()
            return (0 until length()).map { getString(it) }
        }
    }
}
