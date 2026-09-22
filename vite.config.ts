import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import packageJson from './package.json' with { type: 'json' };

// https://vite.dev/config/
export default defineConfig({
	plugins: [svelte()],
	define: {
		__APP_VERSION__: JSON.stringify(packageJson.version),
	},
	server: { host: '127.0.0.1', port: 5173, strictPort: true },
	build: {
		target: ['chrome105', 'edge105', 'safari16'],
		cssTarget: ['chrome105', 'edge105', 'safari16'],
	},
});
