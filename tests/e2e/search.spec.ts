import { test, expect } from '@playwright/test';
import { initialCatalog } from '../../src/lib/services/catalog';
import ru from '../../src/locales/ru.json' with { type: 'json' };

test('server search matches country, protocol and subscription after indexing', async ({
	page,
}) => {
	await page.goto('/');
	await page.locator('.server-selector').click();
	await page.getByRole('button', { name: 'Поиск', exact: true }).click();
	const search = page.getByPlaceholder('Поиск по серверам...');
	await search.fill('НИДЕРЛАНДЫ');
	await expect(page.locator('.server-row')).toHaveCount(1);
	await expect(page.locator('.server-row')).toContainText('Нидерланды');
	await search.fill('vless');
	await expect(page.locator('.server-row')).toHaveCount(7);
	await search.fill('blessedvpn');
	await expect(page.locator('.server-row')).toHaveCount(7);
	await search.fill('missing');
	await expect(page.locator('.server-row')).toHaveCount(0);
	await search.fill('');
	await expect(page.locator('.server-row')).toHaveCount(7);
});

test('rule lookup and selection stay reactive after changing a server', async ({
	page,
}) => {
	const catalog = initialCatalog();
	catalog.rules = [
		{
			id: 'example',
			name: 'example.com',
			isDomain: true,
			serverId: 'demo-fi',
			enabled: true,
		},
		{
			id: 'other',
			name: 'other.com',
			isDomain: true,
			serverId: 'demo-de',
			enabled: true,
		},
	];
	await page.addInitScript((value) => {
		localStorage.setItem('lumen_catalog_v1', JSON.stringify(value));
	}, catalog);
	await page.goto('/');
	await page.getByRole('button', { name: 'Правила', exact: true }).click();
	const row = page.locator('.rule-row').filter({ hasText: 'example.com' });
	await expect(row.locator('.server-pill')).toContainText('Финляндия');
	await row
		.getByRole('button', { name: 'example.com: Сервера', exact: true })
		.click();
	await page.getByRole('button', { name: /Нидерланды/ }).click();
	await expect(row.locator('.server-pill')).toContainText('Нидерланды');
	await page
		.getByRole('button', { name: ru.rules.selectMode, exact: true })
		.click();
	const selection = row.getByRole('checkbox', {
		name: 'Выбрать example.com',
		exact: true,
	});
	await selection.click();
	await expect(selection).toHaveAttribute('aria-checked', 'true');
	await selection.click();
	await expect(selection).toHaveAttribute('aria-checked', 'false');
});
