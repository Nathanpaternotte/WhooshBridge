# Changelog

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
