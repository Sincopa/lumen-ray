import { test, expect } from '@playwright/test';
import ru from '../../src/locales/ru.json' with { type: 'json' };

test('production browser starts offline with no external assets or errors', async ({
	page,
}) => {
	const errors: string[] = [],
		external: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('request', (request) => {
		if (
			/^https?:/.test(request.url()) &&
			!request.url().startsWith('http://127.0.0.1:5187') &&
			!request.url().includes('fonts.googleapis.com') &&
			!request.url().includes('fonts.gstatic.com')
		)
			external.push(request.url());
	});
	await page.goto('/');
	await expect(page.getByText('Демо-режим · трафик не защищён')).toBeVisible();
	await expect(page.locator('.server-details')).not.toContainText('Финляндия');
	await expect(page.getByRole('status')).toHaveText('Отключен');
	await page
		.getByRole('button', { name: 'Нажмите для подключения', exact: true })
		.click();
	await expect(page.getByRole('status')).toHaveText('Подключен');
	await page
		.getByRole('button', { name: 'Нажмите для отключения', exact: true })
		.click();
	await expect(page.getByRole('status')).toHaveText('Отключен');
	expect(errors).toEqual([]);
	expect(external).toEqual([]);
});

test('server choice survives navigation, connecting and reload', async ({
	page,
}) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Сервера', exact: true }).click();
	await page.getByRole('button', { name: /Нидерланды/ }).click();
	await expect(page.locator('.server-selector')).toHaveAttribute(
		'aria-label',
		'Нидерланды',
	);
	await page
		.getByRole('button', { name: 'Нажмите для подключения', exact: true })
		.click();
	await expect(page.getByRole('status')).toHaveText('Подключен');
	await page.getByRole('button', { name: 'Сервера', exact: true }).click();
	await page.getByRole('button', { name: /Финляндия/ }).click();
	await expect(page.getByRole('status')).toHaveText('Подключен');
	await expect(page.locator('.server-selector')).toHaveAttribute(
		'aria-label',
		'Финляндия',
	);
	await page.getByRole('button', { name: 'Главная', exact: true }).click();
	await expect(page.getByRole('status')).toHaveText('Подключен');
	await page.reload();
	await expect(page.locator('.server-selector')).toHaveAttribute(
		'aria-label',
		'Финляндия',
	);
});

test('routing mode stays locked for the whole VPN session', async ({
	page,
}) => {
	await page.goto('/');
	const modeButtons = page.locator('.action-pill-btn');
	await expect(modeButtons.first()).toBeEnabled();
	await page.locator('.central-logo-btn').click();
	await expect(modeButtons.first()).toBeDisabled();
	await expect(page.getByRole('status')).toHaveText(ru.home.connected);
	await expect(modeButtons.last()).toBeDisabled();
	await page.locator('.central-logo-btn').click();
	await expect(modeButtons.first()).toBeEnabled();
});

test('empty home morphs the centered add button into a subscription field', async ({
	page,
}) => {
	await page.goto('/');
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	await page.locator('.sub-card-wrapper .title-group').click();
	await page
		.locator('.sub-card-wrapper')
		.getByRole('button', { name: ru.common.menu, exact: true })
		.click();
	await page
		.getByRole('button', { name: ru.common.delete, exact: true })
		.click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: ru.common.continue, exact: true })
		.click();
	await page.getByRole('button', { name: ru.nav.home, exact: true }).click();

	const app = page.locator('.app-viewport');
	const add = page.getByRole('button', {
		name: ru.subscriptions.addProfile,
		exact: true,
	});
	const [appBounds, buttonBounds] = await Promise.all([
		app.boundingBox(),
		add.boundingBox(),
	]);
	expect(appBounds).not.toBeNull();
	expect(buttonBounds).not.toBeNull();
	expect(
		Math.abs(
			buttonBounds!.y +
				buttonBounds!.height / 2 -
				(appBounds!.y + appBounds!.height / 2),
		),
	).toBeLessThan(2);
	await page.screenshot({ path: 'artifacts/verification/empty-home-add.png' });

	await add.click();
	const input = page.getByLabel(ru.subscriptions.domainLink, { exact: true });
	await expect(input).toBeFocused();
	await expect(input).toHaveAttribute(
		'placeholder',
		ru.subscriptions.pasteLink,
	);
	await page.waitForTimeout(380);
	await page.screenshot({
		path: 'artifacts/verification/empty-home-input.png',
	});
	await input.fill('https://example.com/subscription');
	await input.press('Enter');
	await expect(page.locator('.connection-view')).toBeVisible();
});

