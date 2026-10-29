#!/usr/bin/env bash
# Toolchain paths for the Android build. Source it: `. android/scripts/env.sh`.
# Defaults match the layout README.md describes; existing values win.
LUMEN_ANDROID_TOOLS="${LUMEN_ANDROID_TOOLS:-$HOME/Android}"
export JAVA_HOME="${JAVA_HOME:-$LUMEN_ANDROID_TOOLS/jdk17}"
export ANDROID_HOME="${ANDROID_HOME:-$LUMEN_ANDROID_TOOLS/sdk}"
if [ -z "${NDK_HOME:-}" ]; then
	NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)"
	export NDK_HOME
fi
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$NDK_HOME}"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"
# A Go toolchain unpacked next to the SDK, when none is installed system-wide.
if ! command -v go >/dev/null 2>&1 && [ -d "$LUMEN_ANDROID_TOOLS/go" ]; then
	export GOROOT="$LUMEN_ANDROID_TOOLS/go"
	export GOPATH="${GOPATH:-$LUMEN_ANDROID_TOOLS/gopath}"
	export PATH="$GOROOT/bin:$PATH"
fi
# gomobile and gobind live in GOPATH/bin.
if command -v go >/dev/null 2>&1; then
	gobin="$(go env GOPATH)/bin"
	if command -v cygpath >/dev/null 2>&1; then
		gobin="$(cygpath -u "$gobin")"
	fi
	export PATH="$gobin:$PATH"
fi
