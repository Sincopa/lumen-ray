#!/bin/sh
set -eu
app=${1:?Usage: verify-macos-bundle.sh /path/to/Lumen.app}
core="$app/Contents/Resources/runtime/xray/xray"
executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")

for binary in "$app" "$core"; do
	codesign --verify --strict --verbose=2 "$binary"
	details=$(codesign -d --verbose=4 "$binary" 2>&1)
	printf '%s\n' "$details"
	printf '%s\n' "$details" | grep -Eq '^CodeDirectory .*flags=.*[(,]adhoc[),]'
done
codesign -d --verbose=4 "$core" 2>&1 | grep -Eq 'flags=.*[(,]runtime[),]'
for binary in "$app/Contents/MacOS/$executable" "$core"; do
	lipo "$binary" -verify_arch arm64
	lipo "$binary" -verify_arch x86_64
done
test ! -e "$app/Contents/Resources/runtime/xray/xray.exe"
test ! -e "$app/Contents/Resources/runtime/drivers"