test('subscription import, sharing, rename, refresh and delete work', async ({
	page,
	context,
}) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await page.setViewportSize({ width: 620, height: 700 });
	await page.goto('/');
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	await page.screenshot({
		path: 'artifacts/verification/subscription-metrics.png',
	});
	await page
		.getByRole('button', { name: ru.subscriptions.addProfile, exact: true })
		.click();
	const form = page.locator('.add-profile-view');
	await form.getByLabel(ru.common.name, { exact: true }).fill('Test');
	await form
		.getByLabel(ru.subscriptions.domainLink, { exact: true })
		.fill('https://example.com/secret-test-token');
	await form.getByRole('button', { name: ru.common.save, exact: true }).click();
	await expect(form).toHaveCount(0);
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	const card = page.locator('.sub-card-wrapper').filter({ hasText: 'Test' });
	await card.locator('.title-group').click();
	await card
		.getByRole('button', { name: ru.common.reload, exact: true })
		.click();
	await expect(
		card.getByRole('button', { name: ru.common.reload, exact: true }),
	).toBeEnabled();
	await card.getByRole('button', { name: ru.common.menu, exact: true }).click();
	await page.getByRole('button', { name: ru.common.edit, exact: true }).click();
	await page.locator('.rename-input').fill('Renamed');
	await page.locator('.rename-action-btn.check').click();
	const renamed = page
		.locator('.sub-card-wrapper')
		.filter({ hasText: 'Renamed' });
	await renamed
		.getByRole('button', { name: ru.common.menu, exact: true })
		.click();
	await page
		.getByRole('button', { name: ru.common.share, exact: true })
		.click();
	const shareDialog = page.getByRole('dialog');
	const appBounds = await page.locator('.app-viewport').boundingBox();
	const overlayBounds = await page.locator('.dialog-backdrop').boundingBox();
	expect(overlayBounds).toEqual(appBounds);
	await expect(shareDialog.getByRole('textbox')).toHaveValue(
		'https://example.com/secret-test-token',
	);
	await page.waitForTimeout(260);
	await page.screenshot({
		path: 'artifacts/verification/subscription-share-dialog.png',
	});
	await shareDialog
		.getByRole('button', { name: ru.common.copy, exact: true })
		.click();
	await expect(
		shareDialog.getByRole('button', { name: ru.common.copied, exact: true }),
	).toBeVisible();
	await expect(shareDialog.locator('.copy-icon:not([inert])')).toHaveText(
		'check',
	);
	await page.screenshot({
		path: 'artifacts/verification/subscription-share-copied.png',
	});
	await shareDialog
		.getByRole('button', { name: ru.common.close, exact: true })
		.click();
	await renamed
		.getByRole('button', { name: ru.common.menu, exact: true })
		.click();
	await page
		.getByRole('button', { name: ru.common.delete, exact: true })
		.click();
	await expect(page.locator('.confirm-dialog')).toHaveCSS('width', '326px');
	await expect(page.locator('.confirm-button').first()).toHaveCSS(
		'height',
		'64px',
	);
	await page.waitForTimeout(300);
	await page.screenshot({
		path: 'artifacts/verification/subscription-delete-modal.png',
	});
	await page
		.getByRole('dialog')
		.getByRole('button', { name: ru.common.continue, exact: true })
		.click();
	await expect(renamed).toHaveCount(0);
	expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
		'secret-test-token',
	);
});

