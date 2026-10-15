import { test, expect } from '@playwright/test';

declare global {
	interface Window {
		shaderTestStats: {
			draws: number;
			deformationUploads: number;
		};
	}
}

test('shader animates without reuploading static settings and pauses when disabled', async ({
	page,
}) => {
	await page.addInitScript(() => {
		window.shaderTestStats = { draws: 0, deformationUploads: 0 };
		const names = new WeakMap<WebGLUniformLocation, string>();
		const prototype = WebGL2RenderingContext.prototype;
		const getUniformLocation = prototype.getUniformLocation;
		prototype.getUniformLocation = function (program, name) {
			const location = getUniformLocation.call(this, program, name);
			if (location) names.set(location, name);
			return location;
		};
		const uniform1f = prototype.uniform1f;
		prototype.uniform1f = function (location, value) {
			if (location && names.get(location) === 'uDeformation') {
				window.shaderTestStats.deformationUploads++;
			}
			uniform1f.call(this, location, value);
		};
		const drawArrays = prototype.drawArrays;
		prototype.drawArrays = function (mode, first, count) {
			drawArrays.call(this, mode, first, count);
			window.shaderTestStats.draws++;
		};
	});

	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	page.on('console', (message) => {
		if (message.type() === 'error') errors.push(message.text());
	});
	await page.goto('/');
	await expect
		.poll(() => page.evaluate(() => window.shaderTestStats.draws))
		.toBeGreaterThan(5);
	expect(
		await page.evaluate(() => window.shaderTestStats.deformationUploads),
	).toBe(1);
	await page.getByRole('button', { name: 'Настройки', exact: true }).click();
	const enabled = page.getByRole('checkbox', {
		name: 'Анимированный фон',
		exact: true,
	});
	await enabled.uncheck();
	await expect(page.locator('canvas')).toHaveClass(/disabled/);
	const pausedDraws = await page.evaluate(() => window.shaderTestStats.draws);
	await page.waitForTimeout(200);
	expect(await page.evaluate(() => window.shaderTestStats.draws)).toBe(
		pausedDraws,
	);
	await enabled.check();
	await expect
		.poll(() => page.evaluate(() => window.shaderTestStats.draws))
		.toBeGreaterThan(pausedDraws + 2);
	expect(
		await page.evaluate(() => window.shaderTestStats.deformationUploads),
	).toBe(1);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	// Allow the single replacement frame requested by the media change.
	await page.evaluate(async () => {
		await new Promise<void>((resolve) =>
			requestAnimationFrame(() => resolve()),
		);
		await new Promise<void>((resolve) =>
			requestAnimationFrame(() => resolve()),
		);
	});
	const reducedMotionDraws = await page.evaluate(
		() => window.shaderTestStats.draws,
	);
	await page.waitForTimeout(200);
	expect(await page.evaluate(() => window.shaderTestStats.draws)).toBe(
		reducedMotionDraws,
	);
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	await expect
		.poll(() => page.evaluate(() => window.shaderTestStats.draws))
		.toBeGreaterThan(reducedMotionDraws + 2);
	expect(errors).toEqual([]);
});
