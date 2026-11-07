# Signed macOS updates

Whoosh Bridge uses Sparkle 2, with separate `appcast-arm64.xml` and
`appcast-x86_64.xml` feeds. Only publish a feed for an architecture after building
and testing its archive. A source ZIP is never an application update.

The app checks daily by default. Users can disable checks or check manually in
**•••**. Installation requires the user's action in Sparkle's window. Relaunch
is deferred while a Whoosh Bridge operation is in progress. Sparkle's native
windows follow macOS language preferences; app-owned controls follow the app's
English/French setting.

## Build and prepare a release

1. Set the user-facing version in `desktop/Cargo.toml` (update Cargo.lock too).
   Increase `build_number` in `desktop/update-config.json` for **every release**,
   even if the user-facing version stays the same. Never reuse a published build.
2. Build on the target architecture and package:

   ```bash
   WHOOSH_PYTHON="$(command -v python3.12)" bash desktop/scripts/build-macos.sh
   bash scripts/package-app.sh
   ```

3. Generate the signed feed, specifying the exact future GitHub Release tag:

   ```bash
   python3.12 scripts/prepare-update.py --tag v0.1.0-alpha.3 \
     --archive dist/Whoosh-Bridge-0.1.0-alpha.3-macos-arm64.zip
   ```

   The signing tools use the Keychain account `com.mywhooshbridge.updates`.
   The script verifies the bundled key, build, feed, signature attributes and
   archive length. It writes a publication manifest in `dist/update-arm64.json`.
   Feed generation refuses to replace an equal/newer published build; increase
   the build number if changing an already prepared release.

4. Upload **that exact ZIP** and its `.sha256` to the specified GitHub Release.
   Verify the public download and checksum before committing/pushing the feed
   to `main`. The app loads the raw GitHub feed URL embedded in its Info.plist.
   Do not regenerate, rename or modify the ZIP after signing.
5. Commit the generated architecture-specific appcast and publish it to `main`
   **after** the Release asset is publicly downloadable. Verify its URL returns
   the signed XML and check for updates from an older installed build.

Neither packaging nor feed generation publishes files or creates a GitHub Release.
An existing installation without Sparkle needs one manual upgrade first.
No Intel compatibility claim is implied by an Apple Silicon build.

## Signing and privacy

The public Ed25519 key is committed in `desktop/update-config.json`. The private
key remains in the maintainer's login Keychain; never commit/export it into this
repository, an app bundle or a source archive. Back it up securely using Sparkle's
`generate_keys` instructions before relying on this update channel. Losing it
means existing ad-hoc signed installations cannot accept a replacement signing
key through this channel and will need a manual reinstall.

Both the feed and archives must validate. Archive verification happens **before
extraction**. Signed-feed failures do not expire. GitHub serves the feed and ZIPs
over HTTPS; it receives normal network metadata (including IP address and update
client headers), but no Garmin/MyWhoosh credentials, tokens or activity files.
System profiling and unattended installation are disabled.

The framework download is pinned by version and SHA-256. The app remains ad-hoc
signed and unnotarized: Sparkle does not remove Gatekeeper's first-install checks
or Keychain permission prompts after an app identity changes.

## Checks

```bash
bash scripts/check.sh
WHOOSH_PYTHON="$(command -v python3.12)" bash desktop/scripts/test-updater.sh
```

The native lifecycle test uses synthetic state and never updates the installed
app or accesses Garmin/MyWhoosh. The Python tests validate architecture selection
and reject stale builds, unexpected keys/feeds and unsigned update policies.
The local Apple Silicon integration test passed with the bundled adapter:
version 1 of an isolated app downloaded a signed ZIP, installed version 2, then
relaunched and wrote its version marker. No Garmin/MyWhoosh accounts were used.
This does not validate a future public GitHub release or Intel installation.

To repeat the isolated test (requires the update signing key in Keychain):

```bash
python3.12 desktop/scripts/prepare-updater-smoke.py
python3.12 -m http.server 18765 --bind 127.0.0.1 \
  --directory desktop/target/updater-smoke/server
# With the server running, launch this app in Finder:
# desktop/target/updater-smoke/installed/Whoosh Update Test.app
```

Choose Install Update and, if prompted, Install and Relaunch. The file
`desktop/target/updater-smoke/installed/launched-version.txt` must contain `2`.
Stop the local server afterward. Only the synthetic fixture uses local HTTP;
production feeds and archives use HTTPS.

Reference: [Sparkle setup](https://sparkle-project.org/documentation/),
[publishing](https://sparkle-project.org/documentation/publishing/).
