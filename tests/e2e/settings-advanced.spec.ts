import { test, expect } from '@playwright/test';
import ru from '../../src/locales/ru.json' with { type: 'json' };

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
