#!/bin/sh
# Builds the universal macOS Xray runtime from pinned official archives and writes its manifest.
# Set APPLE_SIGNING_IDENTITY to sign it: the bundler seals Resources but never signs Mach-O files there.
set -eu
version=26.9.9
arm_sha=b7cf765d60ccc703853d4218c49a1eacc5bca764543b9540bdeaf45c951afc7d
x64_sha=32b5d106b9936f3ae2044cd283d9e22749b57fd30b34a58792b86c90018bb5e4
geoip_sha=9315c8677c101db1097cc16a53c47cd315e576a6b50734b6a9eed72862e32402
geosite_sha=f404e3ef2f68a167cab0a4383bc239e9cdb831672982819d1549701263095b8f
license_sha=1f256ecad192880510e84ad60474eab7589218784b9a50bc7ceee34c2b91f1d5
root=$(cd "$(dirname "$0")/../.." && pwd)
xray_dir="$root/src-tauri/runtime/xray"
license="$root/src-tauri/runtime/licenses/xray.txt"
binary="$xray_dir/xray"
manifest="$root/src-tauri/build/manifest-macos.json"
cache="$root/artifacts/core-download"
identity=${APPLE_SIGNING_IDENTITY:-}

sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
fail() { echo "$1" >&2; exit 1; }

[ "$(sha "$xray_dir/geoip.dat")" = "$geoip_sha" ] || fail 'Integrity verification failed for geoip.dat'
[ "$(sha "$xray_dir/geosite.dat")" = "$geosite_sha" ] || fail 'Integrity verification failed for geosite.dat'
[ "$(sha "$license")" = "$license_sha" ] || fail 'Integrity verification failed for xray.txt'

# Re-signing changes the hash on every run, so keep a binary this script already signed and recorded.
if [ -n "$identity" ] && [ -f "$binary" ] && [ -f "$manifest" ] &&
	grep -q "\"version\": \"$version\"" "$manifest" &&
	grep -q "\"xray\": \"$(sha "$binary")\"" "$manifest" &&
	codesign --verify --strict "$binary" 2>/dev/null &&
	codesign -dvv "$binary" 2>&1 | grep -qxF "Authority=$identity" &&
	codesign -dvv "$binary" 2>&1 | grep -q 'flags=.*(runtime)'; then
	echo "Reusing signed Xray $version (universal) in $xray_dir."
	exit 0
fi

mkdir -p "$cache"
work=$(mktemp -d "$cache/prepare.XXXXXX")
trap 'rm -rf "$work"' EXIT
for pair in "Xray-macos-arm64-v8a.zip:$arm_sha" "Xray-macos-64.zip:$x64_sha"; do
	name=${pair%%:*}
	expected=${pair#*:}
	archive="$cache/$name"
	if [ ! -f "$archive" ] || [ "$(sha "$archive")" != "$expected" ]; then
		curl -fsSL --proto '=https' -o "$work/$name" "https://github.com/XTLS/Xray-core/releases/download/v$version/$name"
		mv "$work/$name" "$archive"
	fi
	[ "$(sha "$archive")" = "$expected" ] || fail "Xray archive integrity verification failed for $name"
	mkdir "$work/${name%.zip}"
	unzip -q "$archive" xray LICENSE -d "$work/${name%.zip}"
	[ "$(sha "$work/${name%.zip}/LICENSE")" = "$license_sha" ] || fail "Unexpected Xray license in $name"
done

lipo -create -output "$work/xray" "$work/Xray-macos-arm64-v8a/xray" "$work/Xray-macos-64/xray"
{ lipo "$work/xray" -verify_arch arm64 && lipo "$work/xray" -verify_arch x86_64; } || fail "Universal Xray is missing an architecture"
chmod 755 "$work/xray"
if [ -n "$identity" ]; then
	# Hardened runtime and a secure timestamp are required for notarization.
	codesign --force --options runtime --timestamp --sign "$identity" "$work/xray"
	codesign --verify --strict "$work/xray"
fi
[ "$("$work/xray" version | head -n 1 | cut -d' ' -f2)" = "$version" ] || fail 'Prepared Xray does not report the pinned version'
mv -f "$work/xray" "$binary"

cat >"$work/manifest.json" <<EOF
{
  "version": "$version",
  "architecture": "universal",
  "archiveSha256": {
    "Xray-macos-arm64-v8a.zip": "$arm_sha",
    "Xray-macos-64.zip": "$x64_sha"
  },
  "files": {
    "xray": "$(sha "$binary")",
    "xray.txt": "$license_sha",
    "geoip.dat": "$geoip_sha",
    "geosite.dat": "$geosite_sha"
  }
}
EOF
# Rewriting an identical manifest would only force a Rust rebuild.
cmp -s "$work/manifest.json" "$manifest" || mv -f "$work/manifest.json" "$manifest"
echo "Verified Xray $version (universal) prepared in $xray_dir."
