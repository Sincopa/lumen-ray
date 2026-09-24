import { test, expect } from '@playwright/test';

test('dropdown starts closed, toggles on click, dismisses on outside click and on scroll', async ({
	page,
}) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Настройки', exact: true }).click();
	await page.waitForTimeout(300);

	const dropdownLists = page.locator('.dropdown-list');
	const count = await dropdownLists.count();
	expect(count).toBeGreaterThan(0);

	// 1. None of the dropdowns should be visible on mount
	for (let i = 0; i < count; i++) {
		await expect(dropdownLists.nth(i)).not.toBeVisible();
	}

	const langTrigger = page.getByRole('combobox', {
		name: 'Язык интерфейса',
		exact: true,
	});
	const langList = page.locator('.dropdown-list[aria-label="Язык интерфейса"]');

	// 2. Click trigger to open
	await langTrigger.click();
	await expect(langList).toBeVisible();
	await page.screenshot({
		path: 'C:/Users/user/.gemini/antigravity/brain/313795e2-8fd5-4114-b3af-5c82fe56fd3c/verify-dropdown-opened.png',
	});

	// 3. Click trigger again to close
	await langTrigger.click();
	await expect(langList).not.toBeVisible();

	// 4. Click to open, then click outside
	await langTrigger.click();
	await expect(langList).toBeVisible();
	await page.locator('.section-title').first().click();
	await expect(langList).not.toBeVisible();

	// 5. Click to open, then scroll container -> must auto-dismiss
	await langTrigger.click();
	await expect(langList).toBeVisible();
	const scrollContainer = page
		.locator('.settings-content, .settings-view')
		.first();
	await scrollContainer.evaluate((el) => (el.scrollTop = 100));
	await page.waitForTimeout(100);
	await expect(langList).not.toBeVisible();

	// 6. Screenshot clean settings view
	await page.screenshot({
		path: 'C:/Users/user/.gemini/antigravity/brain/313795e2-8fd5-4114-b3af-5c82fe56fd3c/verify-dropdown-closed.png',
	});
});
