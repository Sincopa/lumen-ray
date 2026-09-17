import { runtime } from '../runtime';
import { BrowserVpnService } from '../platform/browser/BrowserVpnService';
import { TauriVpnService } from '../platform/tauri/TauriVpnService';
import { CatalogService, type StoragePort } from './catalog';
import { ConnectionController } from '../stores/connection';
let storage: StoragePort | undefined;
try {
  storage = typeof localStorage !== 'undefined' ? localStorage : undefined;
} catch {
  /* Session-only mode. */
}
export const catalog = new CatalogService(storage);
const vpn = runtime.isTauri ? new TauriVpnService() : new BrowserVpnService();
export const connection = new ConnectionController(vpn);
export const services = {
  getVpn: () => vpn,
  getServers: () => catalog,
  getSubscriptions: () => catalog,
};
