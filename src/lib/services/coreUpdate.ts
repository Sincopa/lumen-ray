import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';
import { runtime } from '../runtime';
import { nativeSettings } from './settings';

interface CoreUpdateInfo {
	version: string;
}

type Status =
	'idle' | 'checking' | 'latest' | 'available' | 'installing' | 'error';

export const coreUpdate = (() => {
	const state = writable<{ status: Status; version: string | null }>({
		status: 'idle',
		version: null,
	});
	let busy = false;
	async function check() {
		if (!runtime.isTauri || busy) return;
		busy = true;
		state.update((value) => ({ ...value, status: 'checking' }));
		try {
			const update = await invoke<CoreUpdateInfo | null>('check_core_update');
			state.set({
				status: update ? 'available' : 'latest',
				version: update?.version ?? null,
			});
		} catch {
			state.update((value) => ({ ...value, status: 'error' }));
		} finally {
			busy = false;
		}
	}
	async function install() {
		if (!runtime.isTauri || busy) return;
		busy = true;
		state.update((value) => ({ ...value, status: 'installing' }));
		try {
			const version = await invoke<string>('install_core_update');
			nativeSettings.update((value) => ({ ...value, coreVersion: version }));
			state.set({ status: 'latest', version: null });
		} catch {
			state.update((value) => ({ ...value, status: 'error' }));
		} finally {
			busy = false;
		}
	}
	return { state, check, install };
})();
