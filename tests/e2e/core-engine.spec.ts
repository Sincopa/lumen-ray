import { test, expect } from '@playwright/test';
import ru from '../../src/locales/ru.json' with { type: 'json' };

test('engine setting fits the existing settings page and defaults to Xray', async ({
	page,
}) => {
	await page.goto('/');
	await page
		.locator('.window-header')
		.getByRole('button', { name: ru.settings.title })
		.click();
	const engine = page.getByRole('combobox', { name: ru.settings.coreEngine });
	await engine.evaluate((element) =>
		element.scrollIntoView({ block: 'center' }),
	);
	await expect(engine).toContainText('Xray Core');
	await expect(engine).toBeDisabled(); // Browser demo does not change native settings.
	await expect(page.getByText(ru.settings.coreEngineDesc)).toBeVisible();
	await page.screenshot({ path: 'test-results/core-engine-settings.png' });
});
