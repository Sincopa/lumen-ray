import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
	LatencyProbeTracker,
	type Catalog,
	type Rule,
} from '../../services/catalog';

type Optimistic = (catalog: Catalog) => Catalog;

export class TauriCatalogService {
	private readonly probes = new LatencyProbeTracker();
	/** Last catalog confirmed by the backend. */
	private confirmed: Catalog = {
		version: 1,
		servers: [],
		subscriptions: [],
		rules: [],
		selectedServerId: null,
		selectedSubscriptionId: null,
		routingMode: 'global',
	};
	/** Changes sent to the backend and not answered yet, in request order. */
	private pending: { apply: Optimistic }[] = [];
	private current: Catalog = this.confirmed;
	readonly state = writable(this.current);
	readonly pinging = this.probes.state;
	readonly persistenceError = writable(false);
	private readonly ready: Promise<void>;
	constructor() {
		this.ready = invoke<Catalog>('get_catalog').then((value) =>
			this.publish(value),
		);
		void this.ready.catch(() => this.persistenceError.set(true));
		void listen<Catalog>('catalog://changed', (event) =>
			this.publish(event.payload),
		);
	}
	private publish(value: Catalog) {
		const existingLatencies = new Map(
			this.confirmed.servers.map((s) => [s.id, s.latency]),
		);
		const mergedServers = value.servers.map((s) => ({
			...s,
			latency:
				typeof s.latency === 'number'
					? s.latency
					: (existingLatencies.get(s.id) ?? s.latency ?? 'timeout'),
		}));
		this.confirmed = { ...value, servers: mergedServers };
		this.render();
		this.persistenceError.set(false);
	}
	/** Show the confirmed catalog with every unanswered change applied on top. */
	private render() {
		this.current = this.pending.reduce(
			(catalog, change) => change.apply(catalog),
			this.confirmed,
		);
		this.state.set(this.current);
	}
	private async mutate(mutation: object, optimistic?: Optimistic) {
		await this.ready;
		// Switches and the mode button respond at once; the backend answer (or the
		// persisted state after a failure) replaces the guess when it arrives.
		const change = optimistic ? { apply: optimistic } : null;
		if (change) {
			this.pending.push(change);
			this.render();
		}
		const settle = () => {
			if (change) this.pending = this.pending.filter((item) => item !== change);
		};
		try {
			const value = await invoke<Catalog>('mutate_catalog', { mutation });
			settle();
			this.publish(value);
		} catch (error) {
			settle();
			// Persistence may have succeeded even when applying new network rules failed.
			await invoke<Catalog>('get_catalog')
				.then((value) => this.publish(value))
				.catch(() => {
					this.render();
					this.persistenceError.set(true);
				});
			throw error;
		}
	}
	async getServers() {
		await this.ready;
		return this.current.servers;
	}
	async selectServer(id: string) {
		await this.mutate({ kind: 'selectServer', id }, (catalog) => {
			const server = catalog.servers.find((item) => item.id === id);
			return server
				? {
						...catalog,
						selectedServerId: id,
						selectedSubscriptionId: server.subscriptionId,
					}
				: catalog;
		});
	}
	async selectSubscription(id: string) {
		await this.mutate({ kind: 'selectSubscription', id });
	}
	async renameSubscription(id: string, name: string) {
		await this.mutate({ kind: 'renameSubscription', id, name });
	}
	async removeSubscription(id: string) {
		await this.mutate({ kind: 'removeSubscription', id });
	}
	async setRoutingMode(mode: Catalog['routingMode']) {
		await this.mutate({ kind: 'setRoutingMode', mode }, (catalog) => ({
			...catalog,
			routingMode: mode,
		}));
	}
	async saveRule(rule: Rule) {
		await this.saveRules([rule]);
	}
	async saveRules(rules: readonly Rule[]) {
		await this.mutate({ kind: 'saveRules', rules }, (catalog) => {
			const updated = new Map(rules.map((rule) => [rule.id, rule]));
			const existing = catalog.rules.map(
				(rule) => updated.get(rule.id) ?? rule,
			);
			const known = new Set(catalog.rules.map((rule) => rule.id));
			return {
				...catalog,
				rules: [...existing, ...rules.filter((rule) => !known.has(rule.id))],
			};
		});
	}
	async removeRules(ids: Set<string>) {
		const removed = new Set(ids);
		await this.mutate(
			{ kind: 'removeRules', ids: [...removed] },
			(catalog) => ({
				...catalog,
				rules: catalog.rules.filter((rule) => !removed.has(rule.id)),
			}),
		);
	}
	async applyPreset(presetId: string) {
		await this.mutate({ kind: 'applyPreset', presetId });
	}
	async removePreset(presetId: string) {
		await this.mutate({ kind: 'removePreset', presetId }, (catalog) => ({
			...catalog,
			rules: catalog.rules.filter((rule) => rule.presetId !== presetId),
		}));
	}
	async importSubscription(input: string, name?: string) {
		await this.ready;
		const result = await invoke<{ catalog: Catalog }>('import_subscription', {
			input,
			name,
		});
		this.publish(result.catalog);
	}
	async refreshSubscription(id: string) {
		await this.ready;
		const result = await invoke<{ catalog: Catalog }>('refresh_subscription', {
			id,
		});
		this.publish(result.catalog);
	}
	async getSubscriptionSource(id: string) {
		await this.ready;
		return invoke<string>('get_subscription_source', { id });
	}
	async pingServer(id: string): Promise<number | 'timeout'> {
		return this.probes.track(id, async () => {
			await this.ready;
			const latency = await invoke<number | 'timeout'>('ping_server', { id });
			this.publish({
				...this.confirmed,
				servers: this.confirmed.servers.map((server) =>
					server.id === id ? { ...server, latency } : server,
				),
			});
			return latency;
		});
	}
}
