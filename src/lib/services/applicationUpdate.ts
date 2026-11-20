import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';
import { runtime } from '../runtime';

export type ApplicationUpdateStatus =
	'idle' | 'checking' | 'latest' | 'available' | 'downloading' | 'error';

interface NativeApplicationUpdate {
	currentVersion: string;
	version?: string | null;
}

export interface ApplicationUpdateState {
	status: ApplicationUpdateStatus;
	currentVersion: string;
	version: string | null;
	/** Share of the update downloaded, 0-1, while it downloads; null if unknown. */
	progress: number | null;
}

function createApplicationUpdateController() {
	const state = writable<ApplicationUpdateState>({
		status: 'idle',
		currentVersion: __APP_VERSION__,
		version: null,
		progress: null,
	});
	let pendingCheck: Promise<void> | null = null;

	async function check(options: { silent?: boolean } = {}) {
		if (pendingCheck) return pendingCheck;
		if (!runtime.isTauri) {
			state.set({
				status: 'latest',
				currentVersion: __APP_VERSION__,
				version: null,
				progress: null,
			});
			return;
		}

		if (!options.silent) {
			state.update((value) => ({ ...value, status: 'checking' }));
		}
		pendingCheck = invoke<NativeApplicationUpdate>('check_application_update')
			.then((result) => {
				state.set({
					status: result.version ? 'available' : 'latest',
					currentVersion: result.currentVersion,
					version: result.version ?? null,
					progress: null,
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
		state.update((value) => ({ ...value, status: 'downloading', progress: 0 }));
		// The backend reports [downloaded, total] bytes as the package arrives.
		const unlisten = await listen<[number, number | null]>(
			'application-update-progress',
			({ payload: [downloaded, total] }) => {
				state.update((value) => ({
					...value,
					progress: total ? Math.min(1, downloaded / total) : null,
				}));
			},
		).catch(() => null);
		try {
			await invoke('install_application_update');
			await check();
		} catch (cause) {
			console.error('[application-update] Update installation failed', cause);
			state.update((value) => ({ ...value, status: 'error', progress: null }));
		} finally {
			unlisten?.();
		}
	}

	return { state, check, install };
}

export const applicationUpdate = createApplicationUpdateController();
