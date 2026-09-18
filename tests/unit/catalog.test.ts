import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import {
  CatalogService,
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
