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
	brightnessOpen = $state(false);
	pendingImport = $state('');

	goToTab = (tab: TabName) => {
		if (tab === this.currentTab) return;
		this.prevTab = this.currentTab;
		this.currentTab = tab;
		if (tab !== 'rules') {
			this.ruleChrome = 'default';
			this.selectionBar = null;
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

	chooseServer = async (server: { id: string }) => {
		const snapshot = get(connection.state);
		if (!canSwitch(snapshot.status)) return;
		const wasActive = isActive(snapshot.status);
		try {
			if (wasActive) {
				if (snapshot.info?.serverId !== server.id) {
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
			}
			this.goToTab('home');
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
