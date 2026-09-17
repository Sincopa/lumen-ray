import { writable } from 'svelte/store';
import type { VpnService } from '../services/contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../types/models';
import { errorCode, withTimeout, type ErrorCode } from '../services/errors';

export interface ConnectionSnapshot {
  status: VpnConnectionState;
  info: VpnConnectionInfo | null;
  busy: boolean;
  error: ErrorCode | null;
}
export class ConnectionController {
  private current: ConnectionSnapshot = {
    status: 'initializing',
    info: null,
    busy: true,
    error: null,
  };
  readonly state = writable(this.current);
  private update(patch: Partial<ConnectionSnapshot>) {
    this.current = { ...this.current, ...patch };
    this.state.set(this.current);
  }
  constructor(
    private vpn: VpnService,
    private timeoutMs = 10000,
  ) {}
  async initialize() {
    this.update({ status: 'initializing', busy: true, error: null });
    try {
      const [status, info] = await withTimeout(
        Promise.all([this.vpn.getStatus(), this.vpn.getConnectionInfo()]),
        this.timeoutMs,
      );
      this.update({ status, info });
    } catch (e) {
      this.update({ status: 'error', error: errorCode(e) });
    } finally {
      this.update({ busy: false });
    }
  }
  async toggle(serverId: string | null) {
    if (this.current.busy) return;
    const disconnecting = this.current.status === 'connected';
    if (!disconnecting && !serverId) {
      this.update({ error: 'invalidServer' });
      return;
    }
    this.update({
      busy: true,
      error: null,
      status: disconnecting ? 'disconnecting' : 'connecting',
    });
    try {
      await withTimeout(
        disconnecting ? this.vpn.disconnect() : this.vpn.connect(serverId!),
        this.timeoutMs,
      );
      const [status, info] = await withTimeout(
        Promise.all([this.vpn.getStatus(), this.vpn.getConnectionInfo()]),
        this.timeoutMs,
      );
      this.update({ status, info });
    } catch (e) {
      // An IPC timeout is an unknown outcome: reconcile before enabling another connect.
      try {
        const [status, info] = await withTimeout(
          Promise.all([this.vpn.getStatus(), this.vpn.getConnectionInfo()]),
          this.timeoutMs,
        );
        this.update({ status, info, error: errorCode(e) });
      } catch {
        this.update({ status: 'error', info: null, error: errorCode(e) });
      }
    } finally {
      this.update({ busy: false });
    }
  }
}
