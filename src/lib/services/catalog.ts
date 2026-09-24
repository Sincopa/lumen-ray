import { writable } from 'svelte/store';
import { ServiceError } from './errors';

export interface Server {
	name?: string;
	id: string;
	subscriptionId: string;
	countryCode: string;
	nameKey: string;
	protocol: string;
	latency: number | 'timeout';
}

export interface Subscription {
	id: string;
	name: string;
	updatedAt: number;
	status?: 'active' | 'expired';
	daysRemaining?: number | string;
	traffic?: string;
	updatedMinutesAgo?: number;
}

export type RuleMatcher = 'domain' | 'application' | 'geosite' | 'geoip';

export interface Rule {
	id: string;
	name: string;
	matcher?: RuleMatcher;
	isDomain?: boolean;
	serverId: string;
	enabled: boolean;
	presetId?: string;
}

export interface Catalog {
	version: 1;
	servers: Server[];
	subscriptions: Subscription[];
	rules: Rule[];
	selectedServerId: string | null;
	selectedSubscriptionId?: string | null;
	selectedServers?: Record<string, string>;
	routingMode: 'global' | 'rules';
}

export interface StoragePort {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const key = 'lumen_catalog_v1';

export class LatencyProbeTracker {
	private readonly active = new Map<string, number>();
	readonly state = writable<ReadonlySet<string>>(new Set());

	async track<T>(id: string, operation: () => Promise<T>): Promise<T> {
		this.active.set(id, (this.active.get(id) ?? 0) + 1);
		this.publish();
		try {
			return await operation();
		} finally {
			const remaining = (this.active.get(id) ?? 1) - 1;
			if (remaining > 0) this.active.set(id, remaining);
			else this.active.delete(id);
			this.publish();
		}
	}

