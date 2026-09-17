import type { VpnService } from '../../services/contracts';
import type { VpnConnectionState, VpnConnectionInfo } from '../../types/models';
import { invoke } from '@tauri-apps/api/core';

export class TauriVpnService implements VpnService {
  async connect(serverId: string): Promise<void> {
    await invoke('connect', { serverId });
  }

  async disconnect(): Promise<void> {
    await invoke('disconnect');
  }

  async reconnect(): Promise<void> {
    await invoke('reconnect');
  }

  async getStatus(): Promise<VpnConnectionState> {
    return await invoke<VpnConnectionState>('get_status');
  }

  async getConnectionInfo(): Promise<VpnConnectionInfo | null> {
    return await invoke<VpnConnectionInfo | null>('get_connection_info');
  }
}
