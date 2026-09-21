import type {
	VpnConnectionState,
	VpnConnectionInfo,
	TrafficStats,
} from '../../types/models';
import type { CatalogService } from '../catalog';
export interface VpnService {
	getStats?(): Promise<TrafficStats>;
	subscribeStats?(listener: (stats: TrafficStats) => void): Promise<() => void>;
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
	disconnect(): Promise<void>;
	reconnect(): Promise<void>;
	getStatus(): Promise<VpnConnectionState>;
	getConnectionInfo(): Promise<VpnConnectionInfo | null>;
}
export type ServerService = Pick<
	CatalogService,
	'getServers' | 'selectServer' | 'pingServer'
>;
export type SubscriptionService = Pick<
	CatalogService,
	| 'importSubscription'
	| 'refreshSubscription'
	| 'renameSubscription'
	| 'removeSubscription'
>;
