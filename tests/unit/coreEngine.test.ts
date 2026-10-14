import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { errorCode } from '../../src/lib/services/errors';
import { validateSubscription } from '../../src/lib/services/catalog';
import { importPreview } from '../../src/lib/services/importPreview';

const ipc = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ipc);
vi.mock('../../src/lib/runtime', () => ({ runtime: { isTauri: true } }));

describe('core engine settings', () => {
	it('persists the choice without sending a connect or disconnect command', async () => {
		let saved = {
			coreEngine: 'xray',
			networkMode: 'tun',
			autoConnect: false,
			coreEngines: [],
		};
		ipc.invoke.mockImplementation(
			async (command: string, args?: { settings: typeof saved }) => {
				if (command === 'get_settings') return { ...saved };
				if (command === 'set_settings' && args) {
					saved = { ...args.settings };
					return { ...saved };
				}
				throw new Error('Unexpected networking operation');
			},
		);
		const settings = await import('../../src/lib/services/settings');
		await settings.settingsReady;
		await settings.updateNativeSettings({ coreEngine: 'lumen' });
		expect(get(settings.nativeSettings).coreEngine).toBe('lumen');
		expect(saved.networkMode).toBe('tun');
		await settings.updateNativeSettings({ autoConnect: true });
		expect(saved.coreEngine).toBe('lumen');
		expect(ipc.invoke.mock.calls.map((call) => call[0])).toEqual([
			'get_settings',
			'set_settings',
			'set_settings',
		]);
		vi.resetModules();
		const restarted = await import('../../src/lib/services/settings');
		await restarted.settingsReady;
		expect(get(restarted.nativeSettings).coreEngine).toBe('lumen');
	});
	it('accepts native proxy imports and keeps credentials out of previews', () => {
		for (const scheme of ['socks', 'socks5', 'http-proxy']) {
			const input = `${scheme}://fixture:private-password@127.0.0.1:1080`;
			expect(() => validateSubscription(input)).not.toThrow();
			expect(importPreview(input).description).not.toContain(
				'private-password',
			);
		}
	});
	it('maps unsupported configurations to the actionable engine error', () => {
		expect(errorCode({ code: 'LUMEN_UNSUPPORTED_CONFIGURATION' })).toBe(
			'lumenUnsupported',
		);
	});
	it('maps tunnel and backup failures onto known messages', () => {
		expect(errorCode({ code: 'LIVE_SWITCH_FAILED' })).toBe('unavailable');
		expect(errorCode({ code: 'IP_UNAVAILABLE' })).toBe('unavailable');
		expect(errorCode({ code: 'INVALID_CONFIG' })).toBe('unavailable');
		expect(errorCode({ code: 'DNS_UNAVAILABLE' })).toBe('unavailable');
		expect(errorCode({ code: 'BACKUP_INVALID' })).toBe('storage');
		expect(errorCode({ code: 'GEO_UPDATE_FAILED' })).toBe('geoDataMissing');
		expect(errorCode({ code: 'ALREADY_RUNNING' })).toBe('unavailable');
	});
});
