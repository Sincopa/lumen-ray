// Invoked by the opt-in Rust private_geo_video_check test. No credentials in output.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
const [core, assets, configFile, mode] = process.argv.slice(2);
const config = JSON.parse(readFileSync(configFile, 'utf8'));
const port = config.inbounds.find((i) => i.tag === 'system-proxy').port;
const child = spawn(core, ['run', '-config', 'stdin:'], {
	windowsHide: true,
	env: { ...process.env, XRAY_LOCATION_ASSET: assets },
	stdio: ['pipe', 'pipe', 'pipe'],
});
child.stdin.end(JSON.stringify(config));
const routes = new Set();
let log = '';
let played = 0;
child.stdout.on('data', (b) => { log += b.toString(); });
child.stderr.on('data', (b) => { log += b.toString(); });
let browser;
try {
	await new Promise((resolve) => setTimeout(resolve, 800));
	browser = await chromium.launch({
		headless: true,
		channel: 'msedge',
		proxy: { server: `http://127.0.0.1:${port}` },
		args: ['--autoplay-policy=no-user-gesture-required'],
	});
	const page = await browser.newPage();
	page.setDefaultTimeout(5000);
	const media = [];
	page.on('response', (response) => {
		const url = new URL(response.url());
		if (url.hostname.endsWith('.googlevideo.com')) {
			media.push({ host: url.hostname, status: response.status() });
		}
	});
	page.on('requestfailed', (request) => {
		const host = new URL(request.url()).hostname;
		if (/youtube|googlevideo|ytimg/.test(host)) {
			routes.add(JSON.stringify({ host, error: request.failure()?.errorText }));
		}
	});
	for (const id of ['aqz-KE-bpKQ', 'jNQXAC9IVRw']) {
		try {
			await page.goto(`https://www.youtube.com/watch?v=${id}`, { waitUntil: 'domcontentloaded', timeout: 45000 });
			const reject = page.getByRole('button', { name: /Reject all|Отклонить все/ });
			if (await reject.first().isVisible()) await reject.first().click();
			await page.locator('video').waitFor({ timeout: 20000 });
			await page.locator('video').evaluate((v) => { v.muted = true; return v.play().catch(() => {}); });
			await page.waitForFunction(() => document.querySelector('video')?.currentTime > 3, null, { timeout: 30000 });
			const state = await page.locator('video').evaluate((v) => ({ time: v.currentTime, ready: v.readyState, width: v.videoWidth, error: v.error?.code }));
			played++;
			console.log(JSON.stringify({ mode, id, state, media: media.slice(-5) }));
		} catch (error) {
			const status = await page.locator('video').evaluate((v) => ({ time: v.currentTime, ready: v.readyState, error: v.error?.code })).catch(() => null);
			const playerError = await page.locator('.ytp-error-content-wrap').innerText().catch(() => '');
			console.log(JSON.stringify({ mode, id, error: error.name, status, playerError, media: media.slice(-5) }));
		}
	}
} finally {
	await browser?.close();
	child.kill();
	await new Promise((resolve) => child.exitCode !== null ? resolve() : child.once('exit', resolve));
	for (const line of log.split('\n')) {
		const match = line.match(/taking detour \[([^\]]+)\] for \[(tcp|udp):([^:\]]+):([0-9]+)\]/);
		if (match && /youtube|googlevideo|ytimg|ggpht|yandex/.test(match[3])) {
			routes.add(JSON.stringify({ outbound: match[1] === 'direct' ? 'direct' : 'vpn', protocol: match[2], host: match[3], port: match[4] }));
		}
	}
		console.log(JSON.stringify({ mode, routes: [...routes].map((r) => JSON.parse(r)) }));
		if (mode === 'fixed-rules') {
			const routeEntries = [...routes].map((r) => JSON.parse(r));
			const mediaRoutedDirect = routeEntries.some((route) =>
				route.outbound === 'direct' && /youtube|googlevideo|ytimg/.test(route.host),
			);
			if (played === 0 || mediaRoutedDirect) process.exitCode = 1;
		}
}
