import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Catalog, Rule } from '../../services/catalog';

export class TauriCatalogService {
	private current: Catalog = {
		version: 1,
		servers: [],
		subscriptions: [],
		rules: [],
		selectedServerId: null,
		selectedSubscriptionId: null,
		routingMode: 'global',
	};
	readonly state = writable(this.current);
	readonly persistenceError = writable(false);
	private readonly ready: Promise<void>;
	constructor() {
		this.ready = invoke<Catalog>('get_catalog').then((value) =>
			this.publish(value),
		);
		void this.ready.catch(() => this.persistenceError.set(true));
	}
	private publish(value: Catalog) {
		const existingLatencies = new Map(
			this.current.servers.map((s) => [s.id, s.latency]),
		);
		const mergedServers = value.servers.map((s) => ({
			...s,
			latency:
				typeof s.latency === 'number'
					? s.latency
					: (existingLatencies.get(s.id) ?? s.latency ?? 'timeout'),
		}));
		this.current = { ...value, servers: mergedServers };
		this.state.set(this.current);
		this.persistenceError.set(false);
	}
	private async mutate(mutation: object) {
		await this.ready;
		try {
			this.publish(await invoke<Catalog>('mutate_catalog', { mutation }));
		} catch (error) {
			// Persistence may have succeeded even when applying new network rules failed.
			await invoke<Catalog>('get_catalog')
				.then((value) => this.publish(value))
				.catch(() => this.persistenceError.set(true));
			throw error;
		}
	}
	async getServers() {
		await this.ready;
		return this.current.servers;
	}
	async selectServer(id: string) {
		await this.mutate({ kind: 'selectServer', id });
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
		await this.mutate({ kind: 'setRoutingMode', mode });
	}
	async saveRule(rule: Rule) {
		await this.saveRules([rule]);
	}
	async saveRules(rules: readonly Rule[]) {
		await this.mutate({ kind: 'saveRules', rules });
	}
	async removeRules(ids: Set<string>) {
		await this.mutate({ kind: 'removeRules', ids: [...ids] });
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
		await this.ready;
		const latency = await invoke<number | 'timeout'>('ping_server', { id });
		this.publish({
			...this.current,
			servers: this.current.servers.map((server) =>
				server.id === id ? { ...server, latency } : server,
			),
		});
		return latency;
	}
}
