import type { VpnService } from '../../services/contracts';
import type {
	VpnConnectionState,
	VpnConnectionInfo,
	TrafficStats,
} from '../../types/models';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export class TauriVpnService implements VpnService {
	async getStats(): Promise<TrafficStats> {
		return invoke<TrafficStats>('get_stats');
	}
	async subscribeStats(listener: (stats: TrafficStats) => void) {
		return listen<TrafficStats>('vpn://stats', (event) =>
			listener(event.payload),
		);
	}
	async getSnapshot() {
		return invoke<{
			status: VpnConnectionState;
			info: VpnConnectionInfo | null;
			error?: unknown;
		}>('get_connection_snapshot');
	}
	async subscribe(
		listener: (snapshot: {
			status: VpnConnectionState;
			info: VpnConnectionInfo | null;
			error?: unknown;
		}) => void,
	) {
		return listen<{
			status: VpnConnectionState;
			info: VpnConnectionInfo | null;
			error?: unknown;
		}>('vpn://state', (event) => listener(event.payload));
	}
	async connect(serverId: string): Promise<void> {
		await invoke('connect', { serverId });
	}
	async connectExact(serverId: string): Promise<void> {
		await invoke('connect_exact', { serverId });
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
