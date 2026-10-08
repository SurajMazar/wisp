#!/usr/bin/env bash
# Creates a local, self-signed code-signing identity named "Wisp Development" in your login
# keychain so that macOS privacy permissions (Screen Recording, Accessibility, Microphone)
# survive rebuilds. Run it yourself once; it modifies your login keychain and may prompt.
#
# This identity is for local development only. Release builds use a Developer ID certificate.
set -euo pipefail
name="Wisp Development"
if security find-identity -v -p codesigning | grep -q "\"$name\""; then
  echo "\"$name\" already exists."; exit 0
fi
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/cert.cnf" <<CNF
[ req ]
distinguished_name = dn
x509_extensions = ext
prompt = no
[ dn ]
CN = $name
[ ext ]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -keyout "$tmp/key.pem" -out "$tmp/cert.pem" -config "$tmp/cert.cnf"
pass="$(openssl rand -hex 16)"
openssl pkcs12 -export -legacy -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -out "$tmp/id.p12" -passout "pass:$pass" -name "$name"
security import "$tmp/id.p12" -k "$HOME/Library/Keychains/login.keychain-db" -P "$pass" -T /usr/bin/codesign
security add-trusted-cert -p codeSign -k "$HOME/Library/Keychains/login.keychain-db" "$tmp/cert.pem"
echo "Created \"$name\". Rebuild with scripts/build/build-macos.sh — permissions will now persist."
