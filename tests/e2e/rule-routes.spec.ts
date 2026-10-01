import { test, expect } from '@playwright/test';
import { initialCatalog } from '../../src/lib/services/catalog';
import ru from '../../src/locales/ru.json' with { type: 'json' };

for (const width of [390, 293]) {
	test(`rule routes remain visible without latency at width ${width}`, async ({
		page,
	}, testInfo) => {
		const catalog = initialCatalog();
		catalog.servers[0].name = 'Финляндия — основной сервер';
		catalog.servers[0].latency = 'timeout';
		catalog.subscriptions.push({
			id: 'other',
			name: 'Другая подписка',
			updatedAt: 0,
		});
		catalog.rules = [
			{
				id: 'app',
				name: 'Discord.exe',
				matcher: 'application',
				serverId: 'demo-fi',
				enabled: true,
			},
			{
				id: 'app-direct',
				name: 'Telegram.exe',
				matcher: 'application',
				serverId: 'direct',
				enabled: true,
			},
			{
				id: 'app-sub',
				name: 'Code.exe',
				matcher: 'application',
				serverId: 'other',
				enabled: true,
			},
			{
				id: 'direct',
				name: 'private',
				matcher: 'geoip',
				serverId: 'direct',
				enabled: true,
			},
			{
				id: 'block',
				name: 'заблокировано.рф',
				matcher: 'domain',
				serverId: 'block',
				enabled: true,
			},
			{
				id: 'sub',
				name: 'antifilter-community',
				matcher: 'geosite',
				serverId: 'subscription',
				enabled: true,
			},
		];
		await page.addInitScript((value) => {
			if (!localStorage.getItem('lumen_catalog_v1')) {
				localStorage.setItem('lumen_catalog_v1', JSON.stringify(value));
			}
		}, catalog);
		await page.setViewportSize({ width, height: 844 });
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await page.goto('/');
		await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
		await page.evaluate(() => document.fonts.ready);
		const rows = page.locator('.rule-row');
		await expect(rows).toHaveCount(6);
		await expect(rows.nth(0).locator('.server-pill')).toContainText(
			catalog.servers[0].name!,
		);
		await expect(rows.nth(0).locator('.server-pill')).toHaveAttribute(
			'title',
			catalog.servers[0].name!,
		);
		await expect(rows.nth(0).locator('.flag-sm')).toBeVisible();
		await expect(rows.nth(1).locator('.server-pill')).toContainText(
			ru.rules.direct,
		);
		await expect(rows.nth(2).locator('.server-pill')).toContainText(
			'Другая подписка',
		);
		await expect(rows.nth(3).locator('.server-pill')).toContainText(
			ru.rules.direct,
		);
		await expect(rows.nth(4).locator('.server-pill')).toContainText(
			ru.rules.block,
		);
		await expect(rows.nth(5).locator('.server-pill')).toContainText('Tun');

		// Check the actual loaded font's glyph height, plus row/container bounds.
		const geometry = await rows.evaluateAll((elements) => {
			const canvas = document.createElement('canvas');
			const context = canvas.getContext('2d')!;
			return elements.flatMap((row) => {
				const rowBox = row.getBoundingClientRect();
				return [
					...row.querySelectorAll<HTMLElement>(
						'.rule-name, .rule-details, .server-pill-name',
					),
				]
					.filter((label) => getComputedStyle(label).display !== 'none')
					.map((label) => {
						const style = getComputedStyle(label);
						context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
						const metrics = context.measureText(label.textContent!.trim());
						const box = label.getBoundingClientRect();
						return {
							glyphsFit:
								metrics.actualBoundingBoxAscent +
									metrics.actualBoundingBoxDescent <=
								box.height,
							verticalFit: label.scrollHeight <= label.clientHeight + 1,
							insideRow:
								box.top >= rowBox.top &&
								box.bottom <= rowBox.bottom &&
								box.left >= rowBox.left &&
								box.right <= rowBox.right,
						};
					});
			});
		});
		for (const label of geometry) {
			expect(label).toEqual({
				glyphsFit: true,
				verticalFit: true,
				insideRow: true,
			});
		}
		expect(
			await page.evaluate(() => document.documentElement.scrollWidth),
		).toBeLessThanOrEqual(width);
		await page.screenshot({
			path: testInfo.outputPath(`rule-routes-${width}.png`),
		});

		await rows.nth(0).locator('.server-pill').click();
		await page
			.locator('.server-row')
			.filter({ hasText: ru.servers.countries.nl })
			.click();
		await expect(rows.nth(0).locator('.server-pill')).toContainText(
			ru.servers.countries.nl,
		);
		await page.reload();
		await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
		await expect(rows.nth(0).locator('.server-pill')).toContainText(
			ru.servers.countries.nl,
		);
	});
}
