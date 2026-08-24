#!/usr/bin/env bash
set -euo pipefail

runfiles="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
apk="$(find "$runfiles" -type f -name neo.apk -print -quit)"
if [[ -z "$apk" ]]; then
  echo "deploy-apk: Buck APK artifact is missing" >&2
  exit 1
fi

if [[ -n "${ANDROID_HOME:-}" && -x "$ANDROID_HOME/platform-tools/adb" ]]; then
  adb="$ANDROID_HOME/platform-tools/adb"
elif [[ -n "${ANDROID_SDK_ROOT:-}" && -x "$ANDROID_SDK_ROOT/platform-tools/adb" ]]; then
  adb="$ANDROID_SDK_ROOT/platform-tools/adb"
else
  adb="$(command -v adb || true)"
fi

if [[ -z "$adb" ]]; then
  echo "deploy-apk: adb not found; set ANDROID_HOME or put adb on PATH" >&2
  exit 1
fi

exec "$adb" install -r "$apk"
