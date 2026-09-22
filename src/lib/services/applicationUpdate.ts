import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';
import { runtime } from '../runtime';

export type ApplicationUpdateStatus =
	| 'idle'
	| 'checking'
	| 'latest'
	| 'available'
	| 'downloading'
	| 'unconfigured'
	| 'error';

interface NativeApplicationUpdate {
	configured: boolean;
	currentVersion: string;
	version?: string | null;
}

export interface ApplicationUpdateState {
	status: ApplicationUpdateStatus;
	currentVersion: string;
	version: string | null;
}

function createApplicationUpdateController() {
	const state = writable<ApplicationUpdateState>({
		status: 'idle',
		currentVersion: __APP_VERSION__,
		version: null,
	});
	let pendingCheck: Promise<void> | null = null;

	async function check(options: { silent?: boolean } = {}) {
		if (pendingCheck) return pendingCheck;
		if (!runtime.isTauri) {
			state.set({
				status: 'latest',
				currentVersion: __APP_VERSION__,
				version: null,
			});
			return;
		}

		if (!options.silent) {
			state.update((value) => ({ ...value, status: 'checking' }));
		}
		pendingCheck = invoke<NativeApplicationUpdate>('check_application_update')
			.then((result) => {
				state.set({
					status: !result.configured
						? 'unconfigured'
						: result.version
							? 'available'
							: 'latest',
					currentVersion: result.currentVersion,
					version: result.version ?? null,
				});
			})
			.catch((cause) => {
				console.error('[application-update] Update check failed', cause);
				state.update((value) => ({ ...value, status: 'error' }));
			})
			.finally(() => {
				pendingCheck = null;
			});
		return pendingCheck;
	}

	async function install() {
		if (!runtime.isTauri) return;
		state.update((value) => ({ ...value, status: 'downloading' }));
		try {
			await invoke('install_application_update');
			await check();
		} catch (cause) {
			console.error('[application-update] Update installation failed', cause);
			state.update((value) => ({ ...value, status: 'error' }));
		}
	}

	return { state, check, install };
}

export const applicationUpdate = createApplicationUpdateController();
