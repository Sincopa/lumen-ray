import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import packageJson from './package.json' with { type: 'json' };

// `tauri android dev --host` sets TAURI_DEV_HOST so a phone can reach the server.
const devHost = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
	plugins: [svelte()],
	define: {
		__APP_VERSION__: JSON.stringify(packageJson.version),
	},
	server: {
		host: devHost || '127.0.0.1',
		port: 5173,
		strictPort: true,
		hmr: devHost ? { protocol: 'ws', host: devHost, port: 5174 } : undefined,
		// Rust and Gradle outputs change and lock files during native builds.
		watch: { ignored: ['**/src-tauri/**', '**/android/**', '**/release/**'] },
	},
	build: {
		target: ['chrome105', 'edge105', 'safari16'],
		cssTarget: ['chrome105', 'edge105', 'safari16'],
	},
});