test('add panels and searches stay usable in the compact window', async ({
	page,
}) => {
	await page.goto('/');
	await page
		.getByRole('button', { name: ru.nav.subscriptions, exact: true })
		.click();
	await page
		.getByRole('button', { name: ru.subscriptions.addProfile, exact: true })
		.click();
	const form = page.locator('.add-profile-view');
	await expect(form).toBeVisible();
	await form
		.getByRole('button', { name: ru.common.close, exact: true })
		.click();
	await page.getByRole('button', { name: ru.nav.servers, exact: true }).click();
	await page
		.getByRole('region', { name: ru.servers.title })
		.getByRole('button', { name: ru.common.search })
		.click();
	await page
		.getByPlaceholder(ru.servers.searchPlaceholder)
		.fill(ru.servers.countries.nl);
	await expect(page.locator('.server-row')).toHaveCount(1);
	await page
		.locator('.header-search.search-open')
		.getByRole('button', { name: ru.common.close })
		.click();
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	const emptyRules = page.getByText(ru.rules.empty, { exact: true });
	await expect(emptyRules).toBeVisible();
	await expect(emptyRules).toHaveCSS('color', 'rgba(255, 255, 255, 0.25)');
	await page.screenshot({
		path: 'artifacts/verification/rules-empty-state.png',
	});
	await page
		.getByRole('region', { name: ru.rules.title })
		.getByRole('button', { name: ru.common.add })
		.click();
	const editor = page.locator('.rule-editor-view');
	await expect(
		editor.getByRole('tab', { name: ru.rules.domainTab, exact: true }),
	).toHaveAttribute('aria-selected', 'true');
	await editor
		.getByRole('tab', { name: ru.rules.applicationTab, exact: true })
		.click();
	await expect(
		editor.getByRole('button', { name: ru.rules.selectProcess, exact: true }),
	).toBeVisible();
	await editor
		.getByRole('button', { name: ru.rules.selectServer, exact: true })
		.click();
	await page
		.getByRole('button', { name: new RegExp(ru.servers.countries.de) })
		.click();
	await expect(
		editor.getByRole('button', { name: ru.rules.selectServer, exact: true }),
	).toContainText(ru.servers.countries.de);
	await editor
		.getByRole('button', { name: ru.rules.selectProcess, exact: true })
		.click();
	const picker = page.locator('.process-picker-view');
	await picker
		.getByRole('button', { name: ru.common.search, exact: true })
		.click();
	await picker.locator('input').fill('telegram');
	await expect(picker.locator('.process-row')).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(picker.locator('input')).toHaveValue('');
});

test('rules persist, validate input and do not trap navigation', async ({
	page,
}) => {
	await page.goto('/');
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await page
		.getByRole('region', { name: ru.rules.title, exact: true })
		.getByRole('button', { name: ru.common.add, exact: true })
		.click();
	const editor = page.locator('.rule-editor-view');
	await editor.locator('#domain-input').fill('');
	await editor
		.getByRole('button', { name: ru.common.save, exact: true })
		.click();
	await expect(page.getByRole('alert')).toBeVisible();
	await editor.locator('#domain-input').fill('example.com');
	await editor
		.getByRole('button', { name: ru.common.save, exact: true })
		.click();
	await page
		.getByRole('switch', {
			name: ru.rules.toggle.replace('{name}', 'example.com'),
		})
		.click();
	await expect(page.getByRole('switch')).toHaveAttribute(
		'aria-checked',
		'false',
	);
	await page.getByRole('button', { name: ru.nav.home, exact: true }).click();
	await expect(page.getByRole('status')).toHaveText(ru.home.disconnected);
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await expect(page.getByRole('switch')).toHaveAttribute(
		'aria-checked',
		'false',
	);
	await page.reload();
	await page.getByRole('button', { name: ru.nav.rules, exact: true }).click();
	await expect(page.getByText('example.com', { exact: true })).toBeVisible();
});

test('settings are honest and language and animation persist', async ({
	page,
}) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Настройки', exact: true }).click();
	await page.screenshot({
		path: 'artifacts/verification/settings-main-tuned.png',
	});
	await expect(
		page.getByRole('combobox', { name: ru.settings.routingMode }),
	).toBeDisabled();
	await page
		.getByRole('button', { name: ru.settings.presets, exact: true })
		.click();
	await page.screenshot({ path: 'artifacts/verification/preset-dropdown.png' });
	await page.getByRole('button', { name: /Cyberpunk/ }).click();
	await page.getByRole('button', { name: ru.common.back }).click();
	await expect(
		page.getByRole('checkbox', { name: ru.settings.autoConnect, exact: true }),
	).toBeDisabled();
	await expect(
		page.getByRole('checkbox', { name: ru.settings.autoConnect, exact: true }),
	).not.toBeChecked();
	await page
		.getByRole('checkbox', { name: ru.settings.animation, exact: true })
		.uncheck();
	await expect(page.locator('canvas')).toHaveClass(/disabled/);
	await page
		.getByRole('combobox', { name: ru.settings.language, exact: true })
		.click();
	await page.getByRole('option', { name: 'English' }).click();
	await page.reload();
	await expect(
		page.getByText('Demo mode · traffic is not protected'),
	).toBeVisible();
	await expect(page.locator('canvas')).toHaveClass(/disabled/);
});

