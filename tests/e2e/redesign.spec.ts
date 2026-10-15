import { test, expect, type Locator } from '@playwright/test';
import { initialCatalog } from '../../src/lib/services/catalog';
import ru from '../../src/locales/ru.json' with { type: 'json' };

async function bounds(
	locator: Locator,
	target: [number, number, number, number],
) {
	await expect
		.poll(async () => {
			const box = await locator.boundingBox();
			if (!box) return Infinity;
			return Math.max(
				...(['x', 'y', 'width', 'height'] as const).map((key, index) =>
					Math.abs(box[key] - target[index]),
				),
			);
		})
		.toBeLessThan(0.6);
}

test('Figma home geometry and routing toggle retain their behavior', async ({
	page,
}) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	await expect(page.locator('.lottie-player.ready svg')).toBeVisible();
	await bounds(page.locator('.window-header'), [16, 16, 358, 40]);
	await bounds(page.locator('.central-logo-btn'), [67, 292.85, 256, 175]);
	await bounds(page.locator('.server-selector'), [32, 666, 326, 64]);
	await bounds(page.locator('.nav-pill'), [32, 762, 188, 50]);
	await bounds(page.locator('.action-pill'), [271, 762, 87, 50]);
	await expect(page.locator('.nav-pill button')).toHaveCount(4);
	await expect(page.locator('.action-pill')).toHaveText('All');
	await page.locator('.action-pill').click();
	await expect(page.locator('.action-pill')).toHaveText('Tun');
	await expect(page.locator('.action-pill')).toHaveAttribute(
		'aria-label',
		ru.home.modeRules,
	);
	await page.reload();
	await expect(page.locator('.action-pill')).toHaveText('Tun');
	await page.locator('.action-pill').click();
	await expect(page.locator('.action-pill')).toHaveAttribute(
		'aria-label',
		ru.home.modeGlobal,
	);
});

test('subscriptions expand independently with the design card dimensions', async ({
	page,
}) => {
	const catalog = initialCatalog();
	catalog.subscriptions = ['one', 'two', 'three'].map((id) => ({
		id,
		name: 'GRBNK VPN',
		updatedAt: Date.now() - 60_000,
		daysRemaining: 40,
		traffic: '∞',
	}));
	catalog.servers = catalog.servers.map((server) => ({
		...server,
		subscriptionId: 'two',
	}));
	catalog.selectedSubscriptionId = 'two';
	await page.addInitScript(
		(value) => localStorage.setItem('lumen_catalog_v1', JSON.stringify(value)),
		catalog,
	);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	const cards = page.locator('.sub-card');
	await bounds(cards.nth(0), [16, 72, 358, 68]);
	await cards.nth(1).locator('.title-group').click();
	await cards.nth(2).locator('.title-group').click();
	await expect(page.locator('.sub-card.expanded')).toHaveCount(2);
	await bounds(cards.nth(1), [16, 146, 358, 117]);
	await bounds(cards.nth(2), [16, 269, 358, 117]);
	await page.mouse.move(0, 0);
	await page.screenshot({
		path: 'artifacts/verification/design-subscriptions.png',
	});
});

