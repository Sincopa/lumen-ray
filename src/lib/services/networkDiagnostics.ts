export interface NetworkDiagnostics {
	coreEngine?: 'xray' | 'lumen';
	selectedCoreEngine?: 'xray' | 'lumen';
	status: string;
	networkMode: string;
	coreVersion: string;
	publicIpv4: string | null;
	publicIpv6: string | null;
	systemDns: string[];
	configuredDns: string;
	tunnelReady: boolean;
	routesReady: boolean;
	routingConflict: boolean;
	systemProxyEnabled: boolean;
	proxyManaged: boolean;
}

export function formatNetworkDiagnostics(
	diagnostics: NetworkDiagnostics,
): string {
	return [
		`Lumen / Xray ${diagnostics.coreVersion}`,
		`Active engine: ${diagnostics.coreEngine ?? 'xray'}`,
		`Selected engine: ${diagnostics.selectedCoreEngine ?? 'xray'}`,
		`Status: ${diagnostics.status}`,
		`Mode: ${diagnostics.networkMode}`,
		`IPv4: ${diagnostics.publicIpv4 ?? 'unavailable'}`,
		`IPv6: ${diagnostics.publicIpv6 ?? 'unavailable'}`,
		`System DNS: ${diagnostics.systemDns.join(', ') || 'unavailable'}`,
		`Selected DNS: ${diagnostics.configuredDns}`,
		`TUN adapter: ${diagnostics.tunnelReady}`,
		`TUN routes: ${diagnostics.routesReady}`,
		`Routing conflict: ${diagnostics.routingConflict}`,
		`System proxy enabled: ${diagnostics.systemProxyEnabled}`,
		`System proxy managed: ${diagnostics.proxyManaged}`,
	].join('\n');
}
