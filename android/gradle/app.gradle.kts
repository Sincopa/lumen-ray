// Lumen settings for the generated app module: release signing and packaging.
// android/scripts/init.sh applies this file, so src-tauri/gen/android stays regenerable.
//
// The key comes from the environment (CI) or from android/keystore.properties
// (local, never committed). Without either, release APKs stay unsigned.
import java.util.Properties

val lumenRoot = rootDir.resolve("../../..").canonicalFile
val localProperties = Properties().apply {
    val file = lumenRoot.resolve("android/keystore.properties")
    if (file.isFile) file.inputStream().use { load(it) }
}

fun signingValue(env: String, property: String): String? =
    System.getenv(env)?.takeIf { it.isNotBlank() } ?: localProperties.getProperty(property)

val storePath = signingValue("LUMEN_ANDROID_KEYSTORE", "storeFile")
val storeSecret = signingValue("LUMEN_ANDROID_KEYSTORE_PASSWORD", "storePassword")
val keyName = signingValue("LUMEN_ANDROID_KEY_ALIAS", "keyAlias")
val keySecret = signingValue("LUMEN_ANDROID_KEY_PASSWORD", "keyPassword")

extensions.configure<com.android.build.api.dsl.ApplicationExtension>("android") {
    // The APK is downloaded from GitHub: compressed native libraries roughly halve
    // it. Android extracts them at install; their ELF segments are 16 KB aligned.
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
    if (storePath != null && storeSecret != null && keyName != null && keySecret != null) {
        signingConfigs {
            create("lumenRelease") {
                storeFile = lumenRoot.resolve("android").resolve(storePath)
                storePassword = storeSecret
                keyAlias = keyName
                keyPassword = keySecret
                enableV2Signing = true
                enableV3Signing = true
            }
        }
        buildTypes {
            getByName("release") {
                signingConfig = signingConfigs.getByName("lumenRelease")
            }
        }
    } else {
        logger.warn("Lumen: no release key configured; the release APK will be unsigned.")
    }
}
