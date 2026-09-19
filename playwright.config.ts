import { defineConfig } from '@playwright/test';
export default defineConfig({
	testDir: './tests/e2e',
	fullyParallel: false,
	workers: 1,
	use: {
		baseURL: 'http://127.0.0.1:5187',
		viewport: { width: 390, height: 845 },
		locale: 'ru-RU',
		launchOptions: { channel: 'msedge' },
		screenshot: 'only-on-failure',
	},
	webServer: {
		command:
			'node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 5187 --strictPort',
		url: 'http://127.0.0.1:5187',
		reuseExistingServer: false,
	},
});
