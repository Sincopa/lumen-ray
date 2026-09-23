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
	autoReconnect: boolean;
	serverSelection: 'manual' | 'best';
	pingPolicy: 'startup' | 'list' | 'manual';
	notifyDisconnect: boolean;
	notifyReconnect: boolean;
	notifyExpiry: boolean;
	notifyUpdate: boolean;
	coreVersion: string;
}
const defaults: NativeSettings = {
	networkMode: 'tun',
	autoConnect: false,
	closeToTray: true,
	startMinimized: false,
	autoStart: false,
	dnsServer: '1.1.1.1',
	bypassLan: true,
	updateIntervalHours: 1,
	pingUrl: 'http://www.google.com/generate_204',
	autoReconnect: false,
	serverSelection: 'manual',
	pingPolicy: 'startup',
	notifyDisconnect: false,
	notifyReconnect: false,
	notifyExpiry: false,
	notifyUpdate: false,
	coreVersion: '',
};
export const nativeSettings = writable<NativeSettings>(defaults);
export const settingsError = writable('');
let current: NativeSettings = defaults;
let saved: NativeSettings = defaults;
nativeSettings.subscribe((value) => {
	current = value;
});
const ready = runtime.isTauri
	? invoke<NativeSettings>('get_settings')
			.then((value) => {
				saved = value;
				nativeSettings.set(value);
			})
			.catch((error: unknown) => {
				settingsError.set('unavailable');
				throw error;
			})
	: Promise.resolve();
export const settingsReady = ready;
void ready.catch(() => {});
let writes: Promise<void> = Promise.resolve();
export async function updateNativeSettings(
	patch: Partial<Omit<NativeSettings, 'coreVersion'>>,
) {
	if (!runtime.isTauri) return;
	const operation = writes.then(async () => {
		await ready;
		const settings = { ...current, ...patch };
		nativeSettings.set(settings);
		try {
			const result = await invoke<NativeSettings>('set_settings', { settings });
			saved = result;
			nativeSettings.set(result);
			settingsError.set('');
		} catch (error) {
			try {
				saved = await invoke<NativeSettings>('get_settings');
			} catch {
				// The last confirmed snapshot remains the safe local value.
			}
			nativeSettings.set(saved);
			settingsError.set('unavailable');
			throw error;
		}
	});
	writes = operation.catch(() => {});
	return writes;
}

export async function resetNetworkSettings(): Promise<void> {
	if (runtime.isTauri) {
		await invoke('reset_network_settings');
	}
}