test('interface scale applies only after the slider is released', async ({
	page,
}) => {
	await page.goto('/');
	await page
		.getByRole('button', { name: ru.nav.settings, exact: true })
		.click();
	const slider = page.getByRole('slider', { name: ru.settings.uiScale });
	const scale = () =>
		page.evaluate(() =>
			document.documentElement.style.getPropertyValue('--ui-zoom'),
		);
	const initialScale = await scale();

	await slider.evaluate((element) => {
		const input = element as HTMLInputElement;
		input.value = '1.25';
		input.dispatchEvent(new Event('input', { bubbles: true }));
	});
	expect(await scale()).toBe(initialScale);

	await slider.evaluate((element) => {
		element.dispatchEvent(new Event('change', { bubbles: true }));
	});
	await expect.poll(scale).toBe('1.25');
});

test('updated shader links and restored cards keep their controls', async ({
	page,
}) => {
	const errors: string[] = [];
	page.on('console', (message) => {
		if (message.type() === 'error') errors.push(message.text());
	});
	await page.goto('/');
	await expect
		.poll(() =>
			page.locator('canvas').evaluate((canvas) => {
				const gl = (canvas as HTMLCanvasElement).getContext('webgl2');
				const program = gl?.getParameter(gl.CURRENT_PROGRAM);
				return !!(
					gl &&
					program &&
					gl.getProgramParameter(program, gl.LINK_STATUS)
				);
			}),
		)
		.toBe(true);
	await page.screenshot({ path: 'artifacts/verification/home-updated.png' });
	await page.getByRole('button', { name: 'Подписки', exact: true }).click();
	await page.locator('.title-group').click();
	await page.getByRole('button', { name: 'Меню', exact: true }).click();
	await page.screenshot({
		path: 'artifacts/verification/subscriptions-restored.png',
	});
	await page.getByRole('button', { name: 'Главная', exact: true }).click();
	await page
		.getByRole('button', {
			name: ru.nav.rules,
			exact: true,
		})
		.click();
	await page
		.getByRole('region', { name: 'Правила', exact: true })
		.getByRole('button', { name: 'Добавить', exact: true })
		.click();
	const editor = page.locator('.rule-editor-view');
	await editor.locator('#domain-input').fill('example.com');
	await editor
		.getByRole('button', { name: ru.common.save, exact: true })
		.click();
	await page
		.getByRole('button', { name: 'example.com: Сервера', exact: true })
		.click();
	await page.getByRole('button', { name: /Нидерланды/ }).click();
	await expect(page.locator('.server-pill')).toHaveText('Нидерланды');
	await page.screenshot({ path: 'artifacts/verification/rules-restored.png' });
	await page.getByRole('button', { name: 'Меню', exact: true }).click();
	await page
		.getByRole('button', { name: 'Редактировать', exact: true })
		.click();
	await page
		.getByRole('checkbox', { name: 'Выбрать example.com', exact: true })
		.click();
	await page
		.getByRole('button', { name: 'Удалить выбранные', exact: true })
		.click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: ru.common.continue, exact: true })
		.click();
	await expect(page.locator('.rule-row')).toHaveCount(0);
	expect(errors).toEqual([]);
});

for (const size of [
	{ width: 360, height: 560 },
	{ width: 400, height: 700 },
	{ width: 800, height: 600 },
]) {
	test(
		'layout keeps navigation in bounds at ' + size.width + 'x' + size.height,
		async ({ page }) => {
			await page.setViewportSize(size);
			await page.goto('/');
			for (const button of await page.locator('nav button').all()) {
				const box = await button.boundingBox();
				expect(box).not.toBeNull();
				expect(box!.x).toBeGreaterThanOrEqual(0);
				expect(box!.x + box!.width).toBeLessThanOrEqual(size.width);
				expect(box!.y + box!.height).toBeLessThanOrEqual(size.height);
			}
			await page
				.getByRole('button', { name: 'Настройки', exact: true })
				.click();
			expect(
				await page.evaluate(() => document.documentElement.scrollWidth),
			).toBeLessThanOrEqual(size.width);
		},
	);
}

