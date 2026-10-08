#!/usr/bin/env bash
# Build and assemble Wisp.app.
#
#   scripts/build/build-macos.sh            # release → build/Wisp.app
#   scripts/build/build-macos.sh --debug    # debug   → build/debug/Wisp.app
#   scripts/build/build-macos.sh --debug --no-web   # skip the web build (dev server mode)
#
# Signing: uses $WISP_SIGN_IDENTITY, else a "Wisp Development" identity if present
# (scripts/development/create-signing-identity.sh), else ad-hoc. With ad-hoc signing macOS
# forgets privacy permissions after every rebuild (DECISIONS.md D-005).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

config=release
build_web=1
for arg in "$@"; do
  case "$arg" in
    --debug) config=debug ;;
    --no-web) build_web=0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
build_number="$(git rev-list --count HEAD 2>/dev/null || echo 1)"
[[ "$build_number" == "0" ]] && build_number=1

step() { printf '\n\033[1m▸ %s\033[0m\n' "$*"; }

step "Contracts up to date"
pnpm -s contracts --check

step "Rust core ($config)"
if [[ $config == release ]]; then cargo build --release -p wisp-core; else cargo build -p wisp-core; fi

if [[ $build_web == 1 ]]; then
  step "Web UI"
  pnpm --filter @wisp/web build
fi

step "Swift host ($config)"
scripts/development/swiftpm.sh build -c "$config" --product Wisp
swift_bin="$(scripts/development/swiftpm.sh build -c "$config" --show-bin-path)"

out="build"
[[ $config == debug ]] && out="build/debug"
app="$out/Wisp.app"
step "Assemble $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$swift_bin/Wisp" "$app/Contents/MacOS/Wisp"
cp "target/$config/wisp-core" "$app/Contents/MacOS/wisp-core"
sed -e "s/__VERSION__/$version/" -e "s/__BUILD__/$build_number/" \
  apps/macos/Resources/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist" >/dev/null
if [[ -f apps/macos/Resources/AppIcon.icns ]]; then cp apps/macos/Resources/AppIcon.icns "$app/Contents/Resources/"; fi
if [[ -d apps/web/dist ]]; then
  mkdir -p "$app/Contents/Resources/web"
  cp -R apps/web/dist/. "$app/Contents/Resources/web/"
  find "$app/Contents/Resources/web" -name '*.map' -delete
fi

step "Sign"
identity="${WISP_SIGN_IDENTITY:-}"
if [[ -z "$identity" ]] && security find-identity -v -p codesigning 2>/dev/null | grep -q '"Wisp Development"'; then
  identity="Wisp Development"
fi
if [[ -z "$identity" ]]; then
  identity="-"
  echo "note: ad-hoc signing — macOS will ask for permissions again after each rebuild." >&2
  echo "      run scripts/development/create-signing-identity.sh once to avoid this." >&2
fi
codesign --force --options runtime --timestamp=none --sign "$identity" \
  --entitlements apps/macos/Entitlements/wisp-core.entitlements "$app/Contents/MacOS/wisp-core"
codesign --force --options runtime --timestamp=none --sign "$identity" \
  --entitlements apps/macos/Entitlements/Wisp.entitlements "$app"
codesign --verify --strict --deep "$app"

echo
echo "✓ $app ($version build $build_number, signed: $identity)"
