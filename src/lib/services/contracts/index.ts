import type { VpnConnectionState, VpnConnectionInfo } from '../../types/models';
export interface VpnService {
	subscribe?(
		listener: (snapshot: {
			status: VpnConnectionState;
			info: VpnConnectionInfo | null;
			error?: unknown;
		}) => void,
	): Promise<() => void>;
	getSnapshot?(): Promise<{
		status: VpnConnectionState;
		info: VpnConnectionInfo | null;
		error?: unknown;
	}>;
	connect(serverId: string): Promise<void>;
	connectExact?(serverId: string): Promise<void>;
	disconnect(): Promise<void>;
	reconnect(): Promise<void>;
	getStatus(): Promise<VpnConnectionState>;
	getConnectionInfo(): Promise<VpnConnectionInfo | null>;
}
