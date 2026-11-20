import { get } from 'svelte/store';
import { catalog, connection } from '../lib/services';
import { errorCode } from '../lib/services/errors';
import { canSwitch, isActive } from '../lib/services/connection';
import { t } from '../lib/i18n';
import { runtime } from '../lib/runtime';

export type TabName =
	'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';

/** Rule selection mode borrows the bottom bar for delete and done. */
export interface SelectionBar {
	count: number;
	onDelete: () => void;
	onDone: () => void;
}

const tabOrder: Record<TabName, number> = {
	home: 0,
	subscriptions: 1,
	servers: 2,
	rules: 3,
	settings: 4,
};

class Shell {
	currentTab = $state<TabName>('home');
	prevTab = $state<TabName>('home');
	importing = $state(false);
	actionError = $state('');
	ruleChrome = $state<'default' | 'selection' | 'picker'>('default');
	selectionBar = $state<SelectionBar | null>(null);
	/** Opens the rule presets; set while the rules list is on screen. */
	presetsAction = $state<(() => void) | null>(null);
	brightnessOpen = $state(false);
	pendingImport = $state('');

	goToTab = (tab: TabName) => {
		if (tab === this.currentTab) return;
		this.prevTab = this.currentTab;
		this.currentTab = tab;
		if (tab !== 'rules') {
			this.ruleChrome = 'default';
			this.selectionBar = null;
			this.presetsAction = null;
		}
	};

	direction = (to: string, from: string) => {
		return (tabOrder[to as TabName] ?? 0) > (tabOrder[from as TabName] ?? 0)
			? 1
			: -1;
	};

	toggleConnection = () => {
		connection.toggle(get(catalog.state).selectedServerId);
	};

	/**
	 * A pick returns to the home screen at once. A connection on its way up is
	 * superseded by the new server instead of making the user wait for it.
	 */
	chooseServer = async (server: { id: string }) => {
		const snapshot = get(connection.state);
		if (snapshot.status === 'initializing') return;
		const wasActive = isActive(snapshot.status) && canSwitch(snapshot.status);
		// Where the tunnel is, or is heading: picking it again changes nothing.
		const current =
			snapshot.status === 'connected'
				? snapshot.info?.serverId
				: (snapshot.target ?? undefined);
		this.goToTab('home');
		try {
			if (wasActive) {
				if (current !== server.id) {
					await connection.switchServer(server.id);
				}
				const after = get(connection.state);
				if (
					!runtime.isTauri &&
					after.status === 'connected' &&
					after.info?.serverId === server.id
				) {
					// The native backend persists successful switches; the demo uses local storage.
					await catalog.selectServer(server.id);
				}
			} else {
				await catalog.selectServer(server.id);
				// The pick waited for a background connection (start-up or recovery)
				// that came up on the old server meanwhile. The user's choice wins.
				const after = get(connection.state);
				const heading =
					after.status === 'connected'
						? after.info?.serverId
						: (after.target ?? undefined);
				if (
					isActive(after.status) &&
					canSwitch(after.status) &&
					heading !== server.id
				) {
					await connection.switchServer(server.id);
				}
			}
		} catch (e) {
			this.actionError = t('errors.' + errorCode(e));
		}
	};

	changeMode = async (mode: 'global' | 'rules') => {
		// The backend orders this after an in-flight connection; the button
		// reflects the new mode immediately.
		try {
			await catalog.setRoutingMode(mode);
		} catch (e) {
			this.actionError = t('errors.' + errorCode(e));
		}
	};
}

export const shell = new Shell();
