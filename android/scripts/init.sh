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
app="src-tauri/gen/android/app/build.gradle.kts"
hook='apply(from = file("../../../../android/gradle/app.gradle.kts"))'
grep -qF "$hook" "$app" || printf '\n%s\n' "$hook" >> "$app"
echo "Android project ready: src-tauri/gen/android"
