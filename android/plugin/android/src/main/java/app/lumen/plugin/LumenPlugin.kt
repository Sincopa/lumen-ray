package app.lumen.plugin

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.net.Uri
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.util.Base64
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import app.lumen.core.lumencore.Lumencore
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.ByteArrayOutputStream
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

@InvokeArg
class StartArgs {
    var config: String = ""
    var assetDir: String = ""
    var excludedApps: List<String> = emptyList()
    var includedApps: List<String> = emptyList()
    var serverName: String = ""
    var dns: String = "172.31.255.2"
    var ipv6: Boolean = true
    var mtu: Int = 1500
    var autoConnect: Boolean = false
}

@InvokeArg
class ValidateArgs {
    var config: String = ""
    var assetDir: String = ""
}

@InvokeArg
class ProbeArgs {
    var id: Long = 0
}

@InvokeArg
class UrlArgs {
    var url: String = ""
}

@InvokeArg
class IconsArgs {
    var packages: List<String> = emptyList()
    var size: Int = 96
}

/**
 * Commands the Rust core calls. They arrive on the main thread, so anything
 * slow (Xray loading geosite, icon rendering) moves to [worker], and start/stop
 * resolve later from the service's state changes.
 */
@TauriPlugin
class LumenPlugin(private val activity: Activity) : Plugin(activity) {
    private val worker = Executors.newCachedThreadPool()
    private val main = Handler(Looper.getMainLooper())

