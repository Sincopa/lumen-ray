import { describe, it, expect, vi } from 'vitest';
import { get } from 'svelte/store';
import {
	CatalogService,
	LatencyProbeTracker,
	initialCatalog,
	validCatalog,
	validateSubscription,
	orderedRules,
	type StoragePort,
} from '../../src/lib/services/catalog';

it('combines presets in either order and preserves manual priority and deleted rows', async () => {
	const results = [];
	for (const order of [['russia-direct', 'bypass-blocking'], ['bypass-blocking', 'russia-direct']]) {
		const service = new CatalogService();
		service.saveRule({ id: 'manual', name: 'googlevideo.com', matcher: 'domain', serverId: 'block', enabled: true });
		for (const id of order) await service.applyPreset(id);
		const rules = get(service.state).rules;
		expect(rules[0].id).toBe('manual');
		expect(rules.findIndex((r) => r.name === 'youtube')).toBeLessThan(rules.findIndex((r) => r.name === 'ru'));
		results.push(rules);
		await service.applyPreset(order[0]);
		expect(get(service.state).rules).toEqual(rules);
		const deleted = rules.filter((r) => r.name !== 'youtube');
		expect(orderedRules(deleted).some((r) => r.name === 'youtube')).toBe(false);
		await service.removePreset('bypass-blocking');
		expect(get(service.state).rules.some((r) => r.name === 'youtube')).toBe(false);
		expect(get(service.state).rules[0].id).toBe('manual');
	}
	expect(results[0]).toEqual(results[1]);
});

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
	it('tracks each latency probe until its final overlapping run finishes', async () => {
		const tracker = new LatencyProbeTracker();
		let finishFirst!: () => void;
		let finishSecond!: () => void;
		const first = tracker.track(
			'server',
			() => new Promise<void>((resolve) => (finishFirst = resolve)),
		);
		const second = tracker.track(
			'server',
			() => new Promise<void>((resolve) => (finishSecond = resolve)),
		);

		expect(get(tracker.state).has('server')).toBe(true);
		finishFirst();
		await first;
		expect(get(tracker.state).has('server')).toBe(true);
		finishSecond();
		await second;
		expect(get(tracker.state).has('server')).toBe(false);
	});

	it('derives a profile name when import name is omitted', async () => {
		const service = new CatalogService();
		await service.importSubscription('https://sub.example.com/profile');
		expect(get(service.state).subscriptions.at(-1)?.name).toBe(
			'sub.example.com',
		);
	});

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

		const source = 'https://example.com/private-token';
		await service.importSubscription(source, 'Test');

		expect(JSON.stringify(get(service.state))).not.toContain('private-token');
		expect(disk.getItem('lumen_catalog_v1')).not.toContain('private-token');
		expect(get(service.state).subscriptions).toHaveLength(2);
		const imported = get(service.state).subscriptions.at(-1)!;
		await expect(service.getSubscriptionSource(imported.id)).resolves.toBe(
			source,
		);
		await expect(
			new CatalogService(disk).getSubscriptionSource(imported.id),
		).rejects.toThrow('invalidSubscription');
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

	it('accepts quotes, BOM, hysteria2, multiline keys, and domain links in validateSubscription', () => {
		expect(() =>
			validateSubscription('  "https://example.com/sub"  '),
		).not.toThrow();
		expect(() =>
			validateSubscription('\uFEFFhttps://example.com/sub'),
		).not.toThrow();
		expect(() =>
			validateSubscription('sub.example.com/api/token'),
		).not.toThrow();
		expect(() =>
			validateSubscription('hysteria2://pass@example.com:443?sni=example.com'),
		).not.toThrow();
		expect(() =>
			validateSubscription(
				'vless://00000000-0000-4000-8000-000000000001@1.1.1.1:443?type=tcp\nvless://00000000-0000-4000-8000-000000000002@2.2.2.2:443?type=tcp',
			),
		).not.toThrow();
	});

	it('normalizes geo tags and handles prefix stripping and mismatch rejection', async () => {
		const { normalizeGeoTag } = await import(
			'../../src/lib/services/catalog'
		);
		expect(normalizeGeoTag('ru', 'geoip')).toBe('ru');
		expect(normalizeGeoTag('geoip:ru', 'geoip')).toBe('ru');
		expect(normalizeGeoTag('geosite:category-ru', 'geosite')).toBe(
			'category-ru',
		);
		expect(normalizeGeoTag('category-ru', 'geosite')).toBe('category-ru');
		expect(normalizeGeoTag('antifilter-community', 'geosite')).toBe(
			'antifilter-community',
		);
		expect(normalizeGeoTag('private', 'geoip')).toBe('private');

		// Mismatched prefixes must throw
		expect(() => normalizeGeoTag('geosite:ru', 'geoip')).toThrow(
			'invalidRule',
		);
		expect(() => normalizeGeoTag('geoip:category-ru', 'geosite')).toThrow(
			'invalidRule',
		);

		// Malformed or unsafe characters
		expect(() => normalizeGeoTag('invalid/tag', 'geoip')).toThrow(
			'invalidRule',
		);
		expect(() => normalizeGeoTag('tag with space', 'geosite')).toThrow(
			'invalidRule',
		);
		expect(() => normalizeGeoTag('', 'geosite')).toThrow('invalidRule');
	});

	it('saves and compiles geosite, geoip, domain, and application rules', () => {
		const service = new CatalogService();
		service.saveRule({
			id: 'geo-1',
			name: 'geosite:category-ru',
			matcher: 'geosite',
			serverId: 'direct',
			enabled: true,
		});
		service.saveRule({
			id: 'geo-2',
			name: 'geoip:private',
			matcher: 'geoip',
			serverId: 'direct',
			enabled: true,
		});
		service.saveRule({
			id: 'custom-app',
			name: 'Code.exe',
			matcher: 'application',
			serverId: 'demo-fi',
			enabled: true,
		});

		const rules = get(service.state).rules;
		const r1 = rules.find((r) => r.id === 'geo-1');
		const r2 = rules.find((r) => r.id === 'geo-2');
		const r3 = rules.find((r) => r.id === 'custom-app');

		expect(r1?.name).toBe('category-ru');
		expect(r1?.matcher).toBe('geosite');
		expect(r1?.isDomain).toBe(false);

		expect(r2?.name).toBe('private');
		expect(r2?.matcher).toBe('geoip');
		expect(r2?.isDomain).toBe(false);

		expect(r3?.name).toBe('Code.exe');
		expect(r3?.matcher).toBe('application');
		expect(r3?.isDomain).toBe(false);
	});

	it('applies presets idempotently and preserves custom user rules', async () => {
		const service = new CatalogService();

		// Add custom user rule
		service.saveRule({
			id: 'user-custom-rule',
			name: 'my-custom-domain.com',
			matcher: 'domain',
			serverId: 'direct',
			enabled: true,
		});

		// Apply preset "russia-optimized"
		await service.applyPreset('russia-optimized');
		let rules = get(service.state).rules;

        expect(rules.length).toBe(9);
        expect(rules[0].id).toBe('user-custom-rule');
        const presetRules = rules.filter((r) => r.presetId === 'russia-optimized');
        expect(presetRules.map((r) => r.name)).toEqual([
            'private', 'youtube', 'antifilter', 'antifilter-community',
            'category-ru', 'antifilter', 'antifilter-community', 'ru',
        ]);
        expect(presetRules[1].serverId).toBe('subscription');

		// Idempotency: applying again must not duplicate rules
		await service.applyPreset('russia-optimized');
		rules = get(service.state).rules;
		expect(rules.length).toBe(9);

		// Remove preset: custom user rule must remain intact
		await service.removePreset('russia-optimized');
		rules = get(service.state).rules;
		expect(rules.length).toBe(1);
		expect(rules[0].id).toBe('user-custom-rule');

		// Apply russia-direct preset and check exact order
		await service.applyPreset('russia-direct');
		const directPresetRules = get(service.state).rules.filter(
			(r) => r.presetId === 'russia-direct',
		);
        expect(directPresetRules.map((r) => r.name)).toEqual(['private', 'category-ru', 'ru']);

		// Verify custom rule with block target
		service.saveRule({
			id: 'block-rule',
			name: 'malware.com',
			matcher: 'domain',
			serverId: 'block',
			enabled: true,
		});
		const savedBlock = get(service.state).rules.find((r) => r.id === 'block-rule');
		expect(savedBlock?.serverId).toBe('block');
	});

	it('provides geo database information and mock updates', async () => {
		const { getGeoDatabaseInfo, updateGeoDatabases } = await import(
			'../../src/lib/services/geoDatabase'
		);
		const info = await getGeoDatabaseInfo();
		expect(info.version).toBe('202410152325');
		expect(info.source).toContain('NetworkKeeper/v2ray-rules-ru');
		expect(info.valid).toBe(true);

		const updated = await updateGeoDatabases();
		expect(updated.version).toBe('202410152325');
		expect(updated.valid).toBe(true);
	});
});
