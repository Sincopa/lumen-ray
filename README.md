# Lumen

Desktop VPN client for Windows x86_64, built with Svelte 5, TypeScript, Tauri 2 and
Xray. The existing interface is preserved. Browser development uses explicit mocks;
the Tauri runtime uses the Rust backend and never falls back to fake connections.

**Release validation is not complete.** Real proxy connectivity and native UI wiring
have been exercised. Global/application TUN traffic verification is currently blocked
by another active VPN (Meta) installing more-specific routes. Do not interpret the
working proxy mode or a successful build as proof of full-system VPN protection.

## Development and builds

Requirements: Node.js 22.12+, Rust 1.92+, MSVC Build Tools, Windows SDK and WebView2.

```powershell
npm ci
powershell -NoProfile -File scripts/prepare-core.ps1
npm run dev
# Native development:
npm run tauri:dev
# Windows installer, including verified Xray and Wintun:
node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis
```

The core preparation script downloads the pinned official Xray v26.9.9 archive and
checks its SHA-256. Binary resources are intentionally excluded from Git; the script
must run before compiling a fresh checkout. The bundled license and hash manifest
are in `src-tauri/resources/core`. The installer currently has no Authenticode signature.
Port 5173 is strict to avoid displaying a different project in the native window.

## Backend Architecture

- `src/lib/services`: existing service contracts and runtime selection.
- `src/lib/platform/tauri`: catalog, connection and statistics IPC adapters.
- `src-tauri/src/commands`: thin IPC handlers; no shell or filesystem access from Svelte.
- `app/state.rs`: encrypted repository, singleton lock, lifecycle operation gate,
  state/event channels and active core version.
- `subscription`: bounded fetching, URI/Base64/Xray JSON/Clash YAML parsers.
- `domain`: validated nodes and typed protocol/transport/security data.
- `xray`: deterministic configuration, bundled hash verification and staged updater.
- `services`: catalog transactions, lifecycle, concurrency-limited HTTPS latency probes,
  background public-IP detection and Xray traffic counters.
- `platform`: Windows process enumeration, DPAPI integration, proxy journal, TUN helper
  and route inspection. Other operating systems are not shipped or runtime-tested.

### Lifecycle and Windows networking

Each connection is generated and passed directly to the managed Xray process over stdin;
startup failure is detected before the connection is published. Separate `xray run -test`
validation remains in the core-update safety path. One connection operation owns the
session; temporary latency probes have separate ports, Job Objects and a four-probe
limit. The backend emits state/statistics events, detects process exits and rejects
common competing-VPN routes before reporting TUN Connected.

TUN elevates a restricted, session-scoped helper through UAC; the GUI remains a normal
user process. Xray owns the LumenTun adapter, IPv4/IPv6 default routes and automatic
outbound-interface binding. Global uses the selected proxy; Rules defaults to DIRECT
and applies enabled process-name/path, domain and IP/CIDR rules. Loopback/private LAN
and Xray itself bypass the tunnel. Rule order is preserved.

Proxy mode changes the current user's Windows HTTP/HTTPS proxy after verifying the
selected server. It affects proxy-aware applications only. Process routing requires
TUN. Original registry/PAC values are DPAPI-journaled and restored on disconnect or
startup recovery; third-party changes made after activation are preserved.

Core shutdown sends a console break signal and waits for graceful exit, with bounded
forced termination and Job Object cleanup as a fallback. Native proxy crash detection
and startup proxy recovery passed; the TUN crash/route/DNS recovery matrix remains open.
Kill Switch is deliberately not implemented. Auto-connect reconnects the saved selected
node when the application starts; it does not install Windows autorun.

### Subscriptions and storage

HTTPS fetches have a 30-second timeout, five-redirect and 4 MiB body limit. Imports
accept plain URI lists, one Base64 layer, full Xray configurations/outbound arrays and
Clash/Mihomo `proxies` YAML. At most 4096 entries are accepted. YAML file includes are
disabled. Unsupported entries have indexed typed diagnostics and do not invalidate
other supported entries. Refresh replaces disappeared nodes and preserves stable selections.

