import { writable } from 'svelte/store';
import { ServiceError } from './errors';

export interface Server {
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
  daysRemaining?: number;
  traffic?: string;
  updatedMinutesAgo?: number;
}
export interface Rule {
  id: string;
  name: string;
  isDomain: boolean;
  serverId: string;
  enabled: boolean;
}
export interface Catalog {
  version: 1;
  servers: Server[];
  subscriptions: Subscription[];
  rules: Rule[];
  selectedServerId: string | null;
  routingMode: 'global' | 'rules';
}
export interface StoragePort {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const key = 'misc_catalog_v1';
export function initialCatalog(): Catalog {
  return {
    version: 1,
    subscriptions: [
      {
        id: 'demo',
        name: 'BlessedVPN',
        updatedAt: 0,
        status: 'expired',
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
    routingMode: 'global',
  };
}

export function validCatalog(value: unknown): value is Catalog {
  if (!value || typeof value !== 'object') return false;
  const c = value as Catalog;
  if (
    c.version !== 1 ||
    !Array.isArray(c.servers) ||
    !Array.isArray(c.subscriptions) ||
    !Array.isArray(c.rules)
  )
    return false;
  if (!['global', 'rules'].includes(c.routingMode)) return false;
  if (
    !c.subscriptions.every(
      (s) =>
        s &&
        typeof s.id === 'string' &&
        typeof s.name === 'string' &&
        Number.isFinite(s.updatedAt),
    )
  )
    return false;
  if (
    !c.servers.every(
      (s) =>
        s &&
        typeof s.id === 'string' &&
        c.subscriptions.some((sub) => sub.id === s.subscriptionId) &&
        /^[a-z]{2}$/.test(s.countryCode) &&
        s.nameKey === s.countryCode &&
        typeof s.protocol === 'string' &&
        (s.latency === 'timeout' || Number.isFinite(s.latency)),
    )
  )
    return false;
  if (
    new Set(c.servers.map((s) => s.id)).size !== c.servers.length ||
    new Set(c.subscriptions.map((s) => s.id)).size !== c.subscriptions.length
  )
    return false;
  if (
    c.selectedServerId !== null &&
    !c.servers.some((s) => s.id === c.selectedServerId)
  )
    return false;
  return c.rules.every(
    (r) =>
      r &&
      typeof r.id === 'string' &&
      typeof r.name === 'string' &&
      typeof r.enabled === 'boolean' &&
      typeof r.isDomain === 'boolean' &&
      c.servers.some((s) => s.id === r.serverId),
  );
}

export function validateSubscription(input: string): void {
  if (!input.trim() || input.length > 65536)
    throw new ServiceError('invalidSubscription');
  try {
    const url = new URL(input.trim());
    if (
      url.protocol === 'https:' &&
      url.hostname &&
      !url.username &&
      !url.password
    )
      return;
    if (
      ['vless:', 'vmess:', 'trojan:', 'ss:'].includes(url.protocol) &&
      url.host
    )
      return;
  } catch {
    /* Try an encoded list of links. */
  }
  try {
    const decoded = atob(input.trim()).trim();
    if (
      decoded &&
      decoded
        .split(/\r?\n/)
        .every((line) => /^(vless|vmess|trojan|ss):\/\/\S+$/.test(line))
    )
      return;
  } catch {
    /* Invalid input is reported without retaining credentials. */
  }
  throw new ServiceError('invalidSubscription');
}

export class CatalogService {
  private current: Catalog;
  readonly state;
  readonly persistenceError = writable(false);
  constructor(private storage?: StoragePort) {
    this.current = initialCatalog();
    try {
      const saved = storage?.getItem(key);
      if (saved) {
        const parsed: unknown = JSON.parse(saved);
        if (validCatalog(parsed)) this.current = parsed;
        else this.persistenceError.set(true);
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
  async getServers() {
    return this.current.servers.map((s) => ({ ...s }));
  }
  async selectServer(id: string) {
    if (!this.current.servers.some((s) => s.id === id))
      throw new ServiceError('invalidServer');
    this.commit({ ...this.current, selectedServerId: id });
  }
  async pingServer(id: string) {
    const server = this.current.servers.find((s) => s.id === id);
    if (!server) throw new ServiceError('invalidServer');
    await new Promise((resolve) => setTimeout(resolve, 200));
    return server.latency;
  }
  async importSubscription(input: string, name: string) {
    validateSubscription(input);
    if (!name.trim() || name.length > 80)
      throw new ServiceError('invalidSubscription');
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
        { id, name: name.trim(), updatedAt: Date.now() },
      ],
      servers: [...this.current.servers, server],
      selectedServerId: this.current.selectedServerId ?? server.id,
    });
  }
  async refreshSubscription(id: string) {
    if (!this.current.subscriptions.some((s) => s.id === id))
      throw new ServiceError('invalidSubscription');
    await new Promise((resolve) => setTimeout(resolve, 250));
    this.commit({
      ...this.current,
      subscriptions: this.current.subscriptions.map((s) =>
        s.id === id ? { ...s, updatedAt: Date.now() } : s,
      ),
    });
  }
  renameSubscription(id: string, name: string) {
    if (!name.trim() || name.length > 80)
      throw new ServiceError('invalidSubscription');
    this.commit({
      ...this.current,
      subscriptions: this.current.subscriptions.map((s) =>
        s.id === id ? { ...s, name: name.trim() } : s,
      ),
    });
  }
  removeSubscription(id: string) {
    const servers = this.current.servers.filter((s) => s.subscriptionId !== id);
    this.commit({
      ...this.current,
      subscriptions: this.current.subscriptions.filter((s) => s.id !== id),
      servers,
      rules: this.current.rules.filter((r) =>
        servers.some((s) => s.id === r.serverId),
      ),
      selectedServerId: servers.some(
        (s) => s.id === this.current.selectedServerId,
      )
        ? this.current.selectedServerId
        : (servers[0]?.id ?? null),
    });
  }
  setRoutingMode(routingMode: Catalog['routingMode']) {
    this.commit({ ...this.current, routingMode });
  }
  saveRule(rule: Rule) {
    const name = rule.name.trim();
    const valid = rule.isDomain
      ? /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,63}$/i.test(name)
      : /^[^<>:"/\\|?*\r\n]+\.exe$/i.test(name);
    if (
      !valid ||
      name.length > 253 ||
      !this.current.servers.some((s) => s.id === rule.serverId)
    )
      throw new ServiceError('invalidRule');
    const next = { ...rule, name };
    this.commit({
      ...this.current,
      rules: this.current.rules.some((r) => r.id === rule.id)
        ? this.current.rules.map((r) => (r.id === rule.id ? next : r))
        : [...this.current.rules, next],
    });
  }
  removeRules(ids: Set<string>) {
    this.commit({
      ...this.current,
      rules: this.current.rules.filter((r) => !ids.has(r.id)),
    });
  }
}
