export const ERROR_CODES = [
	'routingConflict',
	'tunAdapterTimeout',
	'ruleUpdateFailed',
	'unavailable',
	'timeout',
	'invalidServer',
	'invalidSubscription',
	'subscriptionFetchFailed',
	'invalidRule',
	'storage',
	'hwidNotSupported',
	'hwidDeviceLimit',
	'hwidUnavailable',
	'geoDataMissing',
	'invalidGeoTag',
	'invalidPreset',
	'unsupportedProtocol',
	'coreNotFound',
	'coreStartFailed',
	'coreCrashed',
	'coreStopFailed',
	'coreRollbackFailed',
	'tunFailed',
	'tunNetworkUnreachable',
	'tunTrafficBlocked',
	'tunDnsFailed',
	'permissionDenied',
	'serverUnreachable',
	'notConnected',
	'cancelled',
	'networkChanged',
	'networkError',
	'busy',
	'liveSwitchFailed',
	'ipUnavailable',
	'invalidConfig',
	'dnsUnavailable',
	'metricsUnavailable',
	'alreadyRunning',
	'updateFailed',
	'updateUnavailable',
	'processFailed',
	'logConsoleFailed',
	'notificationFailed',
	'platformUnsupported',
	'autostartUnavailable',
	'unknown',
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

export class ServiceError extends Error {
	constructor(public code: ErrorCode) {
		super(code);
	}
}

// Every native code gets its own message: a shared "operation unavailable"
// text hides the reason a connection failed.
const NATIVE_CODES: Record<string, ErrorCode> = {
	ROUTING_CONFLICT: 'routingConflict',
	INVALID_SERVER: 'invalidServer',
	INVALID_RULE: 'invalidRule',
	SUBSCRIPTION_INVALID: 'invalidSubscription',
	SUBSCRIPTION_FETCH_FAILED: 'subscriptionFetchFailed',
	SUBSCRIPTION_ACCESS_DENIED: 'invalidSubscription',
	UNSUPPORTED_PROTOCOL: 'unsupportedProtocol',
	STORAGE_FAILED: 'storage',
	STORAGE_VERSION: 'storage',
	HWID_NOT_SUPPORTED: 'hwidNotSupported',
	HWID_DEVICE_LIMIT: 'hwidDeviceLimit',
	HWID_UNAVAILABLE: 'hwidUnavailable',
	GEO_DATA_MISSING: 'geoDataMissing',
	GEO_DATA_CORRUPTED: 'geoDataMissing',
	GEO_UPDATE_FAILED: 'geoDataMissing',
	INVALID_GEO_TAG: 'invalidGeoTag',
	INVALID_PRESET: 'invalidPreset',
	CORE_NOT_FOUND: 'coreNotFound',
	CORE_START_FAILED: 'coreStartFailed',
	CORE_CRASHED: 'coreCrashed',
	CORE_STOP_FAILED: 'coreStopFailed',
	CORE_ROLLBACK_FAILED: 'coreRollbackFailed',
	TUN_FAILED: 'tunFailed',
	TUN_ADAPTER_TIMEOUT: 'tunAdapterTimeout',
	TUN_NETWORK_UNREACHABLE: 'tunNetworkUnreachable',
	TUN_TRAFFIC_BLOCKED: 'tunTrafficBlocked',
	TUN_DNS_FAILED: 'tunDnsFailed',
	RULE_UPDATE_FAILED: 'ruleUpdateFailed',
	PERMISSION_DENIED: 'permissionDenied',
	SERVER_UNREACHABLE: 'serverUnreachable',
	NOT_CONNECTED: 'notConnected',
	CANCELLED: 'cancelled',
	NETWORK_CHANGED: 'networkChanged',
	NETWORK_ERROR: 'networkError',
	BUSY: 'busy',
	LIVE_SWITCH_FAILED: 'liveSwitchFailed',
	IP_UNAVAILABLE: 'ipUnavailable',
	INVALID_CONFIG: 'invalidConfig',
	DNS_UNAVAILABLE: 'dnsUnavailable',
	METRICS_UNAVAILABLE: 'metricsUnavailable',
	ALREADY_RUNNING: 'alreadyRunning',
	UPDATE_FAILED: 'updateFailed',
	UPDATE_UNAVAILABLE: 'updateUnavailable',
	PROCESS_FAILED: 'processFailed',
	LOG_CONSOLE_FAILED: 'logConsoleFailed',
	NOTIFICATION_FAILED: 'notificationFailed',
	PLATFORM_UNSUPPORTED: 'platformUnsupported',
	AUTOSTART_UNAVAILABLE: 'autostartUnavailable',
};

export function errorCode(error: unknown): ErrorCode {
	let code =
		error instanceof ServiceError
			? error.code
			: typeof error === 'object' && error !== null && 'code' in error
				? error.code
				: error;
	if (typeof code === 'string') code = NATIVE_CODES[code] ?? code;
	return ERROR_CODES.includes(code as ErrorCode)
		? (code as ErrorCode)
		: 'unknown';
}

export async function withTimeout<T>(
	operation: Promise<T>,
	ms = 10000,
): Promise<T> {
	let timer: ReturnType<typeof setTimeout>;
	try {
		return await Promise.race([
			operation,
			new Promise<never>((_, reject) => {
				timer = setTimeout(() => reject(new ServiceError('timeout')), ms);
			}),
		]);
	} finally {
		clearTimeout(timer!);
	}
}
