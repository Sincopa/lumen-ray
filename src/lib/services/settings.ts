import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { runtime } from '../runtime';
export interface NativeSettings {
	networkMode: 'tun' | 'proxy';
	autoConnect: boolean;
	closeToTray: boolean;
	startMinimized: boolean;
	autoStart: boolean;
	dnsServer: string;
	bypassLan: boolean;
	updateIntervalHours: number;
	pingUrl: string;
	coreVersion: string;
}
export const nativeSettings = writable<NativeSettings>({
	networkMode: 'tun',
	autoConnect: false,
	closeToTray: true,
	startMinimized: false,
	autoStart: false,
	dnsServer: '1.1.1.1',
	bypassLan: true,
	updateIntervalHours: 1,
	pingUrl: 'http://www.google.com/generate_204',
	coreVersion: '',
});
export const settingsError = writable('');
let current: NativeSettings;
nativeSettings.subscribe((value) => {
	current = value;
});
if (runtime.isTauri)
	void invoke<NativeSettings>('get_settings')
		.then((value) => nativeSettings.set(value))
		.catch(() => settingsError.set('unavailable'));
export async function updateNativeSettings(
	patch: Partial<Omit<NativeSettings, 'coreVersion'>>,
) {
	try {
		const settings = {
			networkMode: current.networkMode,
			autoConnect: current.autoConnect,
			closeToTray: current.closeToTray,
			startMinimized: current.startMinimized,
			autoStart: current.autoStart,
			dnsServer: current.dnsServer,
			bypassLan: current.bypassLan,
			updateIntervalHours: current.updateIntervalHours,
			pingUrl: current.pingUrl,
			...patch,
		};
		nativeSettings.set(
			await invoke<NativeSettings>('set_settings', { settings }),
		);
		settingsError.set('');
	} catch {
		settingsError.set('unavailable');
	}
}

export async function resetNetworkSettings(): Promise<void> {
	if (runtime.isTauri) {
		await invoke('reset_network_settings');
	}
}
