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
	// A failed background check (no network yet, the tunnel still coming up)
	// tries again a little later instead of waiting for the next launch.
	const retryDelays = [30_000, 120_000, 600_000];
	let retries = 0;
	let retryTimer: ReturnType<typeof setTimeout> | undefined;

	async function check(options: { silent?: boolean } = {}) {
		if (pendingCheck) {
			// The user asked while a background check runs: show that it does.
			if (!options.silent) {
				state.update((value) =>
					value.status === 'downloading'
						? value
						: { ...value, status: 'checking' },
				);
			}
			return pendingCheck;
		}
		clearTimeout(retryTimer);
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
				retries = 0;
				state.set({
					status: result.version ? 'available' : 'latest',
					currentVersion: result.currentVersion,
					version: result.version ?? null,
					progress: null,
				});
			})
			.catch((cause) => {
				console.error('[application-update] Update check failed', cause);
				let shown = false;
				state.update((value) => {
					shown = value.status === 'checking';
					return shown ? { ...value, status: 'error' } : value;
				});
				if (!shown && retries < retryDelays.length) {
					retryTimer = setTimeout(
						() => void check({ silent: true }),
						retryDelays[retries++],
					);
				}
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
