import { runtime } from '../runtime';
import { BrowserVpnService } from '../platform/browser/BrowserVpnService';
import { TauriVpnService } from '../platform/tauri/TauriVpnService';
import { TauriCatalogService } from '../platform/tauri/TauriCatalogService';
import { CatalogService, type StoragePort } from './catalog';
import { ConnectionController } from './connection';

let storage: StoragePort | undefined;
try {
	storage = typeof localStorage !== 'undefined' ? localStorage : undefined;
} catch {
	/* Session-only mode. */
}

export const catalog = runtime.isTauri
	? new TauriCatalogService()
	: new CatalogService(storage);

const vpn = runtime.isTauri ? new TauriVpnService() : new BrowserVpnService();

export const connection = new ConnectionController(
	vpn,
	runtime.isTauri ? 120000 : 10000,
);

export const services = {
	getVpn: () => vpn,
	getServers: () => catalog,
	getSubscriptions: () => catalog,
};
