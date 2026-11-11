# Whoosh Bridge: contributor guide

## Scope and layout

The maintained product is a macOS 14+ app. `desktop/` contains its Cargo project,
Rust sources, Python Garmin bridge, assets, translations, packaging scripts and
offline tests. The historical Python CLI and services have been removed.
Read `desktop/README.md` for architecture and FIT behavior.

- `desktop/native/swift/`: maintained SwiftUI interface, private engine transport and paired French/English UI strings.
- `desktop/src/bin/whoosh-engine.rs`, `lib.rs`: private Rust engine and shared modules.
- `desktop/src/main.rs`, `design.rs`: preserved egui reference, not the packaged app.
- `desktop/src/i18n.rs`, `desktop/locales/en.json`: engine message translations.
- `desktop/src/worker.rs`: background workflow; keep network work off the UI thread.
- `desktop/src/web_session.rs`, `mywhoosh.rs`, `bin/whoosh-web-login.rs`: official
  MyWhoosh website session and WebKit helper. Preserve CAPTCHA and consent handling.
- `desktop/src/garmin.rs`, `desktop/bridge/garmin_bridge.py`: JSON-lines protocol,
  Garmin authentication, session persistence and upload outcome classification.
- `desktop/src/session_store.rs`: macOS Keychain credentials and private local device preferences.
- `desktop/src/fit*.rs`, `devices.rs`: conservative FIT editing and Edge profiles.
- `desktop/tests/device_catalog_audit.json`: device and firmware source evidence.

## Invariants

Preserve original activity measurements, unknown fields and accessory identities.
Use the selected Edge product and the user's Unit ID consistently for the recorder.
Never fabricate physiological metrics or promise Garmin will recalculate them.
Distinguish confirmed uploads, duplicates and unconfirmed receipts. Do not retry an
ambiguous upload automatically. Network failures must not clear valid sessions.
Never print tokens, credentials or raw service responses. Use synthetic test data.
Keep French and English UI strings synchronized.

## Commands (repository root)

```bash
python3.12 -m venv venv
venv/bin/python -m pip install -r desktop/requirements-test.txt
bash scripts/check.sh
WHOOSH_PYTHON="$(command -v python3.12)" bash desktop/scripts/build-macos.sh
bash scripts/package-app.sh
python3 scripts/prepare-source.py
```

Runtime, testing and packaging Python dependencies are separated in
`desktop/requirements-{runtime,test,build}.txt`. Keep Cargo.lock committed.
`dist/`, `desktop/target/` and virtual environments are local generated outputs.
The source exporter uses an explicit allowlist and works without `.git`.

## Publication

Never publish `.env`, real FIT/TCX files, logs, cookies, tokens or account captures.
Keep GPL v3 and upstream acknowledgements in THIRD_PARTY_NOTICES.md and the app.
Attach the packaged app ZIP to Releases; source archives are not installable apps.
The current build is ad-hoc signed, not Apple Developer ID signed or notarized.
Report local checks separately from GitHub CI and real Garmin import verification.
