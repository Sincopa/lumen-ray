import { get, writable, type Readable } from 'svelte/store';
import { catalog } from './index';
import { errorCode, type ErrorCode } from './errors';

export interface SubscriptionRefreshStatus {
	state: 'refreshing' | 'success' | 'error';
	lastAttempt: number;
	lastSuccess?: number;
	error?: ErrorCode;
}

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
	const nextProbeAt = new Map<string, number>();
	const refreshStatus = writable<Record<string, SubscriptionRefreshStatus>>({});
	function publishStatus(
		id: string,
		update: Partial<SubscriptionRefreshStatus>,
	) {
		refreshStatus.update((current) => ({
			...current,
			[id]: {
				...current[id],
				state: 'refreshing',
				lastAttempt: Date.now(),
				...update,
			},
		}));
	}
	async function refreshSubscriptionWithStatus(
		id: string,
		retries = 0,
	): Promise<void> {
		publishStatus(id, { state: 'refreshing', error: undefined });
		for (let attempt = 0; attempt <= retries; attempt++) {
			try {
				await service.refreshSubscription(id);
				publishStatus(id, {
					state: 'success',
					lastSuccess: Date.now(),
					error: undefined,
				});
				return;
			} catch (cause) {
				if (
					attempt === retries ||
					!['subscriptionFetchFailed', 'timeout'].includes(errorCode(cause))
				) {
					publishStatus(id, { state: 'error', error: errorCode(cause) });
					throw cause;
				}
				await wait(2000 * 2 ** attempt);
			}
		}
	}
	function pingAllServers(): Promise<void> {
		if (probing) return probing;
		probing = (async () => {
			const servers = await service.getServers();
			const currentIds = new Set(servers.map((server) => server.id));
			for (const id of nextProbeAt.keys())
				if (!currentIds.has(id)) nextProbeAt.delete(id);
			const now = Date.now();
			await runLimited(
				servers.filter((server) => now >= (nextProbeAt.get(server.id) ?? 0)),
				4,
				async (server) => {
					const latency = await service.pingServer(server.id);
					nextProbeAt.set(
						server.id,
						Date.now() + (typeof latency === 'number' ? 60_000 : 15_000),
					);
				},
			);
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
			await runLimited(subscriptions, 2, (subscription) =>
				refreshSubscriptionWithStatus(subscription.id, 2),
			);
		})().finally(() => {
			refreshing = null;
		});
		return refreshing;
	}
	return {
		pingAllServers,
		refreshAllSubscriptions,
		refreshSubscriptionWithStatus,
		refreshStatus,
	};
}

export const {
	pingAllServers,
	refreshAllSubscriptions,
	refreshSubscriptionWithStatus,
	refreshStatus,
} = createMaintenance(catalog);
