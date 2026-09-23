import { get, type Readable } from 'svelte/store';
import { catalog } from './index';

interface MaintenanceCatalog {
	state: Readable<{ subscriptions: { id: string }[] }>;
	getServers(): Promise<{ id: string }[]>;
	pingServer(id: string): Promise<unknown>;
	refreshSubscription(id: string): Promise<unknown>;
}

async function runLimited<T>(
	items: readonly T[],
	limit: number,
	task: (item: T) => Promise<unknown>,
) {
	let next = 0;
	await Promise.allSettled(
		Array.from({ length: Math.min(limit, items.length) }, async () => {
			while (next < items.length) {
				const item = items[next++];
				try {
					await task(item);
				} catch {
					/* Keep processing other items. */
				}
			}
		}),
	);
}

export function createMaintenance(
	service: MaintenanceCatalog,
	wait: (ms: number) => Promise<void> = (ms) =>
		new Promise((resolve) => setTimeout(resolve, ms)),
) {
	let refreshing: Promise<void> | null = null;
	let probing: Promise<void> | null = null;
	function pingAllServers(): Promise<void> {
		if (probing) return probing;
		probing = (async () => {
			const servers = await service.getServers();
			await runLimited(servers, 4, (server) => service.pingServer(server.id));
		})().finally(() => {
			probing = null;
		});
		return probing;
	}
	function refreshAllSubscriptions(): Promise<void> {
		if (refreshing) return refreshing;
		refreshing = (async () => {
			await service.getServers();
			const subscriptions = [...get(service.state).subscriptions];
			await runLimited(subscriptions, 2, async (subscription) => {
				for (let attempt = 0; attempt < 3; attempt++) {
					try {
						await service.refreshSubscription(subscription.id);
						return;
					} catch {
						if (attempt === 2) return;
						await wait(2000 * 2 ** attempt);
					}
				}
			});
		})().finally(() => {
			refreshing = null;
		});
		return refreshing;
	}
	return { pingAllServers, refreshAllSubscriptions };
}

export const { pingAllServers, refreshAllSubscriptions } =
	createMaintenance(catalog);