    /**
     * The activity draws edge to edge, and Tauri leaves insets to the app. The
     * WebView sits between the system bars and above the keyboard, on Lumen's
     * dark background with light bar icons.
     */
    override fun load(webView: WebView) {
        val window = activity.window
        window.decorView.setBackgroundColor(BACKGROUND)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
        }
        ViewCompat.setOnApplyWindowInsetsListener(webView) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
            val bottom = maxOf(bars.bottom, keyboard.bottom)
            (view.layoutParams as? ViewGroup.MarginLayoutParams)?.let { params ->
                // Insets arrive on every keyboard animation frame; only a change relayouts the WebView.
                if (params.leftMargin != bars.left || params.topMargin != bars.top ||
                    params.rightMargin != bars.right || params.bottomMargin != bottom
                ) {
                    params.setMargins(bars.left, bars.top, bars.right, bottom)
                    view.layoutParams = params
                }
            }
            WindowInsetsCompat.CONSUMED
        }
        ViewCompat.requestApplyInsets(webView)
    }

    @Command
    fun prepare(invoke: Invoke) {
        val consent = VpnService.prepare(activity)
        if (consent == null) {
            requestNotifications()
            invoke.resolve(JSObject().put("granted", true))
            return
        }
        // Consent was revoked, usually because another VPN app took over. A
        // background reconnect must not pop the system dialog over other apps
        // or take the VPN back; the user reconnects from Lumen.
        val visible = (activity as? LifecycleOwner)?.lifecycle?.currentState
            ?.isAtLeast(Lifecycle.State.RESUMED) == true
        if (!visible) {
            invoke.reject("Lumen is not allowed to create a VPN", "VPN_PERMISSION_DENIED")
            return
        }
        startActivityForResult(invoke, consent, "vpnConsent")
    }

    @ActivityCallback
    fun vpnConsent(invoke: Invoke, result: ActivityResult) {
        val granted = VpnService.prepare(activity) == null
        if (granted) requestNotifications()
        invoke.resolve(JSObject().put("granted", granted))
    }

    private fun requestNotifications() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(activity, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.POST_NOTIFICATIONS), NOTIFICATION_REQUEST)
        }
    }

    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(StartArgs::class.java)
        if (args.config.isEmpty() || args.assetDir.isEmpty()) {
            invoke.reject("Missing tunnel configuration", "INVALID_CONFIG")
            return
        }
        val plan = VpnPlan(
            config = args.config,
            assetDir = args.assetDir,
            excludedApps = args.excludedApps,
            includedApps = args.includedApps,
            serverName = args.serverName,
            dns = args.dns,
            ipv6 = args.ipv6,
            mtu = args.mtu,
            autoConnect = args.autoConnect,
        )
        try {
            plan.save(activity)
        } catch (error: Exception) {
            invoke.reject("Could not store the tunnel plan", "STORAGE_FAILED")
            return
        }
        val token = VpnState.request(args.serverName)
        await(invoke, START_TIMEOUT_MS) { state ->
            state.token == token && state.phase in FINAL_START
        }
        try {
            LumenVpnService.start(activity)
        } catch (error: Exception) {
            VpnState.failed("TUN_FAILED", error.message ?: "Android refused to start the VPN service")
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        val state = VpnState.current
        if (state.phase == VpnState.Phase.IDLE || state.phase == VpnState.Phase.ERROR) {
            invoke.resolve(state.toJson())
            return
        }
        await(invoke, STOP_TIMEOUT_MS) { it.phase == VpnState.Phase.IDLE || it.phase == VpnState.Phase.ERROR }
        LumenVpnService.stop(activity)
    }

    /**
     * Resolves [invoke] with the first state matching [done]. A start that ends
     * in an error rejects with that error's code, so Rust sees the same codes as
     * on desktop.
     */
    private fun await(invoke: Invoke, timeoutMs: Long, done: (VpnState.Snapshot) -> Boolean) {
        val finished = AtomicBoolean(false)
        lateinit var listener: (VpnState.Snapshot) -> Unit
        val timeout = Runnable {
            if (finished.compareAndSet(false, true)) {
                VpnState.removeListener(listener)
                invoke.reject("The VPN service did not respond", "TUN_FAILED")
            }
        }
        listener = { state ->
            if (done(state) && finished.compareAndSet(false, true)) {
                VpnState.removeListener(listener)
                main.removeCallbacks(timeout)
                if (state.phase == VpnState.Phase.ERROR) {
                    invoke.reject(state.errorMessage ?: "VPN failed", state.errorCode ?: "TUN_FAILED")
                } else {
                    invoke.resolve(state.toJson())
                }
            }
        }
        VpnState.addListener(listener)
        main.postDelayed(timeout, timeoutMs)
    }

    @Command
    fun status(invoke: Invoke) {
        invoke.resolve(VpnState.current.toJson())
    }

    @Command
    fun stats(invoke: Invoke) {
        // The core's lock is held while a tunnel starts; never wait on it here.
        worker.execute { invoke.resolve(JSObject(Lumencore.queryStats(false))) }
    }

    @Command
    fun startProbe(invoke: Invoke) {
        val args = invoke.parseArgs(ValidateArgs::class.java)
        worker.execute {
            val id = Lumencore.startProbe(args.config, args.assetDir)
            if (id > 0) {
                invoke.resolve(JSObject().put("id", id))
            } else {
                invoke.reject("The probe core did not start", "CORE_START_FAILED")
            }
        }
    }

    @Command
    fun stopProbe(invoke: Invoke) {
        val id = invoke.parseArgs(ProbeArgs::class.java).id
        worker.execute {
            Lumencore.stopProbe(id)
            invoke.resolve()
        }
    }

    @Command
    fun validate(invoke: Invoke) {
        val args = invoke.parseArgs(ValidateArgs::class.java)
        worker.execute {
            val error = Lumencore.validate(args.config, args.assetDir)
            if (error.isEmpty()) invoke.resolve() else invoke.reject(error, "INVALID_CONFIG")
        }
    }

    @Command
    fun coreVersion(invoke: Invoke) {
        invoke.resolve(JSObject().put("version", Lumencore.version()))
    }

    @SuppressLint("QueryPermissionsNeeded")
    @Command
    fun installedApps(invoke: Invoke) {
        worker.execute {
            try {
                val pm = activity.packageManager
                val launcher = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
                val seen = HashSet<String>()
                val apps = JSArray()
                for (entry in pm.queryIntentActivities(launcher, 0).sortedBy { it.loadLabel(pm).toString().lowercase() }) {
                    val info = entry.activityInfo.applicationInfo
                    if (info.packageName == activity.packageName || !seen.add(info.packageName)) continue
                    apps.put(
                        JSObject()
                            .put("package", info.packageName)
                            .put("label", info.loadLabel(pm).toString())
                            .put("system", info.flags and ApplicationInfo.FLAG_SYSTEM != 0),
                    )
                }
                invoke.resolve(JSObject().put("apps", apps))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Cannot list apps", "APPS_UNAVAILABLE")
            }
        }
    }

    @Command
    fun appIcons(invoke: Invoke) {
        val args = invoke.parseArgs(IconsArgs::class.java)
        val size = args.size.coerceIn(16, 256)
        worker.execute {
            val pm = activity.packageManager
            val icons = JSObject()
            for (name in args.packages.take(MAX_ICONS)) {
                try {
                    val drawable = pm.getApplicationIcon(name)
                    val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
                    drawable.setBounds(0, 0, size, size)
                    drawable.draw(Canvas(bitmap))
                    val png = ByteArrayOutputStream()
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, png)
                    bitmap.recycle()
                    icons.put(name, "data:image/png;base64," + Base64.encodeToString(png.toByteArray(), Base64.NO_WRAP))
                } catch (_: Exception) {
                    // Uninstalled or unreadable: the UI shows its placeholder.
                }
            }
            invoke.resolve(JSObject().put("icons", icons))
        }
    }

    @Command
    fun openUrl(invoke: Invoke) {
        val url = invoke.parseArgs(UrlArgs::class.java).url
        if (!url.startsWith("https://")) {
            invoke.reject("Only https links open", "INVALID_URL")
            return
        }
        try {
            activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("No browser is available", "PLATFORM_UNSUPPORTED")
        }
    }

    @Command
    fun moveToBackground(invoke: Invoke) {
        activity.moveTaskToBack(true)
        invoke.resolve()
    }

    @Command
    fun openVpnSettings(invoke: Invoke) {
        try {
            activity.startActivity(Intent(Settings.ACTION_VPN_SETTINGS))
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("VPN settings are unavailable", "PLATFORM_UNSUPPORTED")
        }
    }

    @Command
    fun batteryOptimization(invoke: Invoke) {
        val power = activity.getSystemService(PowerManager::class.java)
        invoke.resolve(JSObject().put("ignored", power?.isIgnoringBatteryOptimizations(activity.packageName) == true))
    }

    @SuppressLint("BatteryLife")
    @Command
    fun requestBatteryExemption(invoke: Invoke) {
        try {
            activity.startActivity(
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${activity.packageName}")),
            )
        } catch (_: Exception) {
            activity.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
        invoke.resolve()
    }

    private companion object {
        /** #08080B, the app viewport background. */
        const val BACKGROUND = 0xFF08080B.toInt()
        const val NOTIFICATION_REQUEST = 0x4c4e
        const val START_TIMEOUT_MS = 45_000L
        const val STOP_TIMEOUT_MS = 15_000L
        const val MAX_ICONS = 400
        val FINAL_START = setOf(VpnState.Phase.CONNECTED, VpnState.Phase.ERROR, VpnState.Phase.IDLE)
    }
}
