import { describe, expect, it } from 'vitest';
import { writable } from 'svelte/store';
import { createMaintenance } from '../../src/lib/services/maintenance';

describe('subscription maintenance', () => {
	it('refreshes every subscription, retries failures and limits parallel fetches', async () => {
		const attempts = new Map<string, number>();
		let active = 0;
		let peak = 0;
		const service = {
			state: writable({
				subscriptions: ['a', 'b', 'c', 'd'].map((id) => ({ id })),
			}),
			getServers: async () => [],
			pingServer: async () => 0,
			refreshSubscription: async (id: string) => {
				active++;
				peak = Math.max(peak, active);
				await Promise.resolve();
				active--;
				attempts.set(id, (attempts.get(id) ?? 0) + 1);
				if (id === 'b' && attempts.get(id) === 1) throw new Error('temporary');
			},
		};
		const maintenance = createMaintenance(service, async () => {});
		await maintenance.refreshAllSubscriptions();
		expect([...attempts.keys()].sort()).toEqual(['a', 'b', 'c', 'd']);
		expect(attempts.get('b')).toBe(2);
		expect(peak).toBeLessThanOrEqual(2);
	});

	it('limits simultaneous latency probes to four', async () => {
		let active = 0;
		let peak = 0;
		const visited: string[] = [];
		const service = {
			state: writable({ subscriptions: [] }),
			getServers: async () =>
				Array.from({ length: 9 }, (_, index) => ({ id: `${index}` })),
			refreshSubscription: async () => {},
			pingServer: async (id: string) => {
				active++;
				peak = Math.max(peak, active);
				await Promise.resolve();
				visited.push(id);
				active--;
			},
		};
		await createMaintenance(service).pingAllServers();
		expect(visited).toHaveLength(9);
		expect(peak).toBe(4);
	});
});
