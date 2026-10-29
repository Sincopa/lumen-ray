plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

// geoip.dat, geosite.dat and their manifest come from the desktop runtime, so
// both platforms ship the same verified databases. RuntimeProvider copies them
// out of the APK on first launch.
val repositoryRoot = projectDir.resolve("../../..").canonicalFile
val runtimeAssets: File = layout.buildDirectory.dir("generated/lumen-runtime").get().asFile
val copyRuntimeAssets = tasks.register<Sync>("copyLumenRuntimeAssets") {
    from(repositoryRoot.resolve("src-tauri/runtime/xray")) {
        include("geoip.dat", "geosite.dat", "manifest.json")
        into("lumen-runtime/xray")
    }
    from(repositoryRoot.resolve("src-tauri/runtime/licenses")) {
        include("xray.txt")
        into("lumen-runtime/licenses")
    }
    into(runtimeAssets)
}

android {
    namespace = "app.lumen.plugin"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets {
        getByName("main") {
            assets.srcDir(runtimeAssets)
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

tasks.named("preBuild") {
    dependsOn(copyRuntimeAssets)
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.6.0")
    implementation("com.fasterxml.jackson.core:jackson-databind:2.15.3")
    // Xray-core and the lumencore wrapper, built by android/core/build.sh.
    implementation(files("libs/lumen-core.aar"))
    implementation(project(":tauri-android"))
}
