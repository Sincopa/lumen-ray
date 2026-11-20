import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { runtime } from '../runtime';
import { errorCode, type ErrorCode } from './errors';
import { nativeSettings } from './settings';
import silver from '../../assets/app-icons/silver.png';
import gold from '../../assets/app-icons/gold.png';
import blue from '../../assets/app-icons/blue.png';
import teal from '../../assets/app-icons/teal.png';
import purple from '../../assets/app-icons/purple.png';
import rainbow from '../../assets/app-icons/rainbow.png';

/**
 * Settings > App appearance. The ids match src-tauri/src/app/app_icon.rs and
 * the Android plugin's launcher aliases. Gold is the default.
 */
export const APP_ICONS = [
	{ id: 'silver', preview: silver },
	{ id: 'gold', preview: gold },
	{ id: 'blue', preview: blue },
	{ id: 'teal', preview: teal },
	{ id: 'purple', preview: purple },
	{ id: 'rainbow', preview: rainbow },
] as const;

export type AppIconId = (typeof APP_ICONS)[number]['id'];
export const DEFAULT_APP_ICON: AppIconId = 'gold';

const DEMO_KEY = 'lumen.appIcon';

export function isAppIcon(value: unknown): value is AppIconId {
	return APP_ICONS.some((icon) => icon.id === value);
}

/** The demo has no native side; the choice lives in local storage there. */
export function demoAppIcon(): AppIconId {
	try {
		const stored = localStorage.getItem(DEMO_KEY);
		return isAppIcon(stored) ? stored : DEFAULT_APP_ICON;
	} catch {
		return DEFAULT_APP_ICON;
	}
}

if (!runtime.isTauri)
	nativeSettings.update((value) => ({ ...value, appIcon: demoAppIcon() }));

/** Saves the icon and shows it on the window, the Dock or the home screen. */
export async function setAppIcon(icon: AppIconId): Promise<ErrorCode | null> {
	if (!runtime.isTauri) {
		try {
			localStorage.setItem(DEMO_KEY, icon);
		} catch {
			/* Storage refused; the pick lasts for this session. */
		}
		nativeSettings.update((value) => ({ ...value, appIcon: icon }));
		return null;
	}
	const previous = get(nativeSettings).appIcon;
	nativeSettings.update((value) => ({ ...value, appIcon: icon }));
	try {
		await invoke('set_app_icon', { icon });
		return null;
	} catch (cause) {
		nativeSettings.update((value) => ({ ...value, appIcon: previous }));
		return errorCode(cause);
	}
}
