import { describe, it, expect, vi } from 'vitest';
import { get } from 'svelte/store';
import {
	CatalogService,
	initialCatalog,
	validCatalog,
	validateSubscription,
	type StoragePort,
} from '../../src/lib/services/catalog';

function storage(): StoragePort {
	const data = new Map<string, string>();
	return {
		getItem: (k) => data.get(k) ?? null,
		setItem: (k, v) => {
			data.set(k, v);
		},
	};
}

describe('catalog', () => {
	it('publishes and persists a batch once, retaining order and the last update per ID', () => {
		const disk = storage();
		const write = vi.spyOn(disk, 'setItem');
		const service = new CatalogService(disk);
		const notify = vi.fn();
		const unsubscribe = service.state.subscribe(notify);
		const rule = {
			id: 'first',
			name: 'example.com',
			isDomain: true,
			serverId: 'demo-fi',
			enabled: true,
		};
		service.saveRules([
			rule,
			{ ...rule, id: 'second' },
			{ ...rule, name: ' updated.com ' },
		]);
		expect(write).toHaveBeenCalledTimes(1);
		expect(notify).toHaveBeenCalledTimes(2); // Initial snapshot plus the complete batch.
		expect(
			get(service.state).rules.map(({ id, name }) => ({ id, name })),
		).toEqual([
			{ id: 'first', name: 'updated.com' },
			{ id: 'second', name: 'example.com' },
		]);
		unsubscribe();
	});

	it('does not partially save an invalid batch or a failed storage write', () => {
		const disk = storage();
		const write = vi.spyOn(disk, 'setItem');
		const service = new CatalogService(disk);
		const rule = {
			id: 'first',
			name: 'example.com',
			isDomain: true,
			serverId: 'demo-fi',
			enabled: true,
		};
		expect(() =>
			service.saveRules([rule, { ...rule, id: 'bad', serverId: 'missing' }]),
		).toThrow('invalidRule');
		expect(write).not.toHaveBeenCalled();
		expect(get(service.state).rules).toEqual([]);
		write.mockImplementation(() => {
			throw new Error('quota');
		});
		expect(() => service.saveRules([rule])).toThrow('storage');
		expect(get(service.state).rules).toEqual([]);
	});

	it('rebuilds the server index after import and removal', async () => {
		const service = new CatalogService();
		await service.selectServer('demo-fi');
		await service.importSubscription(
			'https://example.com/subscription',
			'Imported',
		);
		const snapshot = get(service.state);
		const id = snapshot.selectedServerId!;
		await expect(service.selectServer(id)).resolves.toBeUndefined();
		service.removeSubscription(snapshot.selectedSubscriptionId!);
		await expect(service.selectServer(id)).rejects.toThrow('invalidServer');
		await expect(service.selectServer('demo-fi')).resolves.toBeUndefined();
	});

	it('rejects duplicate IDs and dangling references in stored catalogs', () => {
		const catalog = initialCatalog();
		expect(validCatalog(catalog)).toBe(true);
		expect(
			validCatalog({
				...catalog,
				servers: [...catalog.servers, catalog.servers[0]],
			}),
		).toBe(false);
		expect(
			validCatalog({
				...catalog,
				subscriptions: [...catalog.subscriptions, catalog.subscriptions[0]],
			}),
		).toBe(false);
		expect(validCatalog({ ...catalog, subscriptions: [] })).toBe(false);
		expect(validCatalog({ ...catalog, selectedServerId: 'missing' })).toBe(
			false,
		);
		expect(
			validCatalog({
				...catalog,
				rules: [
					{
						id: 'rule',
						name: 'example.com',
						enabled: true,
						isDomain: true,
						serverId: 'missing',
					},
				],
			}),
		).toBe(false);
	});

	it('keeps unrelated servers, rules and selection when removing a subscription', async () => {
		const service = new CatalogService();
		await service.importSubscription(
			'https://example.com/subscription',
			'Other',
		);
		const selectedServerId = get(service.state).selectedServerId!;
		for (const serverId of ['demo-fi', selectedServerId]) {
			service.saveRule({
				id: serverId,
				name: 'example.com',
				isDomain: true,
				serverId,
				enabled: true,
			});
		}
		service.removeSubscription('demo');
		const remaining = get(service.state);
		expect(remaining.selectedServerId).toBe(selectedServerId);
		expect(remaining.servers.map((server) => server.id)).toEqual([
			selectedServerId,
		]);
		expect(remaining.rules.map((rule) => rule.serverId)).toEqual([
			selectedServerId,
		]);
		expect(validCatalog(remaining)).toBe(true);
	});

	it('persists selection and rules across service recreation', async () => {
		const disk = storage();
		const service = new CatalogService(disk);

		await service.selectServer('demo-nl');
		service.saveRule({
			id: 'rule',
			name: 'Discord.exe',
			isDomain: false,
			serverId: 'demo-nl',
			enabled: true,
		});

		const next = get(new CatalogService(disk).state);
		expect(next.selectedServerId).toBe('demo-nl');
		expect(next.rules[0].name).toBe('Discord.exe');
	});

	it('imports only demo metadata and never retains source credentials', async () => {
		const disk = storage();
		const service = new CatalogService(disk);

		await service.importSubscription(
			'https://example.com/private-token',
			'Test',
		);

		expect(JSON.stringify(get(service.state))).not.toContain('private-token');
		expect(disk.getItem('lumen_catalog_v1')).not.toContain('private-token');
		expect(get(service.state).subscriptions).toHaveLength(2);
	});

	it('removing subscription cleans dangling servers, selection and rules', () => {
		const service = new CatalogService();

		service.saveRule({
			id: 'rule',
			name: 'example.com',
			isDomain: true,
			serverId: 'demo-fi',
			enabled: true,
		});
		service.removeSubscription('demo');

		expect(get(service.state)).toMatchObject({
			selectedServerId: null,
			servers: [],
			rules: [],
			subscriptions: [],
		});
	});

	it('rejects invalid input and missing servers', async () => {
		for (const source of [
			'',
			'javascript:alert(1)',
			'http://example.com',
			'garbage',
		]) {
			expect(() => validateSubscription(source)).toThrow();
		}

		const service = new CatalogService();
		await expect(service.selectServer('missing')).rejects.toThrow();

		expect(() =>
			service.saveRule({
				id: 'bad',
				name: 'https://example.com/path',
				isDomain: true,
				serverId: 'demo-fi',
				enabled: true,
			}),
		).toThrow();
	});

	it('recovers from malformed storage and reports the issue', () => {
		const service = new CatalogService({
			getItem: () => '{',
			setItem: () => {},
		});

		expect(get(service.state).servers.length).toBeGreaterThan(0);
		expect(get(service.persistenceError)).toBe(true);
	});

	it('does not publish a change that failed to persist', async () => {
		const service = new CatalogService({
			getItem: () => null,
			setItem: () => {
				throw new Error('quota');
			},
		});

		await expect(service.selectServer('demo-de')).rejects.toThrow('storage');
		expect(get(service.state).selectedServerId).toBe('demo-fi');
	});
});
