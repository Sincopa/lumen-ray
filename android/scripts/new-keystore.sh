#!/usr/bin/env bash
# Creates the release signing key in android/keystore/ and android/keystore.properties.
# Keep both out of git and back them up offline: Android installs updates only
# when they carry the same signature, so a lost key means users must reinstall.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/android/scripts/env.sh"
store="$root/android/keystore/lumen-release.jks"
if [ -e "$store" ]; then
	echo "Already exists: $store" >&2
	exit 1
fi
mkdir -p "$(dirname "$store")"
read -r -s -p "Keystore password (6+ characters): " password
echo
keytool -genkeypair -v -keystore "$store" -alias lumen -keyalg RSA -keysize 4096 \
	-validity 10000 -storepass "$password" -keypass "$password" \
	-dname "CN=Lumen, O=Lumen"
cat > "$root/android/keystore.properties" <<PROPERTIES
storeFile=keystore/lumen-release.jks
storePassword=$password
keyAlias=lumen
keyPassword=$password
PROPERTIES
chmod 600 "$root/android/keystore.properties" 2>/dev/null || true
echo "Created $store and android/keystore.properties"
