# Lumen

A Windows VPN and proxy client for your own servers and subscriptions. Import a link, choose a server, and connect from a compact desktop window.

**[Download for Windows x64](https://github.com/Sincopa/lumen-ray/releases/latest)** · [Report a bug](https://github.com/Sincopa/lumen-ray/issues) · [Apache 2.0](LICENSE)

## Screenshots

<p align="center">
  <a href="docs/screenshots/home.png"><img src="docs/screenshots/home.png" width="195" alt="Home: connection controls and selected server" /></a>
  <a href="docs/screenshots/servers.png"><img src="docs/screenshots/servers.png" width="195" alt="Servers: countries, protocols and latency" /></a>
  <a href="docs/screenshots/subscriptions.png"><img src="docs/screenshots/subscriptions.png" width="195" alt="Subscriptions: profiles, remaining days and traffic" /></a>
  <a href="docs/screenshots/rules.png"><img src="docs/screenshots/rules.png" width="195" alt="Rules: route domains and applications" /></a>
</p>

<p align="center"><sub>The current interface with sample data in browser preview mode. Click to enlarge. PNGs preserve the window's transparent, rounded corners.</sub></p>

[Settings screenshot](docs/screenshots/settings.png)

## Features

- Import subscription URLs, individual connection keys, Base64 lists, Xray JSON, and Clash/Mihomo proxy lists.
- Search servers, measure latency, refresh subscriptions, and switch the selected server.
- Choose Windows system proxy or a TUN adapter, with global or rule-based routing.
- Route applications, domains, IP ranges, and GeoIP/GeoSite categories through Xray, directly, or to a blocking rule.
- Use English or Russian, adjust interface scale and background colors, and manage app and core updates.
- Keep the app in the system tray, enable startup connection, and use network diagnostics when troubleshooting.

Xray is the default engine. Supported imports include VLESS, VMess, Trojan, Shadowsocks, Hysteria 2, WireGuard, SOCKS, and HTTP proxies; availability depends on the selected engine and configuration. Unsupported provider extensions are reported during import. The experimental **Lumen Core** handles TCP SOCKS5 and HTTP CONNECT upstreams with domain/IP rules; use Xray for TUN, UDP, application rules, GeoIP/GeoSite, and other protocols.

## Install and connect

1. Download **Setup.exe** or **Portable.zip** from [Releases](https://github.com/Sincopa/lumen-ray/releases/latest).
2. Install the app, or extract the **entire** portable ZIP and keep `runtime` beside `Lumen.exe`.
3. Open **Subscriptions → Add profile** and paste your provider's URL or connection key.
4. Choose a server in **Servers**, then click the Lumen symbol on **Home**.
5. Open **Settings → Network → Working Mode** to choose system proxy or TUN.

Lumen does not include a VPN subscription. The portable build needs no development server or launcher script. Windows WebView2 is required; if it is missing, the app offers the [official download page](https://developer.microsoft.com/microsoft-edge/webview2/).

| Mode             | Traffic covered                                                                                      |
| ---------------- | ---------------------------------------------------------------------------------------------------- |
| **System Proxy** | Applications that honor the current user's Windows HTTP/HTTPS proxy settings.                        |
| **TUN Adapter**  | Traffic routed through the virtual adapter. Starting the TUN helper requires administrator approval. |

There is no Kill Switch. Browser preview mode uses demo services and does not route traffic. Protocol support and automated checks do not guarantee that a particular provider, network, or application will work; verify your traffic after changing routing settings.

Windows releases use self-signed Authenticode certificates. Other computers do not automatically trust them, so SmartScreen warnings may appear.

## Build from source

The interface uses **Svelte 5 + TypeScript**, the desktop shell uses **Tauri 2 + Rust**, and the default network engine is **Xray**. Native networking is available only inside Tauri.

On Windows, install Node.js **22.12+**, Rust **1.92+**, MSVC Build Tools with the Windows SDK, and WebView2. Then run:

```powershell
npm ci
powershell -NoProfile -File src-tauri/build/prepare-core.ps1
npm run tauri:dev
```

`prepare-core.ps1` downloads the pinned runtime and routing databases and checks their SHA-256 hashes. For interface development with demo services, use `npm run dev`.

```powershell
npm run build:installer
npm run build:portable
# Build both packages:
npm run build:release
```

Build outputs are written to `release/`. Windows builds use SDK SignTool and create or reuse a local self-signed certificate. The private key stays in the current user's Windows certificate store. See [sign-windows.ps1](src-tauri/build/sign-windows.ps1) for the signing commands.

The [GitHub Actions release workflow](.github/workflows/release.yml) verifies the frontend and backend before packaging a version tag. CI requires `WINDOWS_CERTIFICATE_PFX_BASE64`, `WINDOWS_CERTIFICATE_PASSWORD`, and `TAURI_SIGNING_PRIVATE_KEY` as repository secrets, plus the `WINDOWS_CERTIFICATE_THUMBPRINT` repository variable. The updater key is separate from the Windows certificate; keep both private. The configured updater public key is safe to distribute.

## Development and checks

```powershell
npm run check
npm run lint
npm run format:check
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

| Directory            | Contents                                                                   |
| -------------------- | -------------------------------------------------------------------------- |
| `src/`               | Interface, translations, browser demo services, and Tauri service adapters |
| `src-tauri/src/`     | Native engines, subscriptions, routing, storage, and Windows integration   |
| `src-tauri/runtime/` | Pinned network runtime, databases, and runtime licenses                    |
| `src-tauri/build/`   | Windows packaging, signing, runtime preparation, and diagnostics           |

Bug reports and focused pull requests are welcome. Include the app version, Windows version, steps to reproduce, and the checks you ran. Use sample configurations and remove credentials from logs and screenshots.

For vulnerabilities, use GitHub's private reporting option under **Security** if enabled. Otherwise, request a private contact in an issue without posting sensitive details. Revoke any exposed subscription links or signing keys at their source; deleting a file does not invalidate copied credentials.

## Data and license

On Windows, native application data is encrypted for the current user with DPAPI under `%LOCALAPPDATA%\app.lumen.desktop\data`. Startup diagnostics are written to `%LOCALAPPDATA%\app.lumen.desktop\logs\startup.log`, with `%TEMP%\lumen-startup.log` as a fallback. Never attach subscription links, connection credentials, or decrypted data to a public issue.

Lumen is licensed under [Apache 2.0](LICENSE). Bundled components retain their own licenses:

| Component                                                                   | License and source                                                                                                                                  |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Xray Core](https://github.com/XTLS/Xray-core)                              | [Mozilla Public License 2.0](src-tauri/runtime/licenses/xray.txt). The executable receives an Authenticode signature; upstream source is unchanged. |
| [Wintun 0.14.1](https://www.wintun.net/)                                    | [Prebuilt binary license](src-tauri/runtime/licenses/wintun.txt), preserved from the official 0.14.1 source tag.                                    |
| [Geist](https://github.com/vercel/geist-font)                               | [SIL Open Font License 1.1](src-tauri/runtime/licenses/geist.txt)                                                                                   |
| [Satoshi](https://www.fontshare.com/fonts/satoshi)                          | [ITF Free Font License](src/assets/fonts/Satoshi-LICENSE.txt)                                                                                       |
| [Material Symbols Rounded](https://github.com/google/material-design-icons) | Apache 2.0; [subset notice](src/assets/fonts/MaterialSymbolsRounded-subset-v375.README.txt)                                                         |
| [flag-icons](https://github.com/lipis/flag-icons)                           | [MIT, Panayiotis Lipiridis](src-tauri/runtime/licenses/flag-icons.txt)                                                                              |
| [lottie-web](https://github.com/airbnb/lottie-web)                          | [MIT, Bodymovin](src-tauri/runtime/licenses/lottie-web.txt)                                                                                         |
| [jsQR](https://github.com/cozmo/jsQR)                                       | Apache 2.0                                                                                                                                          |

Routing databases come from [NetworkKeeper/v2ray-rules-ru](https://github.com/NetworkKeeper/v2ray-rules-ru), which lists its upstream data sources in its README. That aggregator has no top-level license file; Lumen's license does not relicense the datasets. The download versions and hashes are pinned in [prepare-core.ps1](src-tauri/build/prepare-core.ps1).

Other dependency versions and their upstream packages are recorded in [package-lock.json](package-lock.json) and [Cargo.lock](src-tauri/Cargo.lock). Preserve their original notices when redistributing them. Windows WebView2 and the Windows SDK are Microsoft components with separate terms.
