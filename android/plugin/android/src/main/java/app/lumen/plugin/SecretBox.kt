package app.lumen.plugin

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * AES-256-GCM sealing with a non-exportable key in the Android Keystore.
 * Blob: version, 12-byte nonce, ciphertext with tag. Files live in
 * noBackupFilesDir, so neither cloud backup nor device transfer copies them;
 * the Keystore key would not exist on another device anyway.
 */
internal object SecretBox {
    private const val ALIAS = "lumen.storage.wrap"
    private const val VERSION: Byte = 1
    private const val NONCE = 12

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }

    fun seal(input: ByteArray): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(byteArrayOf(VERSION))
        val sealed = cipher.doFinal(input)
        val nonce = cipher.iv
        check(nonce.size == NONCE)
        return byteArrayOf(VERSION) + nonce + sealed
    }

    fun open(input: ByteArray): ByteArray {
        require(input.size > 1 + NONCE + 16 && input[0] == VERSION) { "damaged" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, input, 1, NONCE))
        cipher.updateAAD(byteArrayOf(VERSION))
        return cipher.doFinal(input, 1 + NONCE, input.size - 1 - NONCE)
    }

    /**
     * The 32-byte key the Rust core uses for its own AES-GCM storage, like the
     * macOS Keychain key. Only its sealed form touches the disk.
     */
    fun storageKey(context: Context): ByteArray {
        val file = File(context.noBackupFilesDir, "storage-key.bin")
        if (file.isFile) {
            return open(file.readBytes())
        }
        val key = ByteArray(32).also { SecureRandom().nextBytes(it) }
        writeAtomically(file, seal(key))
        return key
    }

    fun writeAtomically(file: File, bytes: ByteArray) {
        val temp = File(file.parentFile, file.name + ".tmp")
        temp.outputStream().use { stream ->
            stream.write(bytes)
            stream.fd.sync()
        }
        if (!temp.renameTo(file)) {
            temp.delete()
            error("cannot replace ${file.name}")
        }
    }
}
