#!/usr/bin/env bash
# Builds Lumen APKs into release/android.
#   android/scripts/build-apk.sh            release, one APK per ABI + universal
#   android/scripts/build-apk.sh debug      debug, arm64 only (fast, for a phone)
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/android/scripts/env.sh"
mode="${1:-release}"
# Size matters for a downloaded APK; the desktop release profile stays as it is.
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_STRIP=true
cd "$root"
[ -f android/plugin/android/libs/lumen-core.aar ] || bash android/core/build.sh
bash android/scripts/init.sh
out="release/android"
mkdir -p "$out"
if [ "$mode" = "debug" ]; then
	CI=true npx tauri android build --apk --debug --target aarch64
	cp src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk "$out/Lumen-debug-arm64.apk"
else
	CI=true npx tauri android build --apk --split-per-abi --target aarch64 armv7 x86_64
	CI=true npx tauri android build --apk --target aarch64 armv7 x86_64
	version="$(node -p "require('./package.json').version")"
	rm -f "$out"/Lumen-*.apk
	find src-tauri/gen/android/app/build/outputs/apk -name '*-release*.apk' | while read -r apk; do
		flavor="$(basename "$(dirname "$(dirname "$apk")")")"
		# Android refuses unsigned APKs ("App not installed"); never ship one by mistake.
		case "$apk" in
		*-unsigned.apk) name="Lumen-$version-$flavor-unsigned.apk" ;;
		*) name="Lumen-$version-$flavor.apk" ;;
		esac
		cp "$apk" "$out/$name"
	done
	if ls "$out"/*-unsigned.apk >/dev/null 2>&1; then
		echo "WARNING: no release key, the APKs are unsigned and will not install. See README.md." >&2
	else
		apksigner="$(ls "$ANDROID_HOME"/build-tools/*/apksigner* 2>/dev/null | sort -V | tail -1)"
		for apk in "$out"/*.apk; do
			"$apksigner" verify "$apk"
		done
	fi
fi
(cd "$out" && sha256sum ./*.apk > SHA256SUMS.txt)
ls -la "$out"
