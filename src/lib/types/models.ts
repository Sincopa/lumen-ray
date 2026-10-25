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
