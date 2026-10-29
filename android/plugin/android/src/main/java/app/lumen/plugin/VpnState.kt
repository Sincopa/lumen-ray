package app.lumen.plugin

import app.tauri.plugin.JSObject
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicLong

/**
 * Tunnel state shared by the service, the plugin, the tile and the notification.
 * Everything runs in one process; listeners are called on the thread that
 * changed the state.
 */
internal object VpnState {
    enum class Phase(val wire: String) {
        IDLE("idle"),
        CONNECTING("connecting"),
        CONNECTED("connected"),
        STOPPING("stopping"),
        ERROR("error"),
    }

    data class Snapshot(
        val phase: Phase,
        val token: Long,
        val serverName: String?,
        val startedAt: Long?,
        val errorCode: String?,
        val errorMessage: String?,
    ) {
        val running get() = phase == Phase.CONNECTED

        fun toJson(): JSObject = JSObject().apply {
            put("state", phase.wire)
            put("running", running)
            serverName?.let { put("serverName", it) }
            startedAt?.let { put("startedAt", it) }
            errorCode?.let { put("errorCode", it) }
            errorMessage?.let { put("errorMessage", it) }
        }
    }

    private val tokens = AtomicLong()
    private val listeners = CopyOnWriteArrayList<(Snapshot) -> Unit>()

    @Volatile
    var current = Snapshot(Phase.IDLE, 0, null, null, null, null)
        private set

    /** A new start request; the service reports its outcome under this token. */
    fun request(serverName: String?): Long {
        val token = tokens.incrementAndGet()
        publish(Snapshot(Phase.CONNECTING, token, serverName, null, null, null))
        return token
    }

    fun connecting(serverName: String?) = publish(current.copy(phase = Phase.CONNECTING, serverName = serverName, errorCode = null, errorMessage = null))

    fun connected(serverName: String?) = publish(current.copy(phase = Phase.CONNECTED, serverName = serverName, startedAt = System.currentTimeMillis(), errorCode = null, errorMessage = null))

    fun stopping() = publish(current.copy(phase = Phase.STOPPING))

    fun idle() = publish(current.copy(phase = Phase.IDLE, startedAt = null, errorCode = null, errorMessage = null))

    fun failed(code: String, message: String) = publish(current.copy(phase = Phase.ERROR, startedAt = null, errorCode = code, errorMessage = message))

    fun addListener(listener: (Snapshot) -> Unit) = listeners.add(listener)

    fun removeListener(listener: (Snapshot) -> Unit) = listeners.remove(listener)

    private fun publish(snapshot: Snapshot) {
        current = snapshot
        for (listener in listeners) {
            listener(snapshot)
        }
    }
}