test('rules preserve toggles, search, server choice and bulk selection', async ({
	page,
}) => {
	const catalog = initialCatalog();
	catalog.routingMode = 'rules';
	catalog.rules = [
		{
			id: 'app',
			name: 'Yandex',
			matcher: 'application',
			isDomain: false,
			serverId: 'demo-fi',
			enabled: true,
		},
		{
			id: 'domain',
			name: 'youtube.com',
			matcher: 'domain',
			isDomain: true,
			serverId: 'demo-fi',
			enabled: true,
		},
	];
	await page.addInitScript(
		(value) => localStorage.setItem('lumen_catalog_v1', JSON.stringify(value)),
		catalog,
	);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await bounds(page.locator('.header-pill'), [16, 72, 300, 66]);
	await bounds(page.locator('.add-rule-btn'), [324, 80, 50, 50]);
	const app = page.locator('.rule-row.application-rule');
	const domain = page.locator('.rule-row').filter({ hasText: 'youtube.com' });
	await expect(app.getByRole('switch')).toHaveCSS('width', '60px');
	await expect(domain.getByRole('switch')).toHaveCSS('width', '28px');
	await app.getByRole('switch').click();
	await expect(app.getByRole('switch')).toHaveAttribute(
		'aria-checked',
		'false',
	);
	await domain.locator('.server-pill').click();
	await bounds(page.locator('.picker-bar button').first(), [32, 762, 50, 50]);
	await bounds(page.locator('.picker-bar button').last(), [308, 762, 50, 50]);
	await page
		.locator('.picker-bar')
		.getByRole('button', { name: ru.common.search })
		.click();
	await page
		.getByPlaceholder(ru.servers.searchPlaceholder)
		.fill(ru.servers.countries.nl);
	await page
		.locator('.server-row')
		.filter({ hasText: ru.servers.countries.nl })
		.click();
	await expect(domain.locator('.server-pill')).toContainText(
		ru.servers.countries.nl,
	);
	await domain.locator('.server-pill').click();
	await expect(
		page.locator('.server-picker-view .header-search.search-open'),
	).toHaveCount(0);
	await page
		.locator('.picker-bar')
		.getByRole('button', { name: ru.common.save })
		.click();
	await page
		.locator('.header-card-actions')
		.getByRole('button', { name: ru.common.search })
		.click();
	await page.getByPlaceholder(ru.rules.searchPlaceholder).fill('yandex');
	await expect(page.locator('.rule-row')).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(page.locator('.rule-row')).toHaveCount(2);
	await page
		.getByRole('button', { name: ru.rules.selectMode, exact: true })
		.click();
	await expect(page.locator('.bottom-nav')).toHaveCount(0);
	await bounds(page.locator('.header-pill'), [16, 72, 358, 66]);
	await domain.getByRole('checkbox').click();
	await expect(domain.getByRole('checkbox')).toHaveAttribute(
		'aria-checked',
		'true',
	);
	await page
		.locator('.select-bar')
		.getByRole('button', { name: ru.common.close })
		.click();
	await expect(page.locator('.bottom-nav')).toBeVisible();
	await page.mouse.move(0, 0);
	await page.screenshot({ path: 'artifacts/verification/design-rules.png' });
});

test('Lottie reacts to hover and connection, and honors reduced motion', async ({
	page,
}) => {
	await page.goto('/');
	const face = page.locator('.lumen-face');
	await expect(face.locator('.lottie-player.ready svg')).toBeVisible();
	// Everything the player draws: where each layer sits and what each contour is.
	const frame = () =>
		face
			.locator('svg')
			.evaluate((svg) =>
				[...svg.querySelectorAll('g[transform], path')]
					.map((element) =>
						element instanceof SVGPathElement
							? getComputedStyle(element).getPropertyValue('d')
							: getComputedStyle(element).transform,
					)
					.join('|'),
			);
	await page.locator('.central-logo-btn').hover();
	await expect(face).toHaveAttribute('data-expression', 'wink');
	await expect(face).toHaveAttribute('data-segment', /^wink(-l)?$/);
	const winkStart = await frame();
	await expect.poll(frame).not.toBe(winkStart);
	await page.evaluate(() => {
		Object.defineProperty(document, 'hidden', {
			configurable: true,
			get: () => true,
		});
		document.dispatchEvent(new Event('visibilitychange'));
	});
	await page.waitForTimeout(50);
	const paused = await frame();
	await page.waitForTimeout(200);
	expect(await frame()).toBe(paused);
	await page.evaluate(() => {
		Object.defineProperty(document, 'hidden', {
			configurable: true,
			get: () => false,
		});
		document.dispatchEvent(new Event('visibilitychange'));
	});
	await expect.poll(frame).not.toBe(paused);
	await page.locator('.central-logo-btn').click();
	await expect(face).toHaveAttribute('data-expression', 'connecting');
	await page.mouse.move(0, 0);
	await expect(face).toHaveAttribute('data-expression', 'connected');
	await page.locator('.central-logo-btn').click();
	await page.mouse.move(0, 0);
	await expect(face).toHaveAttribute('data-expression', 'idle');
	await page.emulateMedia({ reducedMotion: 'reduce' });
	const initial = await frame();
	await page.waitForTimeout(300);
	expect(await frame()).toBe(initial);
});

