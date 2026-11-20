#!/usr/bin/env bash
# Generates src-tauri/gen/android (ignored by git), copies the Lumen launcher
# icons into it and hooks android/gradle/app.gradle.kts (signing, packaging).
# Safe to run again.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/android/scripts/env.sh"
cd "$root"
if [ ! -d src-tauri/gen/android ]; then
	CI=true npx tauri android init --ci
fi
# Lumen launcher icons replace the Tauri placeholders `android init` writes.
res="src-tauri/gen/android/app/src/main/res"
cp -R android/res/. "$res/"
# Stale icon files of earlier Lumen versions; the icons now live in the plugin.
rm -f "$res"/mipmap-*/ic_launcher_foreground.png "$res/values/lumen_launcher.xml"
# The home screen entry is one of the plugin's activity-aliases (the icon the
# user picked), so MainActivity itself must not be a launcher entry too. It
# keeps MAIN, the TV launcher category and the deep links.
manifest="src-tauri/gen/android/app/src/main/AndroidManifest.xml"
sed -i.bak '/<category android:name="android.intent.category.LAUNCHER" \/>/d' "$manifest"
rm -f "$manifest.bak"
app="src-tauri/gen/android/app/build.gradle.kts"
hook='apply(from = file("../../../../android/gradle/app.gradle.kts"))'
grep -qF "$hook" "$app" || printf '\n%s\n' "$hook" >> "$app"
echo "Android project ready: src-tauri/gen/android"
