# Desktop development

User documentation: [English](../README.md) · [Français](../README.fr.md).

## Architecture

`main.rs` → worker thread → MyWhoosh web helper / FIT editor / Garmin sidecar.

- `main.rs`, `design.rs`: egui interface; no account calls on the UI rendering path.
- `i18n.rs`, `locales/en.json`: render-time translations. French is the source
  language. Initial language follows macOS; a menu override is saved in local preferences.
  Activity names, email addresses and numeric identifiers are not translated.
- `mywhoosh.rs`, `web_session.rs`, `bin/whoosh-web-login.rs`: official-site WebKit
  sign-in, persistent website session, activity listing and downloads. Website
  UI/consent language remains controlled by MyWhoosh.
- `garmin.rs`, `bridge/garmin_bridge.py`: JSON-lines subprocess protocol using
  garminconnect/garth. Credentials and tokens travel through private pipes,
  never process arguments. Uploaded/duplicate/received are distinct outcomes.
- `session_store.rs`: macOS Keychain service `com.mywhooshbridge.sessions`;
  `garmin` and `mywhoosh.credentials` accounts. Recorder/language preferences use
  `~/Library/Application Support/Whoosh Bridge/preferences.json` (atomic writes,
  mode 0600). The legacy `garmin.device_preferences` item is read once for migration
  and retained; a denied migration never overwrites existing preferences.
  Startup reads run on the worker, not the UI thread. MyWhoosh credentials are
  loaded only after its website session requires authentication.
- `fit.rs`, `fit_summary.rs`, `fit_identity.rs`: binary FIT parsing, validation,
  conservative summary repair and recorder identity normalization.
- `devices.rs`: Edge catalogue and declared firmware; source links and audited
  identifiers are in `tests/device_catalog_audit.json`.

The app requires macOS 14+ for the persistent WebKit store. It is not 100% Rust:
the Garmin sidecar is Python, bundled in release apps. The historical Python CLI is no longer included.

## FIT behavior

The original file's CRC, size and activity file type are checked before edits.
Measurements, unknown/developer data and accessory identities are preserved.
For MyWhoosh files, missing/invalid/zero session averages are repaired from valid
record samples (zero power/cadence retained, zero heart rate excluded). Multiple
sessions use their own time windows. Virtual record temperatures become FIT's
invalid temperature value. Unusual compressed session summaries are not rewritten.

`file_id`, recorder `device_info` (index 0), and identifiers in `training_file`
use the selected manufacturer/product/Unit ID. A MyWhoosh device_info without an
index is also recognized. Names/descriptors and declared software are consistent;
missing fields are inserted, shared local definitions are restored, and stale
hardware revisions are invalidated. Nonzero accessory indices remain unchanged.
`device_info.software_version` uses scale 100; `file_creator.software_version`
contains the raw release integer. Header and file CRCs are regenerated.

This does not calculate or fabricate Training Effect, VO₂max or training load.
Real Garmin physiological processing must be verified independently of local tests.

## Build

Run the commands below from the repository root. Build prerequisites are listed
in the [main README](../README.md#requirements).

```bash
WHOOSH_PYTHON="$(command -v python3.12)" bash desktop/scripts/build-macos.sh
```

The app is produced at `desktop/target/macos/Whoosh Bridge.app` for the local
architecture. The script uses Cargo.lock, bundles a Python sidecar, includes the
icon and dependency notices, and ad-hoc signs the bundle. Developer ID signing
and notarization are not configured.

To regenerate the icon on macOS:

```bash
swift desktop/scripts/generate-icon.swift
```

The script writes `desktop/assets/icon.svg`, `desktop/assets/icon.png`, and
`desktop/assets/WhooshBridge.icns`.
The editable paths are original project artwork. The system font is loaded from
macOS at runtime; no Apple font is copied into the bundle or repository.

## Offline checks

Install `desktop/requirements-test.txt` in the root `venv`, Node.js (CI uses 22),
and the Rust `rustfmt` and `clippy` components first. `bash scripts/check.sh` runs
all checks below.

```bash
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo test --manifest-path desktop/Cargo.toml --locked
cargo clippy --manifest-path desktop/Cargo.toml --locked --all-targets -- -D warnings
venv/bin/python -m unittest discover -s desktop/tests -v
# Requires desktop/requirements-test.txt and built Rust dependencies:
venv/bin/python desktop/tests/check_fit_identity.py
node --test desktop/tests/*.test.cjs
```

Independent FIT checks decode 26 synthetic exports (13 Edge models, both byte
orders) with fit-tool and compare identities, versions, samples, accessories and
CRCs. Account/network operations are mocked or avoided in these tests.

Synthetic UI previews (debug build only; no account restoration or transfers):

```bash
cargo build --manifest-path desktop/Cargo.toml --bins
WHOOSH_UI_LANGUAGE=en WHOOSH_UI_PREVIEW=rides \
  WHOOSH_UI_SCREENSHOT=/tmp/whoosh-en.png desktop/target/debug/whoosh-bridge
```

Use `fr` for French, `WHOOSH_UI_SMALL=1` for the minimum window, and
`WHOOSH_UI_PREVIEW=empty|rides|confirm|error|mfa|loading|no-rides` for preview states.

## Diagnostics and storage

Logs: `~/Library/Logs/Whoosh Bridge/connection.log`. Only fixed operation categories
and validated HTTP statuses are logged, not raw service responses or secrets.
Keychain write failures are shown without turning an already-confirmed upload
into an apparent failure. Identical Garmin tokens are not rewritten; a failed
save is not repeatedly retried during that session. Transient network errors preserve sessions.

Unit IDs are saved after 800 ms without typing, on focus loss, before transfer and
on normal exit. Clearing the field removes that model's saved ID. A Keychain read
failure during migration prevents overwriting preferences. Account logout preserves device preferences.

Never add real FIT files, logs, cookies, token stores, `.env`, Keychain dumps or
account screenshots to the repository. Use synthetic fixtures and preview mode.
