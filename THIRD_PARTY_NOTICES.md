# Third-party acknowledgements

Whoosh Bridge is a modified/extended version of the GPL v3 repository
[marcelorodrigo/mywhoosh-to-garmin](https://github.com/marcelorodrigo/mywhoosh-to-garmin).
Its historical Python CLI, services and tests have been removed; its origin and
GPL v3 license are retained.
The Rust desktop app, WebKit integration, Keychain persistence, metadata editor,
bilingual UI and original logo were added in November 2026.

## Reference implementations and specifications

- [jdelrue/mywoosh2garmin](https://github.com/jdelrue/mywoosh2garmin), GPL v3:
  `fitfix.go` was consulted for missing session average repair, removal of virtual
  temperatures and device spoofing. The Rust implementation preserves accessory
  and opaque fields and uses the selected Edge profile with the user's Unit ID,
  not that project's fixed Fenix/serial values. The project is credited as a
  behavioral reference; its Garmin metric-calculation claims are not guarantees.
- [Garmin FIT Python SDK](https://github.com/garmin/fit-python-sdk): the official
  profile was consulted to verify product identifiers, field numbers, base types
  and scales. The SDK is not embedded as the desktop FIT parser. The small audit
  fixture records verified identifiers and links to firmware announcements.
- The official MyWhoosh website is opened in WebKit for sign-in. Its website code
  is not vendored. The app's scripts observe its session state and call its site
  actions; they do not bypass CAPTCHA or consent.

## Direct runtime/build dependencies

| Project | Use | License/reference |
|---|---|---|
| [python-garminconnect](https://github.com/cyberjunky/python-garminconnect) | Garmin API wrapper, Python sidecar | MIT |
| [garth](https://github.com/matin/garth) | Authentication and token refresh through garminconnect | MIT |
| [egui / eframe](https://github.com/emilk/egui) | Rust desktop UI | MIT or Apache-2.0 |
| [Wry](https://github.com/tauri-apps/wry) | Native WebKit window | MIT or Apache-2.0 |
| [Tao](https://github.com/tauri-apps/tao) | Web helper event loop | MIT or Apache-2.0 |
| [rust-security-framework](https://github.com/kornelski/rust-security-framework) | macOS Keychain | MIT or Apache-2.0 |
| [PyInstaller](https://github.com/pyinstaller/pyinstaller) | Bundling the Python sidecar | GPL with the PyInstaller bootloader exception; see its license |

Other Rust dependencies are resolved in `desktop/Cargo.lock`. Python build/runtime
versions are pinned in `desktop/requirements-runtime.txt`, with packaging and
test additions in `desktop/requirements-build.txt` and `desktop/requirements-test.txt`.
`fit-tool` is used only for independent FIT validation, not at runtime.

The packaging script collects available dependency license/notice files into the
app's `Contents/Resources/licenses` directory, including Rust dependency licenses
and Python distribution notices. Those dependencies retain their own licenses;
the root GPL license does not replace them.

Garmin and MyWhoosh names/trademarks remain the property of their respective
owners. No endorsement or official partnership is claimed. The blue bridge icon
is original project artwork, not a Garmin or MyWhoosh logo.

## Sparkle

The macOS app embeds [Sparkle](https://sparkle-project.org/), the open-source
software update framework, pinned in `desktop/update-config.json`.
Sparkle is distributed under its permissive license; the complete upstream
license and bundled component notices are included in the app at
`Contents/Resources/Sparkle-LICENSE.txt` and in the framework's resources.
The update signing key is independent of Apple Developer ID signing.
