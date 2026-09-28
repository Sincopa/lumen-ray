import { spawnSync } from 'node:child_process';
// Real bundled Xray + loopback HTTP via SOCKS. No system proxy, routes or user data.
if (process.platform !== 'win32') throw new Error('Native smoke requires Windows');
const result = spawnSync('cargo', ['test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', 'native_local_proxy_smoke', '--', '--nocapture'], { encoding: 'utf8', timeout: 300000, windowsHide: true });
process.stdout.write(result.stdout ?? '');
process.stderr.write(result.stderr ?? '');
if (result.error || result.status !== 0 || !result.stdout?.includes('1 passed')) process.exit(1);
console.log('NATIVE_LOCAL_PROXY_OK (TUN and external provider traffic are separate checks)');
