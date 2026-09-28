import { test, expect } from '@playwright/test';
import ru from '../../src/locales/ru.json' with { type: 'json' };

test('tools extend settings without changing the main navigation', async ({
	page,
}) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto('/');
	await page
		.locator('.window-header')
		.getByRole('button', { name: ru.settings.title })
		.click();
	await page.getByRole('button', { name: 'Инструменты', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	await expect(
		dialog.getByRole('button', { name: 'Применить DNS' }),
	).toBeDisabled();
	for (const section of [
		'Резервная копия',
		'Импорт / QR',
		'Диагностика',
		'История обновлений',
	]) {
		await dialog.getByRole('combobox', { name: 'Раздел' }).click();
		await page.getByRole('option', { name: section, exact: true }).click();
		await expect(dialog).toBeVisible();
		expect(
			await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
		).toBe(true);
	}
	await page.screenshot({ path: 'test-results/tools-settings.png' });
	await dialog.getByRole('button', { name: ru.common.close }).click();
	await expect(dialog).toHaveCount(0);
	expect(errors).toEqual([]);
});

test('advanced settings keep the existing nested navigation in browser demo', async ({
	page,
}) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto('/');
	await page
		.locator('.window-header')
		.getByRole('button', { name: ru.settings.title })
		.click();

	await page.getByRole('button', { name: ru.settings.notifications }).click();
	for (const name of [
		ru.settings.notifyDisconnect,
		ru.settings.notifyReconnect,
		ru.settings.notifyExpiry,
		ru.settings.notifyUpdate,
	]) {
		await expect(page.getByRole('checkbox', { name })).toBeDisabled();
	}
	await page.getByRole('button', { name: ru.common.back }).click();

	await page.getByRole('button', { name: ru.settings.diagnostics }).click();
	await expect(
		page.getByRole('button', { name: ru.settings.diagRefresh }),
	).toBeDisabled();
	await page.getByRole('button', { name: ru.common.back }).click();
	await expect(
		page.getByRole('checkbox', { name: ru.settings.autoReconnect }),
	).toBeDisabled();
	expect(errors).toEqual([]);
});
