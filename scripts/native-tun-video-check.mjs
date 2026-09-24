// Read-only playback check over the current Windows route. Requires an already active TUN session.
import { chromium } from '@playwright/test';
import { execFileSync } from 'node:child_process';

function verifyRoute() {
	if (process.platform !== 'win32') throw new Error('Windows TUN check only');
	const check = `$tun = Get-NetAdapter -Name 'LumenTun' -ErrorAction SilentlyContinue;
$v4 = Find-NetRoute -RemoteIPAddress '8.8.8.8' -ErrorAction SilentlyContinue;
$v6 = Find-NetRoute -RemoteIPAddress '2606:4700:4700::1111' -ErrorAction SilentlyContinue;
$dns = Resolve-DnsName 'www.youtube.com' -DnsOnly -ErrorAction SilentlyContinue;
if ($tun.Status -ne 'Up' -or
    @($v4.InterfaceAlias) -notcontains 'LumenTun' -or
    @($v6.InterfaceAlias) -notcontains 'LumenTun' -or
    @($dns | Where-Object { $_.Type -in @('A', 'AAAA') }).Count -eq 0) { exit 1 }`;
	execFileSync('powershell.exe', ['-NoProfile', '-Command', check], {
		windowsHide: true,
		stdio: 'ignore',
	});
}

verifyRoute();
const browser = await chromium.launch({
	headless: true,
	channel: 'msedge',
	args: ['--autoplay-policy=no-user-gesture-required'],
});
let verified = 0;
try {
	const page = await browser.newPage();
	page.setDefaultTimeout(10_000);
	for (const id of ['aqz-KE-bpKQ', 'jNQXAC9IVRw']) {
		const mediaStatuses = [];
		const collect = (response) => {
			if (new URL(response.url()).hostname.endsWith('.googlevideo.com')) {
				mediaStatuses.push(response.status());
			}
		};
		page.on('response', collect);
		try {
			await page.goto(`https://www.youtube.com/watch?v=${id}`, {
				waitUntil: 'domcontentloaded',
				timeout: 45_000,
			});
			await page.locator('video').waitFor({ timeout: 20_000 });
			await page.locator('video').evaluate((video) => {
				video.muted = true;
				return video.play().catch(() => {});
			});
			await page.waitForFunction(
				() => document.querySelector('video')?.currentTime > 3,
				null,
				{
					timeout: 30_000,
				},
			);
			const mediaOk = mediaStatuses.some(
				(status) => status === 200 || status === 206,
			);
			if (mediaOk) verified++;
			console.log(JSON.stringify({ id, played: true, mediaOk }));
		} catch {
			console.log(
				JSON.stringify({
					id,
					played: false,
					mediaStatuses: mediaStatuses.slice(-5),
				}),
			);
		} finally {
			page.off('response', collect);
		}
	}
} finally {
	await browser.close();
}
verifyRoute();
if (verified !== 2) process.exitCode = 1;
