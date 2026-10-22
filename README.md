# Lumen

Windows and macOS app for a VPN or proxy subscription you already have.

**[Download for Windows x64](https://github.com/Sincopa/lumen-ray/releases/latest)** · **[Download for macOS](https://github.com/Sincopa/lumen-ray/releases/latest)** · [Report a bug](https://github.com/Sincopa/lumen-ray/issues) · [Proprietary (All Rights Reserved)](LICENSE)

## Screenshots

<p align="center">
  <a href="docs/screenshots/empty.png"><img src="docs/screenshots/empty.png" width="195" alt="No subscription yet: paste a link" /></a>
  <a href="docs/screenshots/home.png"><img src="docs/screenshots/home.png" width="195" alt="Home" /></a>
  <a href="docs/screenshots/connected.png"><img src="docs/screenshots/connected.png" width="195" alt="Connected" /></a>
  <a href="docs/screenshots/servers.png"><img src="docs/screenshots/servers.png" width="195" alt="Servers" /></a>
</p>
<p align="center">
  <a href="docs/screenshots/subscriptions.png"><img src="docs/screenshots/subscriptions.png" width="195" alt="Subscriptions" /></a>
  <a href="docs/screenshots/rules.png"><img src="docs/screenshots/rules.png" width="195" alt="Rules" /></a>
  <a href="docs/screenshots/settings.png"><img src="docs/screenshots/settings.png" width="195" alt="Settings" /></a>
  <a href="docs/screenshots/settings-network.png"><img src="docs/screenshots/settings-network.png" width="195" alt="Network settings" /></a>
</p>

No subscription yet, home, connected and servers; then subscriptions, rules and settings.

## Start

1. Download **Setup.exe** or **Portable.zip** from [Releases](https://github.com/Sincopa/lumen-ray/releases/latest).
2. Run Setup, or unpack the whole ZIP into a folder. `runtime` has to stay next to `Lumen.exe`. Do not launch the program from inside the archive.
3. Open **Subscriptions → Add profile** and paste the link or key from your provider.
4. Choose a server, then click the Lumen mark on **Home**.

Lumen does not include a subscription.

**System proxy** is enough for browsers and other programs that follow the Windows proxy setting. **TUN** covers more traffic. Windows asks for administrator approval when the adapter starts. Switch this in **Settings → Network → Working Mode**. There is no kill switch: disconnect in the app when you want traffic to go out normally again.

The first launch can show a SmartScreen warning. The app is signed with a certificate that other computers do not trust yet.

If a message says WebView2 is missing, install it from [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/).

### macOS

1. Download the **.dmg** from [Releases](https://github.com/Sincopa/lumen-ray/releases/latest). One build runs on Apple Silicon and Intel, macOS 13 or newer.
2. Open it and drag **Lumen** to **Applications**, then start it from there.
3. Add a profile and connect the same way as on Windows.

Only **System proxy** works on macOS for now. It covers browsers and other programs that follow the macOS proxy settings. TUN is not available on macOS yet.

## If something breaks

Open an [issue](https://github.com/Sincopa/lumen-ray/issues) with the app version, your Windows or macOS version, and what you clicked. Do not paste a subscription link, a key, or a password.

## Build it yourself

Node.js 22.12 or newer, Rust 1.92 or newer, and Visual Studio Build Tools with the Windows SDK.

```powershell
npm ci
powershell -NoProfile -File src-tauri/build/prepare-core.ps1
npm run tauri:dev
```

`npm run build:release` writes the installer and the portable ZIP to `release/`.

On macOS: Node.js 22.12 or newer, Rust 1.92 or newer with both Apple targets, and the Xcode Command Line Tools.

```sh
npm ci
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri:dev:macos
```

`tauri:dev:macos` first builds the verified universal Xray core into `src-tauri/runtime/xray/xray`; plain `cargo` commands need it too (`npm run prepare:core:macos`). `npm run tauri:build:macos` writes `Lumen.app` and the disk image to `src-tauri/target/universal-apple-darwin/release/bundle/`. Set `APPLE_SIGNING_IDENTITY` to sign the core and the app with a Developer ID, and `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID` to notarize.

Proprietary, All Rights Reserved. See [LICENSE](LICENSE). Xray, Wintun, and the fonts keep their own license files in the repository.
