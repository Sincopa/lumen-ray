import { defineConfig } from '@playwright/test';
import { APP_WIDTH, APP_HEIGHT } from './src/lib/constants/app';
const isCI = Boolean(process.env.CI);
export default defineConfig({
	testDir: './tests/e2e',
	fullyParallel: false,
	workers: 1,
	reporter: 'list',
	maxFailures: isCI ? 3 : undefined,
	globalTimeout: isCI ? 10 * 60 * 1000 : undefined,
	use: {
		baseURL: 'http://127.0.0.1:5187',
		viewport: { width: APP_WIDTH, height: APP_HEIGHT },
		locale: 'ru-RU',
		launchOptions: isCI
			? {
					channel: 'chromium',
					args: ['--use-gl=angle', '--use-angle=swiftshader'],
				}
			: { channel: 'msedge' },
		screenshot: 'only-on-failure',
		trace: isCI ? 'retain-on-failure' : 'off',
	},
	webServer: {
		command:
			'node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 5187 --strictPort',
		url: 'http://127.0.0.1:5187',
		reuseExistingServer: false,
	},
});
