#!/usr/bin/env bash
# Builds lumen-core.aar (Xray-core + the lumencore wrapper) for the Android plugin.
# Needs Go, gomobile/gobind on PATH, ANDROID_HOME and ANDROID_NDK_HOME.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/../plugin/android/libs"
mkdir -p "$out"
cd "$here"

: "${ANDROID_HOME:?ANDROID_HOME is not set}"
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME is not set}"

go mod download
gomobile init

# 16 KB pages are required for targetSdk 35+ on Google Play and new devices.
# -checklinkname=0: github.com/wlynxg/anet (interface list on Android) links into net.
gomobile bind \
	-target=android/arm64,android/arm,android/amd64 \
	-androidapi 26 \
	-javapkg app.lumen.core \
	-trimpath \
	-ldflags="-s -w -buildid= -checklinkname=0 -extldflags=-Wl,-z,max-page-size=16384" \
	-o "$out/lumen-core.aar" \
	.

rm -f "$out/lumen-core-sources.jar"
version="$(go list -m -f '{{.Version}}' github.com/xtls/xray-core)"
sha="$(sha256sum "$out/lumen-core.aar" | cut -d' ' -f1)"
printf '{\n  "xrayModule": "%s",\n  "xrayRelease": "26.9.9",\n  "sha256": "%s"\n}\n' "$version" "$sha" > "$out/lumen-core.json"
echo "lumen-core.aar $sha"
