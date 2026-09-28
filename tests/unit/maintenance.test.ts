import { describe, expect, it } from 'vitest';
import { get, writable } from 'svelte/store';
import { createMaintenance } from '../../src/lib/services/maintenance';

describe('subscription maintenance', () => {
	it('does not retry provider rejection or device limits', async () => {
		for (const code of ['SUBSCRIPTION_ACCESS_DENIED', 'HWID_DEVICE_LIMIT']) {
			let calls = 0;
			const maintenance = createMaintenance(
				{
					state: writable({ subscriptions: [{ id: 'a' }] }),
					getServers: async () => [],
					pingServer: async () => 0,
					refreshSubscription: async () => {
						calls++;
						throw { code };
					},
				},
				async () => {},
			);
			await maintenance.refreshAllSubscriptions();
			expect(calls).toBe(1);
		}
	});
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
				if (id === 'b' && attempts.get(id) === 1)
					throw { code: 'SUBSCRIPTION_FETCH_FAILED' };
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

	it('reuses recent automatic probes but keeps manual probing independent', async () => {
		let calls = 0;
		const service = {
			state: writable({ subscriptions: [] }),
			getServers: async () => [{ id: 'a' }],
			refreshSubscription: async () => {},
			pingServer: async () => {
				calls++;
				return 20;
			},
		};
		const maintenance = createMaintenance(service);
		await maintenance.pingAllServers();
		await maintenance.pingAllServers();
		expect(calls).toBe(1);
		await service.pingServer();
		expect(calls).toBe(2);
	});

	it('exposes a safe failure code and clears it after a successful retry', async () => {
		let fail = true;
		const service = {
			state: writable({ subscriptions: [{ id: 'private' }] }),
			getServers: async () => [],
			pingServer: async () => 0,
			refreshSubscription: async () => {
				if (fail)
					throw { code: 'SUBSCRIPTION_FETCH_FAILED', message: 'private URL' };
			},
		};
		const maintenance = createMaintenance(service, async () => {});
		await maintenance.refreshAllSubscriptions();
		expect(get(maintenance.refreshStatus).private.error).toBe(
			'subscriptionFetchFailed',
		);
		expect(JSON.stringify(get(maintenance.refreshStatus))).not.toContain(
			'private URL',
		);
		fail = false;
		await maintenance.refreshSubscriptionWithStatus('private');
		expect(get(maintenance.refreshStatus).private.state).toBe('success');
		expect(get(maintenance.refreshStatus).private.lastSuccess).toBeTypeOf(
			'number',
		);
		expect(get(maintenance.refreshStatus).private.error).toBeUndefined();
	});
});
