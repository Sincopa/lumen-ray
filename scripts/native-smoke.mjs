import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';

const executable = resolve(process.argv[2] ?? 'src-tauri/target/debug/app.exe');
await access(executable);
const reservation = createServer();
await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
const port = reservation.address().port;
await new Promise(resolve => reservation.close(resolve));
const profile = await mkdtemp(join(tmpdir(), 'misc-vpn-smoke-'));
const output = resolve('artifacts/verification');
await mkdir(output, { recursive: true });
const child = spawn(executable, [], {
  windowsHide: true,
  env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=' + port, WEBVIEW2_USER_DATA_FOLDER: profile },
  stdio: 'ignore',
});
let launchError;
child.on('error', error => { launchError = error; });
let browser;
try {
  const endpoint = 'http://127.0.0.1:' + port;
  let ready = false;
  for (let attempt = 0; attempt < 60; attempt++) {
    if (launchError) throw launchError;
    if (child.exitCode !== null) throw new Error('Native process exited before startup');
    try { const response = await fetch(endpoint + '/json/version'); ready = response.ok; } catch { /* WebView2 is starting. */ }
    if (ready) break;
    await delay(500);
  }
  if (!ready) throw new Error('WebView2 debugging endpoint did not become ready');
  browser = await chromium.connectOverCDP(endpoint);
  const context = browser.contexts()[0];
  const page = context.pages().find(page => /tauri.localhost|tauri:\/\/localhost/.test(page.url())) ?? context.pages()[0];
  if (!page) throw new Error('Native WebView was not created');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await expect(page.getByRole('status')).toHaveText('Отключен');
  await expect(page.getByText('Демо-режим · трафик не защищён')).toBeVisible();
  await page.getByRole('button', { name: 'Сервера', exact: true }).click();
  await page.getByRole('button', { name: /Нидерланды/ }).click();
  await page.getByRole('button', { name: 'Нажмите для подключения', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Подключен');
  await page.reload();
  await expect(page.getByRole('status')).toHaveText('Подключен');
  await expect(page.locator('.server-selector')).toHaveAttribute('aria-label', 'Нидерланды');
  await page.screenshot({ path: join(output, 'native-connected.png') });
  await page.getByRole('button', { name: 'Нажмите для отключения', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Отключен');
  await page.getByRole('button', { name: 'Развернуть', exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.innerWidth)).toBeGreaterThan(390);
  await page.getByRole('button', { name: 'Развернуть', exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(390);
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await expect(page.getByRole('checkbox', { name: 'Kill Switch', exact: true })).toBeDisabled();
  await page.screenshot({ path: join(output, 'native-settings.png') });
  expect(errors).toEqual([]);
  await page.getByRole('button', { name: 'Закрыть', exact: true }).click();
  for (let attempt = 0; attempt < 20 && child.exitCode === null; attempt++) await delay(100);
  expect(child.exitCode).toBe(0);
  console.log('Native WebView2 smoke passed: startup, selected server, connection, reload, disconnect, maximize, settings, close.');
  console.log('Screenshots: ' + output);
} finally {
  await browser?.close().catch(() => {});
  if (child.exitCode === null) child.kill();
}
