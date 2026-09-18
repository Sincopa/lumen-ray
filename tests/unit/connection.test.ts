import { describe, it, expect, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { BrowserVpnService } from '../../src/lib/platform/browser/BrowserVpnService';
import { ConnectionController } from '../../src/lib/stores/connection';
import { ServiceError } from '../../src/lib/services/errors';

afterEach(() => {
  vi.useRealTimers();
});

describe('connection lifecycle', () => {
  it('connects to selected ID and preserves timestamp independently of views', async () => {
    const vpn = new BrowserVpnService(0);
    const controller = new ConnectionController(vpn);

    await controller.initialize();
    await controller.toggle('demo-nl');

    const snapshot = get(controller.state);
    expect(snapshot.status).toBe('connected');
    expect(snapshot.info?.serverId).toBe('demo-nl');
    expect(snapshot.info?.demo).toBe(true);
    expect(snapshot.info?.ip).toBe('');

    await controller.initialize();
    expect(get(controller.state).info?.connectedAt).toBe(
      snapshot.info?.connectedAt,
    );

    await controller.toggle('demo-nl');
    expect(get(controller.state).status).toBe('disconnected');
  });

  it('releases the lock after failure and allows retry', async () => {
    const vpn = new BrowserVpnService(0);
    const controller = new ConnectionController(vpn);

    await controller.initialize();
    vi.spyOn(vpn, 'connect').mockRejectedValueOnce(
      new ServiceError('unavailable'),
    );

    await controller.toggle('demo-fi');
    expect(get(controller.state).busy).toBe(false);
    expect(get(controller.state).error).toBe('unavailable');

    await controller.toggle('demo-fi');
    expect(get(controller.state).status).toBe('connected');
  });

  it('ignores duplicate connect clicks', async () => {
    const vpn = new BrowserVpnService(0);
    const controller = new ConnectionController(vpn);

    await controller.initialize();
    const spy = vi.spyOn(vpn, 'connect');

    await Promise.all([
      controller.toggle('demo-fi'),
      controller.toggle('demo-nl'),
    ]);

    expect(spy).toHaveBeenCalledTimes(1);
    expect(get(controller.state).info?.serverId).toBe('demo-fi');
  });

  it('reports timeout without claiming disconnection', async () => {
    vi.useFakeTimers();
    const vpn = new BrowserVpnService(0);
    const controller = new ConnectionController(vpn, 20);

    vi.spyOn(vpn, 'getStatus').mockImplementation(() => new Promise(() => {}));

    const pending = controller.initialize();
    await vi.advanceTimersByTimeAsync(21);
    await pending;

    expect(get(controller.state)).toMatchObject({
      status: 'error',
      busy: false,
      error: 'timeout',
    });
  });

  it('reconnect remembers server after disconnect clears it', async () => {
    const vpn = new BrowserVpnService(0);

    await vpn.connect('demo-de');
    await vpn.reconnect();

    expect((await vpn.getConnectionInfo())?.serverId).toBe('demo-de');
  });

  it('disconnect invalidates a pending browser connection', async () => {
    const vpn = new BrowserVpnService(5);

    const pending = vpn.connect('demo-fi');
    await vpn.disconnect();
    await pending;

    expect(await vpn.getStatus()).toBe('disconnected');
    expect(await vpn.getConnectionInfo()).toBeNull();
  });
});