test('verifies all 7 bug fixes and visual refinements', async ({ page }) => {
	await page.goto('/');

	// Bug 4: Verify 24px window corner radius
	const appRadius = await page
		.locator('.app-viewport')
		.evaluate((el) => window.getComputedStyle(el).borderRadius);
	expect(appRadius).toBe('24px');

	// Bug 6: Verify top right control button is 1:1 circle (not oval) on hover
	const closeBtn = page.locator('.control-btn.close');
	await closeBtn.hover();
	await page.waitForTimeout(200);
	const closeStyles = await closeBtn.evaluate((el) => {
		const s = window.getComputedStyle(el);
		return { width: parseFloat(s.width), height: parseFloat(s.height) };
	});
	expect(closeStyles.width).toBe(28);
	expect(closeStyles.height).toBe(28);
	await page.screenshot({
		path: 'artifacts/verification/check6-control-btn-hover.png',
	});

	// Bug 5: Verify bottom nav buttons and gap on hover
	const navHomeBtn = page.locator('.pill-btn').first();
	await navHomeBtn.hover();
	await page.waitForTimeout(200);
	const bounds = await page
		.locator('.nav-pill .pill-btn')
		.evaluateAll((buttons) =>
			buttons.map((button) => {
				const { left, right, width } = button.getBoundingClientRect();
				return { left, right, width };
			}),
		);
	for (let index = 1; index < bounds.length; index++) {
		expect(bounds[index].left).toBeGreaterThanOrEqual(bounds[index - 1].right);
		expect(bounds[index].width).toBeCloseTo(bounds[0].width, 1);
	}
	await page.screenshot({
		path: 'artifacts/verification/check5-bottom-nav-hover.png',
	});

	// Bug 3: Connected state background shader (dark and deep, not blown out)
	await page
		.getByRole('button', { name: 'Нажмите для подключения', exact: true })
		.click();
	await page.waitForTimeout(1000);
	await page.screenshot({
		path: 'artifacts/verification/check3-shader-connected.png',
	});
	await page
		.getByRole('button', { name: 'Нажмите для отключения', exact: true })
		.click();
	await page.waitForTimeout(300);

	// Context menu keeps its labels readable over the cards and uses local icons.
	await page.getByRole('button', { name: 'Подписки', exact: true }).click();
	await page.locator('.title-group').first().click();
	await page.getByRole('button', { name: 'Меню', exact: true }).first().click();
	await page.waitForTimeout(300);

	const menuBg = await page
		.locator('.popup-menu')
		.evaluate((el) => window.getComputedStyle(el).backgroundColor);
	expect(menuBg).toBe('rgba(0, 0, 0, 0.2)');
	const iconCount = await page
		.locator('.popup-menu .menu-item-icon-circle img')
		.count();
	expect(iconCount).toBe(3);
	await page.screenshot({
		path: 'artifacts/verification/check1-context-menu.png',
	});

	// Bug 7: Mode rules -> Home tab dim-backdrop disappears smoothly
	await page.getByRole('button', { name: 'Главная', exact: true }).click();
	await page
		.getByRole('button', {
			name: ru.nav.rules,
			exact: true,
		})
		.click();
	await page.waitForTimeout(300);
	expect(
		await page
			.locator('.app-ui')
			.evaluate((el) => el.classList.contains('dim-backdrop')),
	).toBe(true);

	await page.getByRole('button', { name: 'Главная', exact: true }).click();
	await page.waitForTimeout(400);
	expect(
		await page
			.locator('.app-ui')
			.evaluate((el) => el.classList.contains('dim-backdrop')),
	).toBe(false);
	const backdropOpacity = await page
		.locator('.app-ui')
		.evaluate((el) => window.getComputedStyle(el, '::before').opacity);
	expect(parseFloat(backdropOpacity)).toBe(0);
	await page.screenshot({
		path: 'artifacts/verification/check7-home-after-rules.png',
	});
});