	private publish() {
		this.state.set(new Set(this.active.keys()));
	}
}

export function initialCatalog(): Catalog {
	return {
		version: 1,
		subscriptions: [
			{
				id: 'demo',
				name: 'BlessedVPN',
				updatedAt: 0,
			},
		],
		servers: ['fi', 'nl', 'de', 'se', 'us', 'gb', 'jp'].map(
			(countryCode, i) => ({
				id: `demo-${countryCode}`,
				subscriptionId: 'demo',
				countryCode,
				nameKey: countryCode,
				protocol: 'VLESS',
				latency: 32 + i * 12,
			}),
		),
		rules: [],
		selectedServerId: 'demo-fi',
		selectedSubscriptionId: 'demo',
		routingMode: 'global',
	};
}

export function validCatalog(value: unknown): value is Catalog {
	if (!value || typeof value !== 'object') {
		return false;
	}

	const c = value as Catalog;

	if (
		c.version !== 1 ||
		!Array.isArray(c.servers) ||
		!Array.isArray(c.subscriptions) ||
		!Array.isArray(c.rules)
	) {
		return false;
	}

	if (!['global', 'rules'].includes(c.routingMode)) {
		return false;
	}

	const subscriptionsValid = c.subscriptions.every(
		(s) =>
			s &&
			typeof s.id === 'string' &&
			typeof s.name === 'string' &&
			Number.isFinite(s.updatedAt),
	);
	if (!subscriptionsValid) {
		return false;
	}

	const subscriptionIds = new Set(c.subscriptions.map((s) => s.id));
	const serversValid = c.servers.every(
		(s) =>
			s &&
			typeof s.id === 'string' &&
			subscriptionIds.has(s.subscriptionId) &&
			typeof s.countryCode === 'string' &&
			typeof s.protocol === 'string' &&
			(s.latency === 'timeout' || Number.isFinite(s.latency)),
	);
	if (!serversValid) {
		return false;
	}

	const serverIds = new Set(c.servers.map((s) => s.id));
	if (
		serverIds.size !== c.servers.length ||
		subscriptionIds.size !== c.subscriptions.length
	) {
		return false;
	}

	if (c.selectedServerId !== null && !serverIds.has(c.selectedServerId)) {
		return false;
	}

	return c.rules.every(
		(r) =>
			r &&
			typeof r.id === 'string' &&
			typeof r.name === 'string' &&
			typeof r.enabled === 'boolean' &&
			(typeof r.isDomain === 'boolean' ||
				(typeof r.matcher === 'string' &&
					['domain', 'application', 'geosite', 'geoip'].includes(r.matcher))) &&
			(r.serverId === 'direct' ||
				r.serverId === 'block' ||
				r.serverId === 'subscription' ||
				subscriptionIds.has(r.serverId) ||
				serverIds.has(r.serverId)),
	);
}

export function validateSubscription(input: string): void {
	const sanitized = input
		.trim()
		.replace(/^\uFEFF/, '')
		.replace(/^["'<]|["'>]$/g, '')
		.trim();

	if (!sanitized || sanitized.length > 65536) {
		throw new ServiceError('invalidSubscription');
	}

	const urlCandidate =
		!sanitized.includes('://') &&
		!sanitized.startsWith('{') &&
		!sanitized.startsWith('[') &&
		!sanitized.includes('proxies:') &&
		sanitized.includes('.') &&
		sanitized.includes('/') &&
		!sanitized.includes(' ')
			? `https://${sanitized}`
			: sanitized;

	try {
		const url = new URL(urlCandidate);
		if (
			url.protocol === 'https:' &&
			url.hostname &&
			!url.username &&
			!url.password
		) {
			return;
		}
		if (
			['vless:', 'vmess:', 'trojan:', 'ss:', 'hysteria2:', 'hy2:'].includes(
				url.protocol,
			) &&
			url.host
		) {
			return;
		}
	} catch {
		/* Try an encoded list of links or multiline plain keys. */
	}

	const lines = sanitized
		.split(/\r?\n/)
		.map((l) => l.trim())
		.filter((l) => l.length > 0 && !l.startsWith('#'));

	if (
		lines.length > 0 &&
		lines.every((line) =>
			/^(vless|vmess|trojan|ss|hysteria2|hy2):\/\/\S+$/i.test(line),
		)
	) {
		return;
	}

	if (sanitized.includes('proxies:')) {
		return;
	}

	try {
		const normalized = sanitized.replace(/-/g, '+').replace(/_/g, '/');
		const padded = normalized.padEnd(
			normalized.length + ((4 - (normalized.length % 4)) % 4),
			'=',
		);
		const decoded = atob(padded).trim();
		if (
			decoded &&
			decoded
				.split(/\r?\n/)
				.map((l) => l.trim())
				.filter((l) => l.length > 0 && !l.startsWith('#'))
				.every((line) =>
					/^(vless|vmess|trojan|ss|hysteria2|hy2):\/\/\S+$/i.test(line),
				)
		) {
			return;
		}
	} catch {
		/* Invalid input is reported without retaining credentials. */
	}

	throw new ServiceError('invalidSubscription');
}

export function normalizeGeoTag(
	input: string,
	expectedKind?: 'geosite' | 'geoip',
): string {
	let tag = input.trim();
	const lower = tag.toLowerCase();
	if (expectedKind === 'geosite') {
		if (lower.startsWith('geoip:')) throw new ServiceError('invalidRule');
		if (lower.startsWith('geosite:')) tag = tag.slice('geosite:'.length).trim();
	} else if (expectedKind === 'geoip') {
		if (lower.startsWith('geosite:')) throw new ServiceError('invalidRule');
		if (lower.startsWith('geoip:')) tag = tag.slice('geoip:'.length).trim();
	} else {
		if (lower.startsWith('geosite:')) tag = tag.slice('geosite:'.length).trim();
		else if (lower.startsWith('geoip:')) tag = tag.slice('geoip:'.length).trim();
	}
	if (!/^[a-zA-Z0-9_\-!@]+$/.test(tag) || tag.length === 0 || tag.length > 128) {
		throw new ServiceError('invalidRule');
	}
	return tag;
}

export class CatalogService {
	private current: Catalog;
	private readonly subscriptionSources = new Map<string, string>();
	private readonly probes = new LatencyProbeTracker();
	private indexedServers: Server[] | undefined;
	private serverIndex = new Map<string, Server>();
	readonly state;
	readonly pinging = this.probes.state;
	readonly persistenceError = writable(false);

	constructor(private storage?: StoragePort) {
		this.current = initialCatalog();

		try {
			const saved =
				this.storage?.getItem(key) ?? this.storage?.getItem('misc_catalog_v1');
			if (saved) {
				const parsed: unknown = JSON.parse(saved);
				if (validCatalog(parsed)) {
					this.current = parsed;
				} else {
					this.persistenceError.set(true);
				}
			}
		} catch {
			this.persistenceError.set(true);
		}

		this.state = writable(this.current);
	}

	private commit(next: Catalog) {
		try {
			this.storage?.setItem(key, JSON.stringify(next));
			this.persistenceError.set(false);
		} catch {
			throw new ServiceError('storage');
		}
		this.current = next;
		this.state.set(next);
	}

	private get serversById(): ReadonlyMap<string, Server> {
		// Reuse the index when only rules, selection or subscription metadata change.
		if (this.indexedServers !== this.current.servers) {
			this.serverIndex = new Map(
				this.current.servers.map((server) => [server.id, server]),
			);
			this.indexedServers = this.current.servers;
		}
		return this.serverIndex;
	}

	async getServers() {
		return this.current.servers.map((s) => ({ ...s }));
	}

	async selectServer(id: string) {
		const server = this.serversById.get(id);
		if (!server) {
			throw new ServiceError('invalidServer');
		}
		const selectedServers = {
			...(this.current.selectedServers ?? {}),
			[server.subscriptionId]: id,
		};
		this.commit({
			...this.current,
			selectedServerId: id,
			selectedSubscriptionId: server.subscriptionId,
			selectedServers,
		});
	}

	selectSubscription(id: string) {
		if (!this.current.subscriptions.some((s) => s.id === id)) {
			throw new ServiceError('invalidSubscription');
		}
		const rememberedId = this.current.selectedServers?.[id];
		const rememberedServer = rememberedId
			? this.current.servers.find(
					(s) => s.subscriptionId === id && s.id === rememberedId,
				)
			: undefined;
		const matchingServer =
			rememberedServer ??
			this.current.servers.find((s) => s.subscriptionId === id);
		const selectedServers = matchingServer
			? { ...(this.current.selectedServers ?? {}), [id]: matchingServer.id }
			: this.current.selectedServers;
		this.commit({
			...this.current,
			selectedSubscriptionId: id,
			selectedServerId: matchingServer
				? matchingServer.id
				: this.current.selectedServerId,
			selectedServers,
		});
	}

	async pingServer(id: string) {
		const server = this.serversById.get(id);
		if (!server) {
			throw new ServiceError('invalidServer');
		}
		return this.probes.track(id, async () => {
			await new Promise((resolve) => setTimeout(resolve, 500));
			return server.latency;
		});
	}

	async importSubscription(input: string, name?: string) {
		validateSubscription(input);
		const resolvedName =
			name?.trim() ||
			(() => {
				try {
					return new URL(input.trim()).hostname.replace(/^www\./, '');
				} catch {
					return 'Subscription';
				}
			})();
		if (!resolvedName || resolvedName.length > 80) {
			throw new ServiceError('invalidSubscription');
		}

		const id = crypto.randomUUID();
		// Only demo metadata is retained. Never persist subscription credentials in WebView storage.
		const server: Server = {
			id: `${id}-fi`,
			subscriptionId: id,
			countryCode: 'fi',
			nameKey: 'fi',
			protocol: 'VLESS',
			latency: 52,
		};

		this.commit({
			...this.current,
			subscriptions: [
				...this.current.subscriptions,
				{ id, name: resolvedName, updatedAt: Date.now() },
			],
			servers: [...this.current.servers, server],
			selectedSubscriptionId: id,
			selectedServerId: server.id,
		});
		this.subscriptionSources.set(id, input.trim());
	}

	async refreshSubscription(id: string) {
		if (!this.current.subscriptions.some((s) => s.id === id)) {
			throw new ServiceError('invalidSubscription');
		}
		await new Promise((resolve) => setTimeout(resolve, 250));
		this.commit({
			...this.current,
			subscriptions: this.current.subscriptions.map((s) =>
				s.id === id ? { ...s, updatedAt: Date.now() } : s,
			),
		});
	}

	async getSubscriptionSource(id: string) {
		const source = this.subscriptionSources.get(id);
		if (!source) {
			throw new ServiceError('invalidSubscription');
		}
		return source;
	}

	renameSubscription(id: string, name: string) {
		if (!name.trim() || name.length > 80) {
			throw new ServiceError('invalidSubscription');
		}
		this.commit({
			...this.current,
			subscriptions: this.current.subscriptions.map((s) =>
				s.id === id ? { ...s, name: name.trim() } : s,
			),
		});
	}

	removeSubscription(id: string) {
		const servers = this.current.servers.filter((s) => s.subscriptionId !== id);
		const serverIds = new Set(servers.map((s) => s.id));
		const remainingSubs = this.current.subscriptions.filter((s) => s.id !== id);
		this.commit({
			...this.current,
			subscriptions: remainingSubs,
			servers,
			rules: this.current.rules.filter(
				(r) =>
					r.serverId === 'direct' ||
					serverIds.has(r.serverId) ||
					(r.serverId === 'subscription' && remainingSubs.length > 0) ||
					remainingSubs.some((s) => s.id === r.serverId),
			),
			selectedSubscriptionId:
				this.current.selectedSubscriptionId === id
					? (remainingSubs[0]?.id ?? null)
					: this.current.selectedSubscriptionId,
			selectedServerId:
				this.current.selectedServerId !== null &&
				serverIds.has(this.current.selectedServerId)
					? this.current.selectedServerId
					: (servers[0]?.id ?? null),
		});
		this.subscriptionSources.delete(id);
	}

	setRoutingMode(routingMode: Catalog['routingMode']) {
		this.commit({ ...this.current, routingMode });
	}

	private validateRule(rule: Rule): Rule {
		const rawName = rule.name.trim();
		let matcher: RuleMatcher;
		if (rule.matcher === 'geosite' || rule.matcher === 'geoip') {
			matcher = rule.matcher;
		} else if (rule.isDomain !== undefined) {
			matcher = rule.isDomain ? 'domain' : 'application';
		} else {
			matcher = rule.matcher ?? 'application';
		}
		let valid = false;
		let normalizedName = rawName;

		if (matcher === 'domain') {
			valid =
				/^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,63}$/i.test(rawName) ||
				/^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(?:\/\d{1,2})?$/.test(rawName);
		} else if (matcher === 'application') {
			valid = /^[^<>:"\\|?*\r\n]+$/i.test(rawName) && rawName.length > 0;
		} else if (matcher === 'geosite' || matcher === 'geoip') {
			try {
				normalizedName = normalizeGeoTag(rawName, matcher);
				valid = true;
			} catch {
				valid = false;
			}
		}

		const isSpecial =
			rule.serverId === 'direct' ||
			rule.serverId === 'block' ||
			rule.serverId === 'subscription' ||
			this.current.subscriptions.some((s) => s.id === rule.serverId);

		if (
			!valid ||
			normalizedName.length > 253 ||
			(!isSpecial && !this.serversById.has(rule.serverId))
		) {
			throw new ServiceError('invalidRule');
		}

		return {
			...rule,
			name: normalizedName,
			matcher,
			isDomain: matcher === 'domain',
		};
	}

	async applyPreset(presetId: string) {
		const targetServer = 'subscription';
		let rulesToCreate: Array<{
			matcher: RuleMatcher;
			name: string;
			serverId: string;
		}>;

		if (presetId === 'russia-optimized') {
			rulesToCreate = [
				{ matcher: 'geoip', name: 'private', serverId: 'direct' },
				{ matcher: 'geosite', name: 'antifilter', serverId: targetServer },
				{
					matcher: 'geosite',
					name: 'antifilter-community',
					serverId: targetServer,
				},
				{ matcher: 'geoip', name: 'antifilter', serverId: targetServer },
				{
					matcher: 'geoip',
					name: 'antifilter-community',
					serverId: targetServer,
				},
				{ matcher: 'geosite', name: 'category-ru', serverId: 'direct' },
				{ matcher: 'geoip', name: 'ru', serverId: 'direct' },
			];
		} else if (presetId === 'bypass-blocking') {
			rulesToCreate = [
				{ matcher: 'geosite', name: 'antifilter', serverId: targetServer },
				{
					matcher: 'geosite',
					name: 'antifilter-community',
					serverId: targetServer,
				},
				{ matcher: 'geoip', name: 'antifilter', serverId: targetServer },
				{
					matcher: 'geoip',
					name: 'antifilter-community',
					serverId: targetServer,
				},
			];
		} else if (presetId === 'russia-direct') {
			rulesToCreate = [
				{ matcher: 'geosite', name: 'category-ru', serverId: 'direct' },
				{ matcher: 'geoip', name: 'ru', serverId: 'direct' },
				{ matcher: 'geoip', name: 'private', serverId: 'direct' },
			];
		} else {
			throw new ServiceError('invalidRule');
		}

		const customRules = this.current.rules.filter(
			(r) => r.presetId !== presetId,
		);
		const newPresetRules: Rule[] = rulesToCreate.map((r, i) => ({
			id: `preset-${presetId}-${i + 1}`,
			name: r.name,
			matcher: r.matcher,
			isDomain: r.matcher === 'domain',
			serverId: r.serverId,
			enabled: true,
			presetId,
		}));

		this.commit({
			...this.current,
			rules: [...newPresetRules, ...customRules],
		});
	}

	async removePreset(presetId: string) {
		this.commit({
			...this.current,
			rules: this.current.rules.filter((r) => r.presetId !== presetId),
		});
	}

	saveRule(rule: Rule) {
		this.saveRules([rule]);
	}

	/** Validate the entire batch, then persist and publish it once. */
	saveRules(rules: readonly Rule[]) {
		if (rules.length === 0) return;
		const updates = new Map<string, Rule>();
		for (const rule of rules) {
			updates.set(rule.id, this.validateRule(rule));
		}
		const existingIds = new Set(this.current.rules.map((rule) => rule.id));
		const next = this.current.rules.map((rule) => updates.get(rule.id) ?? rule);
		for (const [id, rule] of updates) {
			if (!existingIds.has(id)) next.push(rule);
		}
		this.commit({ ...this.current, rules: next });
	}

	removeRules(ids: Set<string>) {
		this.commit({
			...this.current,
			rules: this.current.rules.filter((r) => !ids.has(r.id)),
		});
	}
}
