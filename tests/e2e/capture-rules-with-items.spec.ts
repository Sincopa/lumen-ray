import { test } from '@playwright/test';

test('capture rule row with switch', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Правила', exact: true }).click();
	await page.waitForTimeout(200);

	// Add a rule
	await page
		.getByRole('region', { name: 'Правила', exact: true })
		.getByRole('button', { name: 'Добавить', exact: true })
		.click();
	await page.waitForTimeout(200);
	await page.locator('#domain-input').fill('youtube.com');
	await page.getByRole('button', { name: 'Сохранить', exact: true }).click();
	await page.waitForTimeout(300);

	await page.screenshot({
		path: 'C:/Users/user/.gemini/antigravity/brain/313795e2-8fd5-4114-b3af-5c82fe56fd3c/verify-rules-with-switch.png',
	});
});
