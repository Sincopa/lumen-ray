# Lumen

Windows app for a VPN or proxy subscription you already have.

**[Download for Windows x64](https://github.com/Sincopa/lumen-ray/releases/latest)** · [Report a bug](https://github.com/Sincopa/lumen-ray/issues) · [Apache 2.0](LICENSE)

## Screenshots

<p align="center">
  <a href="docs/screenshots/home.png"><img src="docs/screenshots/home.png" width="195" alt="Home" /></a>
  <a href="docs/screenshots/servers.png"><img src="docs/screenshots/servers.png" width="195" alt="Servers" /></a>
  <a href="docs/screenshots/subscriptions.png"><img src="docs/screenshots/subscriptions.png" width="195" alt="Subscriptions" /></a>
  <a href="docs/screenshots/rules.png"><img src="docs/screenshots/rules.png" width="195" alt="Rules" /></a>
</p>

## Start

1. Download **Setup.exe** or **Portable.zip** from [Releases](https://github.com/Sincopa/lumen-ray/releases/latest).
2. Run Setup, or unpack the whole ZIP into a folder. `runtime` has to stay next to `Lumen.exe`. Do not launch the program from inside the archive.
3. Open **Subscriptions → Add profile** and paste the link or key from your provider.
4. Choose a server, then click the Lumen mark on **Home**.

Lumen does not include a subscription.

**System proxy** is enough for browsers and other programs that follow the Windows proxy setting. **TUN** covers more traffic. Windows asks for administrator approval when the adapter starts. Switch this in **Settings → Network → Working Mode**. There is no kill switch: disconnect in the app when you want traffic to go out normally again.

The first launch can show a SmartScreen warning. The app is signed with a certificate that other computers do not trust yet.

If a message says WebView2 is missing, install it from [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/).

## If something breaks

Open an [issue](https://github.com/Sincopa/lumen-ray/issues) with the app version, your Windows version, and what you clicked. Do not paste a subscription link, a key, or a password.

## Build it yourself

Node.js 22.12 or newer, Rust 1.92 or newer, and Visual Studio Build Tools with the Windows SDK.

```powershell
npm ci
powershell -NoProfile -File src-tauri/build/prepare-core.ps1
npm run tauri:dev
```

`npm run build:release` writes the installer and the portable ZIP to `release/`.

Apache 2.0. Xray, Wintun, and the fonts keep their own license files in the repository.