Supported protocol families: VLESS (TLS/REALITY/Vision), VMess, Trojan, Shadowsocks,
Hysteria 2 and typed Xray WireGuard outbounds. Transports include RAW/TCP, WebSocket,
gRPC, HTTPUpgrade, basic XHTTP and mKCP. Not every provider extension is supported:
advanced XHTTP `extra`, transport masks, chained proxies, unsafe TLS and unknown
security-sensitive parameters are rejected instead of silently dropping them.

The versioned native repository is stored under `%LOCALAPPDATA%/app.lumen.desktop/data`
as user-scoped DPAPI ciphertext, including subscriptions, credentials, selected node,
rules and network settings. Writes are atomic. Missing optional fields migrate via
schema defaults; unknown future versions fail explicitly. Browser localStorage catalogs
are separate and never seed the native runtime. Appearance/language preferences remain
in the original frontend storage.

The existing interface has no IP/speed panel or updater screen. Real IP/statistics and
safe updater commands are available through the service/IPC layer; new visual controls
were not added. Process paths and executable icons come from Windows; inaccessible
icons use the existing fallback.

### Updates

Core updates use fixed official XTLS GitHub release endpoints, architecture-specific
assets and mandatory SHA-256 metadata. Downloads are bounded, ZIP paths are checked,
files are staged and validated, and activation stores current/previous version pointers.
A failed reconnect restores the previous core. An updated core requires fresh official
metadata on startup/helper launch; offline startup falls back to the verified bundled core.

Application updates use the standard Tauri signature verifier and the latest signed
GitHub Release. `.github/workflows/release.yml` builds the Windows installer, updater
artifact and `latest.json` when a `v*` tag is pushed. The private signing key stays in
GitHub Actions Secrets; the matching public key is compiled into release builds.
See [RELEASING.md](RELEASING.md) for the one-time repository setup and release command.
See [SECURITY.md](SECURITY.md) for trust boundaries and remaining limitations.

## Verification

```powershell
npm run check
npm run lint
npm run format:check
npm test
npm run build
npm run test:e2e
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml
```

Network tests are opt-in. Set `LUMEN_TEST_SUBSCRIPTION_FILE` to a local file containing
a private subscription; never paste credentials into tests or commit the file.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml private_ -- --ignored --nocapture --test-threads=1
# Native WebView2 smoke; uses isolated debug application data:
cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
npm run test:smoke
# Elevated TUN traffic smoke, after disconnecting competing VPNs:
.\src-tauri\target\debug\app.exe --lumen-smoke-tun
```

Verified locally: unit tests; bundled-core validation (including Hysteria/WireGuard/mKCP);
private subscription fetch and generated-config validation; real HTTPS through a selected
node with nonzero counters; native empty first-run, import, connect, events, reload,
disconnect and clean exit; exact restoration of Windows proxy settings; owned-core
crash detection and proxy journal recovery after forced GUI termination. A local NSIS
install/launch/uninstall passed; this does not substitute for a clean-machine test.

Still unverified or incomplete: clean-machine installer operation; complete Global and
selected-application TUN traffic/IPv6 proof; UAC refusal and crash recovery matrix;
end-to-end core rollback and signed application delivery. There is no production-ready
claim until these checks pass. Browser UI screenshots are stored in ignored
`artifacts/backend-before` and `artifacts/backend-after` directories.
The working tree also contains a separate font replacement; these screenshots are
therefore not evidence of pixel equality with the original typography. Formatting
checks currently report existing changes in `src/assets/fonts.css` and two E2E files
(`capture-rules-with-items.spec.ts`, `dropdown-behavior.spec.ts`).

For frontend performance methodology, see [PERFORMANCE.md](PERFORMANCE.md).
API references: [Tauri IPC](https://v2.tauri.app/develop/calling-rust/),
[Xray TUN](https://xtls.github.io/en/config/inbounds/tun.html),
[Xray routing](https://xtls.github.io/en/config/routing.html),
[Tauri updater](https://v2.tauri.app/plugin/updater/).
