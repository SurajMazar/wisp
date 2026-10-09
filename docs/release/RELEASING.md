# Releasing Wisp

## Build locally
```bash
scripts/build/build-macos.sh                 # release → build/Wisp.app (ad-hoc signed)
scripts/release/package.sh 0.1.0-alpha.2     # → build/release/Wisp-<version>-macos-arm64.{dmg,zip} + SHA256SUMS
```
For a stable local identity (so macOS keeps permissions between builds): `scripts/development/create-signing-identity.sh`.

## Publish from CI
Push a tag (`git tag v0.1.0-alpha.2 && git push origin v0.1.0-alpha.2`) or run the **Release** workflow manually.
CI validates everything, builds, packages, and publishes a GitHub release with the DMG, ZIP and checksums.

### Developer ID signing + notarization (optional)
Add these repository secrets and the same workflow signs with hardened runtime and notarizes + staples the DMG:

| Secret | What |
|---|---|
| `MACOS_CERTIFICATE_P12` | Base64 of the "Developer ID Application" certificate (.p12) |
| `MACOS_CERTIFICATE_PASSWORD` | Its export password |
| `APPLE_ID` / `APPLE_TEAM_ID` | Apple Developer account and team |
| `APPLE_APP_PASSWORD` | App-specific password for notarytool |

Without them, builds are ad-hoc signed: on first open, right-click → Open (or allow in Privacy & Security).

## Updates
Settings → Your data → Check for updates asks GitHub for the latest release only when clicked and links to it.
Nothing downloads or installs automatically.

## Backups and export
A database backup is made automatically once a day (`~/Library/Application Support/Wisp/backups/`, last 7 kept);
Settings → Your data → Back up now / Export my data. To restore, quit Wisp and copy a backup over
`database/wisp.sqlite3`.
