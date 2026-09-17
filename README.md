# Lumen

Modern, ultra-lightweight desktop VPN & proxy interface built with Svelte 5, TypeScript, Vite and Tauri 2.

**This version is a demo. It does not protect or route traffic.** Browser and native
Tauri modes simulate connections. Xray, system proxy, TUN, Kill Switch and
auto-connect are not implemented and their controls are disabled.

## Development

- Node.js 22.12 or newer, npm.
- Native Windows build: Rust stable, MSVC Build Tools and Windows SDK, WebView2.
- Install dependencies: `npm ci`.
- Browser development: `npm run dev` (127.0.0.1:5173).
- Desktop development: `npm run tauri:dev`.
- Production frontend: `npm run build`, then `npm run preview`.
- Desktop bundle: `npm run tauri:build`.

Port 5173 is strict: stop the conflicting server rather than silently opening
another project in the Tauri window.

## Architecture

- `src/app/App.svelte`: navigation and composition.
- `src/lib/features`: connection, subscription, server, rule and settings screens.
- `src/lib/stores/connection.ts`: lifecycle controller, timeouts and errors.
- `src/lib/services/catalog.ts`: validated demo catalog and persistence.
- `src/lib/services/contracts`: service contracts.
- `src/lib/platform/browser`: simulated connection adapter.
- `src/lib/platform/tauri`: typed IPC adapter.
- `src-tauri/src/vpn.rs`: simulated native commands and connection state.
- `src/lib/i18n`, `src/locales`: Russian and English.

The catalog is the shared demo implementation in both environments; native
connection state lives in Rust. The frontend never starts processes or changes
system networking. A future Xray integration should implement these contracts,
add backend events for unexpected disconnects and validate native data.

## Demo behavior and storage

Server selection, routing mode, subscriptions and rules survive navigation and
restart using versioned local storage. Invalid stored catalogs fall back to a
sample catalog and display a warning. Failed writes do not publish unsaved changes.
Language and appearance are stored separately. Browser connection state is
session-only; native connection state lasts for the application process.

Subscription import validates an HTTPS URL, supported share link or Base64 list.
It creates sample metadata and one simulated server; it does not fetch a provider
or parse an actual server configuration. Input URLs and credentials are not saved
or included in exports. Do not use this demo as a credential manager.

Rules accept a domain without a path or an executable filename. They can be
edited, toggled and deleted; they do not affect traffic yet. Removing a subscription
also removes its servers and dependent rules, with confirmation.

The background supports reduced motion, a manual off switch, hidden-window
pausing, WebGL context restoration and a 30 FPS limit. Fonts and icons are local.

## Verification

- `npm run check`: Svelte and TypeScript.
- `npm run lint`: ESLint for TypeScript and Svelte.
- `npm run format:check` / `npm run format`: Prettier.
- `npm test`: unit tests for lifecycle, persistence, validation and locales.
- `npm run build && npm run test:e2e`: production-browser scenarios in local Edge.
- `cargo test --locked --manifest-path src-tauri/Cargo.toml`: native command tests.

Browser tests use isolated contexts and port 5187. They do not modify system
VPN settings. They require Microsoft Edge. The production CSP blocks external
assets and network endpoints; native IPC is explicitly permitted.

Native compilation and command tests are not proof of real VPN networking.
Before a public release, implement and test the network backend, secret storage,
crash recovery and installer behavior, and choose application signing and licensing.

API references: [Tauri commands](https://v2.tauri.app/develop/calling-rust/),
[window customization](https://v2.tauri.app/learn/window-customization/),
[CSP](https://v2.tauri.app/security/csp/).
