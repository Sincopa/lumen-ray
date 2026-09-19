import { test, expect } from '@playwright/test';
import ru from '../../src/locales/ru.json' with { type: 'json' };
import { initialCatalog } from '../../src/lib/services/catalog';

declare global {
	interface Window {
		performanceTest: { timers: Set<number>; catalogWrites: number };
	}
}

test.beforeEach(async ({ page }) => {
	await page.addInitScript(() => {
		window.performanceTest = { timers: new Set(), catalogWrites: 0 };
		const interval = window.setInterval;
		const clear = window.clearInterval;
		window.setInterval = (handler, timeout, ...args) => {
			const id = interval(handler, timeout, ...args);
			if (timeout === 1000) window.performanceTest.timers.add(id);
			return id;
		};
		window.clearInterval = (id) => {
			if (id !== undefined) window.performanceTest.timers.delete(id);
			clear(id);
		};
		const setItem = Storage.prototype.setItem;
		Storage.prototype.setItem = function (key, value) {
			if (key === 'lumen_catalog_v1') window.performanceTest.catalogWrites++;
			setItem.call(this, key, value);
		};
	});
});

test('connection clock only ticks while connected on the visible home screen', async ({
	page,
}) => {
	await page.goto('/');
	await expect(page.getByRole('status')).toHaveText(ru.home.disconnected);
	expect(await page.evaluate(() => window.performanceTest.timers.size)).toBe(0);
	await page
		.getByRole('button', { name: ru.home.clickToConnect, exact: true })
		.click();
	await expect(page.getByRole('status')).toHaveText(ru.home.connected);
	await expect
		.poll(() => page.evaluate(() => window.performanceTest.timers.size))
		.toBe(1);
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	await expect
		.poll(() => page.evaluate(() => window.performanceTest.timers.size))
		.toBe(0);
	await page.getByRole('button', { name: ru.nav.home, exact: true }).click();
	await expect
		.poll(() => page.evaluate(() => window.performanceTest.timers.size))
		.toBe(1);
	await page
		.getByRole('button', { name: ru.home.clickToDisconnect, exact: true })
		.click();
	await expect(page.getByRole('status')).toHaveText(ru.home.disconnected);
	await expect
		.poll(() => page.evaluate(() => window.performanceTest.timers.size))
		.toBe(0);
});

test('multiple selected processes are saved with one catalog write', async ({
	page,
}) => {
	await page.goto('/');
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await page
		.getByRole('region', { name: ru.rules.title })
		.getByRole('button', { name: ru.common.add, exact: true })
		.click();
	const editor = page.locator('.rule-editor-view');
	await editor
		.getByRole('tab', { name: ru.rules.applicationTab, exact: true })
		.click();
	await editor
		.getByRole('button', { name: ru.rules.selectProcess, exact: true })
		.click();
	const picker = page.locator('.process-picker-view');
	await picker.locator('.process-row').filter({ hasText: 'Discord' }).click();
	await picker
		.locator('.process-row')
		.filter({ hasText: 'Telegram.exe' })
		.click();
	await picker
		.getByRole('button', { name: ru.common.save, exact: true })
		.click();
	const writesBefore = await page.evaluate(
		() => window.performanceTest.catalogWrites,
	);
	await editor
		.getByRole('button', { name: ru.common.save, exact: true })
		.click();
	await expect(page.locator('.rule-row')).toHaveCount(2);
	expect(await page.evaluate(() => window.performanceTest.catalogWrites)).toBe(
		writesBefore + 1,
	);
	await page.reload();
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await expect(page.locator('.rule-row')).toHaveCount(2);
});

test('large catalogs remain searchable without losing rule associations', async ({
	page,
}, testInfo) => {
	const catalog = initialCatalog();
	catalog.servers = Array.from({ length: 2000 }, (_, index) => ({
		...catalog.servers[index % catalog.servers.length],
		id: `server-${index}`,
	}));
	catalog.selectedServerId = 'server-0';
	catalog.rules = Array.from({ length: 1000 }, (_, index) => ({
		id: `rule-${index}`,
		name: `site-${index}.com`,
		isDomain: true,
		serverId: `server-${index}`,
		enabled: true,
	}));
	await page.addInitScript((value) => {
		localStorage.setItem('lumen_catalog_v1', JSON.stringify(value));
		// Isolate list work from the independent WebGL animation workload.
		localStorage.setItem(
			'lumen_shader_settings',
			JSON.stringify({ enabled: false }),
		);
	}, catalog);
	await page.goto('/');
	const start = performance.now();
	await page.getByRole('button', { name: ru.nav.servers, exact: true }).click();
	await expect(page.locator('.server-row')).toHaveCount(2000);
	const serversReadyMs = performance.now() - start;
	await page
		.getByRole('button', { name: ru.common.search, exact: true })
		.click();
	const searchStart = performance.now();
	await page
		.getByPlaceholder(ru.servers.searchPlaceholder)
		.fill(ru.servers.countries.jp);
	await expect(page.locator('.server-row')).toHaveCount(285);
	const searchReadyMs = performance.now() - searchStart;
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await expect(page.locator('.rule-row')).toHaveCount(1000);
	await expect(
		page.locator('.rule-row').first().locator('.server-pill'),
	).toContainText(ru.servers.countries.fi);
	const results = {
		servers: 2000,
		rules: 1000,
		serversReadyMs,
		searchReadyMs,
		includesAutomationOverhead: true,
		shaderEnabled: false,
	};
	await testInfo.attach('large-catalog-timing', {
		body: JSON.stringify(results, null, 2),
		contentType: 'application/json',
	});
});
