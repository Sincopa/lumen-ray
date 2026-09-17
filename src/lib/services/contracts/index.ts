import type { VpnConnectionState, VpnConnectionInfo } from '../../types/models';
import type { CatalogService } from '../catalog';
export interface VpnService {
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
