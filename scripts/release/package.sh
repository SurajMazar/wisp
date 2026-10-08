#!/usr/bin/env bash
# Package build/Wisp.app into a DMG and a ZIP with SHA-256 checksums.
#   scripts/release/package.sh [version-label]   → build/release/Wisp-<label>-macos-arm64.{dmg,zip}
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

app="build/Wisp.app"
[[ -d "$app" ]] || { echo "missing $app — run scripts/build/build-macos.sh first" >&2; exit 1; }
codesign --verify --strict --deep "$app"

label="${1:-$(defaults read "$root/$app/Contents/Info" CFBundleShortVersionString)}"
arch="$(lipo -archs "$app/Contents/MacOS/Wisp" | tr ' ' '-')"
name="Wisp-${label}-macos-${arch}"
out="build/release"
rm -rf "$out"
mkdir -p "$out/dmg"

# ZIP via ditto keeps code signatures and extended attributes intact.
ditto -c -k --sequesterRsrc --keepParent "$app" "$out/$name.zip"

# DMG with an Applications shortcut for drag-to-install.
cp -R "$app" "$out/dmg/"
ln -s /Applications "$out/dmg/Applications"
hdiutil create -volname "Wisp" -srcfolder "$out/dmg" -ov -format UDZO -fs HFS+ "$out/$name.dmg" >/dev/null
rm -rf "$out/dmg"

(cd "$out" && shasum -a 256 "$name.dmg" "$name.zip" > SHA256SUMS.txt)
echo "$out/$name.dmg"
echo "$out/$name.zip"
cat "$out/SHA256SUMS.txt"