test('the resting face greets, then fidgets with gestures that return to rest', async ({
	page,
}) => {
	await page.goto('/');
	const face = page.locator('.lumen-face');
	await expect(face).toHaveAttribute('data-segment', 'idle:hello');
	await expect(face).toHaveAttribute('data-segment', 'idle', { timeout: 5000 });
	// After a few breaths the face plays one gesture (blink, glance, nod...)...
	await expect
		.poll(() => face.getAttribute('data-segment'), { timeout: 20_000 })
		.toMatch(/^idle:/);
	// ...and then goes back to breathing.
	await expect(face).toHaveAttribute('data-segment', 'idle', { timeout: 5000 });
});

test('connecting scans, connecting celebrates, and disconnecting plays out', async ({
	page,
}) => {
	await page.goto('/');
	const face = page.locator('.lumen-face');
	await expect(face.locator('.lottie-player.ready svg')).toBeVisible();
	await page.locator('.central-logo-btn').click();
	await expect(face).toHaveAttribute('data-segment', 'connecting');
	await page.mouse.move(0, 0);
	await expect(face).toHaveAttribute('data-segment', 'connected');
	await expect(face).toHaveAttribute('data-segment', 'connected-loop', {
		timeout: 5000,
	});
	await page.locator('.central-logo-btn').click();
	await page.mouse.move(0, 0);
	// The status flips to "disconnected" at once; the little performance still plays.
	await expect(face).toHaveAttribute('data-segment', 'disconnecting');
	await expect(face).toHaveAttribute('data-expression', 'idle');
	await expect(face).toHaveAttribute('data-segment', 'idle', { timeout: 5000 });
});

test('reduced motion keeps the mood as a still pose', async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	const face = page.locator('.lumen-face');
	await expect(face).toHaveAttribute('data-segment', 'pose:idle');
	await page.locator('.central-logo-btn').click();
	await page.mouse.move(0, 0);
	await expect(face).toHaveAttribute('data-expression', 'connected');
	await expect(face).toHaveAttribute('data-segment', 'pose:connected');
	const still = () => face.locator('svg').evaluate((svg) => svg.innerHTML);
	const before = await still();
	await page.waitForTimeout(400);
	expect(await still()).toBe(before);
});

for (const size of [
	{ width: 293, height: 634 },
	{ width: 320, height: 480 },
	{ width: 360, height: 560 },
	{ width: 800, height: 600 },
]) {
	test(`redesigned screens remain usable at ${size.width}x${size.height}`, async ({
		page,
	}) => {
		test.fail(size.width === 293, 'Known navigation overflow at 293 px');
		await page.setViewportSize(size);
		await page.goto('/');
		for (const tab of [ru.nav.home, ru.nav.subscriptions, ru.nav.rules]) {
			await page.getByRole('button', { name: tab, exact: true }).click();
			for (const button of await page
				.locator('.bottom-nav button, .controls-pill button')
				.all()) {
				const box = await button.boundingBox();
				expect(box).not.toBeNull();
				expect(box!.x).toBeGreaterThanOrEqual(0);
				expect(box!.x + box!.width).toBeLessThanOrEqual(size.width);
				expect(box!.y + box!.height).toBeLessThanOrEqual(size.height);
			}
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth),
			).toBeLessThanOrEqual(size.width);
		}
		await page.getByRole('button', { name: ru.nav.home, exact: true }).click();
		await page.locator('.server-selector').click();
		await page
			.getByRole('button', { name: ru.common.search, exact: true })
			.click();
		await page
			.getByPlaceholder(ru.servers.searchPlaceholder)
			.fill(ru.servers.countries.nl);
		await expect(page.locator('.server-row')).toHaveCount(1);
		await page.locator('.server-row').click();
		await expect(page.locator('.server-selector')).toHaveAttribute(
			'aria-label',
			ru.servers.countries.nl,
		);
	});
}
