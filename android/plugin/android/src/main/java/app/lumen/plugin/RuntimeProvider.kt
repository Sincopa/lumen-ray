package app.lumen.plugin

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.system.Os
import android.util.Log
import java.io.File

/**
 * Initializes before the activity and before the Rust core reads its
 * environment: the bundled geo databases are copied out of the APK into
 * LUMEN_RUNTIME_DIR, and the Rust storage key is unsealed into LUMEN_STORAGE_KEY.
 * The key stays in this process's memory, like a key held by any keychain client.
 */
class RuntimeProvider : ContentProvider() {
    override fun onCreate(): Boolean {
        val context = context ?: return false
        try {
            val runtime = prepareRuntime(context)
            Os.setenv("LUMEN_RUNTIME_DIR", runtime.absolutePath, true)
        } catch (error: Exception) {
            Log.e(TAG, "Bundled runtime could not be prepared", error)
        }
        try {
            val key = SecretBox.storageKey(context)
            Os.setenv("LUMEN_STORAGE_KEY", key.joinToString("") { "%02x".format(it) }, true)
            key.fill(0)
        } catch (error: Exception) {
            Log.e(TAG, "Storage key is unavailable", error)
        }
        return true
    }

    private fun prepareRuntime(context: Context): File {
        val root = File(context.filesDir, ASSET_ROOT)
        val bundled = context.assets.open("$ASSET_ROOT/xray/manifest.json").use { it.readBytes() }
        val installed = File(root, "xray/manifest.json")
        // The manifest carries the hashes; an unchanged manifest means unchanged files.
        if (installed.isFile && installed.readBytes().contentEquals(bundled) &&
            FILES.all { File(root, it).isFile }
        ) {
            return root
        }
        for (name in FILES) {
            val target = File(root, name)
            target.parentFile?.mkdirs()
            val temp = File(target.parentFile, target.name + ".tmp")
            context.assets.open("$ASSET_ROOT/$name").use { input ->
                temp.outputStream().use { output -> input.copyTo(output) }
            }
            if (!temp.renameTo(target)) error("cannot install $name")
        }
        // Written last: an interrupted copy is repeated on the next start.
        SecretBox.writeAtomically(installed, bundled)
        return root
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0

    private companion object {
        const val TAG = "LumenRuntime"
        const val ASSET_ROOT = "lumen-runtime"
        val FILES = listOf("xray/geoip.dat", "xray/geosite.dat", "licenses/xray.txt")
    }
}
