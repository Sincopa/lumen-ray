import { onMount } from 'svelte';
import { get, type Readable } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { listen } from '@tauri-apps/api/event';
import { catalog, connection } from '../lib/services';
import { shaderSettings } from '../lib/stores/shaderSettings.svelte';
import { runtime } from '../lib/runtime';
import { APP_HEIGHT, APP_WIDTH } from '../lib/constants/app';
import { nativeSettings, settingsReady } from '../lib/services/settings';
import { applicationUpdate } from '../lib/services/applicationUpdate';
import { importPreview } from '../lib/services/importPreview';
import {
	pingAllServers,
	refreshAllSubscriptions,
} from '../lib/services/maintenance';
import { i18n } from '../lib/i18n';
import { shell } from './shell.svelte';

function watchStore<T>(store: Readable<T>) {
	let current = $state(get(store));
	$effect(() => {
		return store.subscribe((value) => {
			current = value;
		});
	});
	return {
		get current() {
			return current;
		},
	};
}

/** Resize the native window to the UI scale. In the browser, zoom the viewport instead. */
async function applyScale(scale: number) {
	const w = Math.round(APP_WIDTH * scale);
	const h = Math.round(APP_HEIGHT * scale);

	document.documentElement.style.setProperty('--ui-zoom', String(scale));

	if (runtime.isTauri) {
		try {
			await getCurrentWindow().setSize(new LogicalSize(w, h));
		} catch (e) {
			console.error('[scale] Failed to resize Tauri window:', e);
		}
	}
}

function notify(kind: 'disconnect' | 'reconnect' | 'expiry' | 'update') {
	if (!runtime.isTauri) return Promise.resolve();
	return invoke<void>('send_lumen_notification', {
		kind,
		language: i18n.currentLanguage,
	});
}

async function startMaintenance() {
	const ready = await Promise.allSettled([settingsReady, catalog.getServers()]);
	if (ready.some((result) => result.status === 'rejected')) return;
	const settings = get(nativeSettings);
	if (settings.pingPolicy === 'startup') void pingAllServers().catch(() => {});
	if (settings.updateIntervalHours > 0)
		void refreshAllSubscriptions().catch(() => {});
}

export function installLifecycle() {
	const settings = watchStore(nativeSettings);
	const connectionSnapshot = watchStore(connection.state);
	const catalogSnapshot = watchStore(catalog.state);
	const updates = watchStore(applicationUpdate.state);

	$effect(() => {
		void applyScale(shaderSettings.uiScale);
	});

	onMount(() => {
		if (runtime.isTauri) {
			void invoke('frontend_ready').catch(() => {});
		}
		void connection.initialize();
		if (runtime.isTauri) {
			void startMaintenance();
		}
		void applicationUpdate.check({ silent: true });

		const handleContextMenu = (e: MouseEvent) => {
			e.preventDefault();
		};
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'Tab') {
				e.preventDefault();
			}
		};
		window.addEventListener('contextmenu', handleContextMenu);
		window.addEventListener('keydown', handleKeyDown);

		let unlistenNavigate: (() => void) | undefined;
		if (runtime.isTauri) {
			listen('navigate:settings', () => {
				shell.goToTab('settings');
			}).then((unlisten) => {
				unlistenNavigate = unlisten;
			});
		}

		return () => {
			window.removeEventListener('contextmenu', handleContextMenu);
			window.removeEventListener('keydown', handleKeyDown);
			unlistenNavigate?.();
		};
	});

	$effect(() => {
		const hours = settings.current.updateIntervalHours ?? 24;
		if (hours <= 0) return;
		const timer = setInterval(
			() => {
				void refreshAllSubscriptions().catch(() => {});
			},
			hours * 3600 * 1000,
		);
		return () => {
			clearInterval(timer);
		};
	});

	$effect(() => {
		if (
			runtime.isTauri &&
			shell.currentTab === 'servers' &&
			settings.current.pingPolicy === 'list'
		) {
			void pingAllServers().catch(() => {});
		}
	});

	let wasConnected = false;
	let interrupted = false;
	$effect(() => {
		const status = connectionSnapshot.current.status;
		if (status === 'connected') {
			if (interrupted) void notify('reconnect').catch(() => {});
			wasConnected = true;
			interrupted = false;
		} else if (status === 'error' && wasConnected) {
			wasConnected = false;
			interrupted = true;
			void notify('disconnect').catch(() => {});
		} else if (status === 'disconnected') {
			wasConnected = false;
			interrupted = false;
		}
	});

	$effect(() => {
		const version = updates.current.version;
		if (!runtime.isTauri || !settings.current.notifyUpdate || !version) return;
		const key = `lumen-update-notified-${version}`;
		if (localStorage.getItem(key)) return;
		localStorage.setItem(key, '1');
		void notify('update').catch(() => localStorage.removeItem(key));
	});

	$effect(() => {
		if (!runtime.isTauri || !settings.current.notifyExpiry) return;
		const marked: string[] = [];
		for (const subscription of catalogSnapshot.current.subscriptions) {
			if (typeof subscription.daysRemaining !== 'number') continue;
			const key = `lumen-expiry-notified-${subscription.id}`;
			if (subscription.daysRemaining > 7) {
				localStorage.removeItem(key);
				continue;
			}
			if (localStorage.getItem(key)) continue;
			localStorage.setItem(key, '1');
			marked.push(key);
		}
		if (marked.length)
			void notify('expiry').catch(() => {
				for (const key of marked) localStorage.removeItem(key);
			});
	});

	onMount(() => {
		if (!runtime.isTauri) return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		const read = async () => {
			const value = await invoke<string | null>('take_import_link');
			if (disposed || !value) return;
			// The link only fills the add-profile form; importing still needs a confirm.
			try {
				shell.pendingImport = importPreview(value).input;
				shell.importing = true;
			} catch {
				/* Ignore malformed links. */
			}
		};
		void listen('import://pending', () => {
			void read().catch(() => {});
		})
			.then((off) => {
				if (disposed) {
					off();
					return;
				}
				unlisten = off;
				return read();
			})
			.catch(() => {});
		return () => {
			disposed = true;
			unlisten?.();
		};
	});
}
