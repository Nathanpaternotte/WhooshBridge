# Changelog

## 0.1.0-alpha.4

### Added

- Optional active installation counting through PostHog EU.
- A startup consent dialog with equally accessible Accept and Decline actions. Both choices are remembered and can be changed from the app menu.
- French and English consent text and settings.

### Privacy

- No telemetry is sent before acceptance. Events contain a random installation identifier and the app version, with at most one attempt per UTC day.
- No Garmin or MyWhoosh account data, Unit IDs or activity data is included. Person profiles and geographic enrichment are disabled.
- Telemetry runs in a background thread with a short timeout and private local state.

### Validation

- Local Rust, Python, JavaScript and FIT checks passed, including consent persistence, legacy preference migration, daily event limits and telemetry payload tests.
- A synthetic integration event was accepted by the PostHog EU endpoint (HTTP 200); dashboard ingestion was not independently verified.
- GitHub CI and delivery of this release through the public update feed remain to be verified.

### Installation notes

- Version 0.1.0-alpha.4 uses Sparkle build 4. Existing Sparkle-enabled installations can discover it after the exact release ZIP and signed appcast are published.
- This Apple Silicon build remains ad-hoc signed and is not Apple-notarized.

## 0.1.0-alpha.3

### Added

- In-app updates powered by Sparkle. Open **••• → Check for updates…** to check manually, or enable daily automatic checks.
- Download, install and relaunch from the update dialog, with relaunch deferred while an activity operation is running.
- English and French update controls. Sparkle's native dialogs follow macOS language preferences.
- Signed update feeds and archives, with verification before extraction. The private signing key stays in the maintainer's macOS Keychain.
- Architecture-specific update feeds, release preparation tools and a documented publication procedure.

### Updated

- macOS packaging now embeds Sparkle and its license, and uses an increasing build number for update detection.
- Privacy documentation explains update requests to GitHub. No Garmin/MyWhoosh credentials or activity data are sent with update checks; system profiling is disabled.
- Source exports include the updater, release configuration and update documentation.

### Validation

- Local Rust, Python, JavaScript and FIT checks passed.
- Native updater tests cover deferred relaunch, single resume and recovery after an aborted update.
- An isolated Apple Silicon test app successfully downloaded, installed and relaunched a signed update.
- The release ZIP and feed signatures were verified locally. GitHub CI, public update delivery and Intel installation have not been verified for this release.

### Installation notes

- Users of alpha.1 or alpha.2 must install alpha.3 manually once to receive future updates in the app.
- Publish the release ZIP before publishing its signed update feed. Update checks cannot succeed until the feed is available on GitHub.
- This release remains ad-hoc signed and is not notarized by Apple. First-install macOS warnings and renewed Keychain access prompts after an update may still occur.
