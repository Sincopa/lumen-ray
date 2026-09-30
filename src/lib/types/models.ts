export type VpnConnectionState =
	| 'initializing'
	| 'disconnecting'
	| 'disconnected'
	| 'connecting'
	| 'connected'
	| 'reconnecting'
	| 'error';

export interface VpnConnectionInfo {
	serverId: string;
	demo: boolean;
	ip: string;
	protocol: VpnProtocol;
	connectedAt: number; // timestamp
}

export type VpnProtocol =
	| 'Vless'
	| 'VMess'
	| 'Trojan'
	| 'Shadowsocks'
	| 'Reality'
	| 'Hysteria'
	| 'WireGuard'
	| 'SOCKS5'
	| 'HTTP';

export interface VpnServer {
	id: string;
	name: string;
	country: string;
	protocol: VpnProtocol;
	latencyMs?: number;
}

export interface TrafficStats {
	available: boolean;
	uploadSpeed: number;
	downloadSpeed: number;
	durationSeconds: number;
	upload: number; // bytes
	download: number; // bytes
}

export interface AppError {
	code: string;
	message: string;
}
