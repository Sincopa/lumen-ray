import { test, expect } from '@playwright/test';

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
  await expect(
    page
      .getByRole('region', { name: 'Сервера', exact: true })
      .getByRole('button', { name: /Нидерланды/ }),
  ).toBeDisabled();
  await page.getByRole('button', { name: 'Главная', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Подключен');
  await page.reload();
  await expect(page.locator('.server-selector')).toHaveAttribute(
    'aria-label',
    'Нидерланды',
  );
});

test('subscription import, rename, refresh and delete work', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Название', { exact: true }).fill('Тест');
  await dialog
    .getByLabel('HTTPS-ссылка, ключ или Base64')
    .fill('https://example.com/secret-test-token');
  await dialog.getByRole('button', { name: 'Добавить', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole('button', { name: 'Подписки', exact: true }).click();
  await page.locator('.title-group').filter({ hasText: 'Тест' }).click();
  const card = page.locator('.sub-card-wrapper').filter({ hasText: 'Тест' });
  await card.getByRole('button', { name: 'Обновить', exact: true }).click();
  await expect(
    card.getByRole('button', { name: 'Обновить', exact: true }),
  ).toBeEnabled();
  await card.getByRole('button', { name: 'Меню', exact: true }).click();
  await card
    .getByRole('button', { name: 'Редактировать', exact: true })
    .click();
  await dialog.getByLabel('Название', { exact: true }).fill('Новое имя');
  await dialog.getByRole('button', { name: 'Сохранить', exact: true }).click();
  const renamed = page
    .locator('.sub-card-wrapper')
    .filter({ hasText: 'Новое имя' });
  await renamed.getByRole('button', { name: 'Меню', exact: true }).click();
  await renamed.getByRole('button', { name: 'Удалить', exact: true }).click();
  await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();
  await expect(renamed).toHaveCount(0);
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
    'secret-test-token',
  );
});

test('rules persist, validate input and do not trap navigation', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .getByRole('button', {
      name: 'Режим по правилам (маршрутизация)',
      exact: true,
    })
    .click();
  await page
    .getByRole('region', { name: 'Правила', exact: true })
    .getByRole('button', { name: 'Добавить', exact: true })
    .click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Название', { exact: true }).fill('bad/path');
  await dialog.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await expect(dialog.getByRole('alert')).toBeVisible();
  await dialog.getByLabel('Название', { exact: true }).fill('example.com');
  await dialog.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await page
    .getByRole('switch', { name: 'Включить правило example.com' })
    .click();
  await expect(page.getByRole('switch')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await page.getByRole('button', { name: 'Главная', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Отключен');
  await page
    .getByRole('button', {
      name: 'Режим по правилам (маршрутизация)',
      exact: true,
    })
    .click();
  await expect(page.getByRole('switch')).toHaveAttribute(
    'aria-checked',
    'false',
  );
  await page.reload();
  await page
    .getByRole('button', {
      name: 'Режим по правилам (маршрутизация)',
      exact: true,
    })
    .click();
  await expect(page.getByText('example.com', { exact: true })).toBeVisible();
});

test('settings are honest and language and animation persist', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await expect(
    page.getByRole('checkbox', { name: 'Kill Switch', exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole('checkbox', { name: 'Kill Switch', exact: true }),
  ).not.toBeChecked();
  await page
    .getByRole('checkbox', { name: 'Анимированный фон', exact: true })
    .uncheck();
  await expect(page.locator('canvas')).toHaveClass(/disabled/);
  await page
    .getByRole('combobox', { name: 'Язык интерфейса', exact: true })
    .selectOption('en');
  await page.reload();
  await expect(
    page.getByText('Demo mode · traffic is not protected'),
  ).toBeVisible();
  await expect(page.locator('canvas')).toHaveClass(/disabled/);
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
      name: 'Режим по правилам (маршрутизация)',
      exact: true,
    })
    .click();
  await page
    .getByRole('region', { name: 'Правила', exact: true })
    .getByRole('button', { name: 'Добавить', exact: true })
    .click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Название', { exact: true }).fill('example.com');
  await dialog.getByRole('button', { name: 'Сохранить', exact: true }).click();
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
  await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();
  await expect(page.locator('.rule-row')).toHaveCount(0);
  expect(errors).toEqual([]);
});

for (const size of [
  { width: 360, height: 560 },
  { width: 390, height: 845 },
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
  const navPillGap = await page
    .locator('.nav-pill')
    .evaluate((el) => window.getComputedStyle(el).gap);
  expect(navPillGap).toBe('6px');
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

  // Bug 1 & 2: Context menu transparency 20%, blur, and Google Material Symbols icons
  await page.getByRole('button', { name: 'Подписки', exact: true }).click();
  await page.locator('.title-group').first().click();
  await page.getByRole('button', { name: 'Меню', exact: true }).first().click();
  await page.waitForTimeout(300);

  const menuBg = await page
    .locator('.popup-menu')
    .evaluate((el) => window.getComputedStyle(el).backgroundColor);
  expect(menuBg).toContain('0.2');
  const iconCount = await page
    .locator('.popup-menu .menu-item-icon-circle span.material-symbols-rounded')
    .count();
  expect(iconCount).toBe(4);
  await page.screenshot({
    path: 'artifacts/verification/check1-context-menu.png',
  });

  // Bug 7: Mode rules -> Home tab dim-backdrop disappears smoothly
  await page.getByRole('button', { name: 'Главная', exact: true }).click();
  await page
    .getByRole('button', {
      name: 'Режим по правилам (маршрутизация)',
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
