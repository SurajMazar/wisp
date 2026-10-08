#!/usr/bin/env bash
# Run SwiftPM for the macOS host: scripts/development/swiftpm.sh build|test [args...]
#
# Works around a broken Command Line Tools install where stale (2024) PackageDescription
# *.private.swiftinterface files shadow the current manifest API, making every Package.swift
# fail with errors like "reference to member 'v15' cannot be resolved". The proper fix is to
# reinstall the Command Line Tools (or install Xcode); this shim avoids touching system files by
# pointing SwiftPM at a clean user-space copy of the manifest API. On a healthy toolchain it is
# a no-op pass-through.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
clt_pm="$(dirname "$(xcrun --find swift)")/../lib/swift/pm"
pd="$clt_pm/ManifestAPI/PackageDescription.swiftmodule"

if [[ -f "$pd/arm64-apple-macos.private.swiftinterface" ]] \
  && ! grep -q "swiftLanguageModes" "$pd/arm64-apple-macos.private.swiftinterface"; then
  shim="$root/build/.swiftpm-libs"
  if [[ ! -d "$shim/ManifestAPI" ]]; then
    mkdir -p "$shim"
    cp -R "$clt_pm/ManifestAPI" "$shim/"
    if [[ -d "$clt_pm/PluginAPI" ]]; then cp -R "$clt_pm/PluginAPI" "$shim/"; fi
    rm -f "$shim"/ManifestAPI/PackageDescription.swiftmodule/*.private.swiftinterface
    echo "swiftpm: stale Command Line Tools manifest API detected; using shim at $shim" >&2
  fi
  export SWIFTPM_CUSTOM_LIBS_DIR="$shim"
fi

# Command Line Tools ship Swift Testing outside the default framework search path.
extra=()
dev_fw="$(dirname "$(xcrun --find swift)")/../../Library/Developer/Frameworks"
if [[ "${1:-}" == "test" && -d "$dev_fw/Testing.framework" ]]; then
  extra=(-Xswiftc -F -Xswiftc "$dev_fw" -Xlinker -rpath -Xlinker "$dev_fw"
         -Xlinker -rpath -Xlinker "$dev_fw/../usr/lib")
fi

exec swift "$@" ${extra[@]+"${extra[@]}"} --package-path "$root/apps/macos"
