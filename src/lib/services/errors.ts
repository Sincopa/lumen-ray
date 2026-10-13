export type ErrorCode =
	| 'lumenUnsupported'
	| 'routingConflict'
	| 'tunAdapterTimeout'
	| 'ruleUpdateFailed'
	| 'unavailable'
	| 'timeout'
	| 'invalidServer'
	| 'invalidSubscription'
	| 'subscriptionFetchFailed'
	| 'invalidRule'
	| 'storage'
	| 'hwidNotSupported'
	| 'hwidDeviceLimit'
	| 'hwidUnavailable'
	| 'geoDataMissing'
	| 'invalidGeoTag'
	| 'invalidPreset'
	| 'unknown';

export class ServiceError extends Error {
	constructor(public code: ErrorCode) {
		super(code);
	}
}

export function errorCode(error: unknown): ErrorCode {
	const codes: ErrorCode[] = [
		'lumenUnsupported',
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
	];
	let code =
		error instanceof ServiceError
			? error.code
			: typeof error === 'object' && error !== null && 'code' in error
				? error.code
				: error;
	const nativeCodes: Record<string, ErrorCode> = {
		LUMEN_UNSUPPORTED_CONFIGURATION: 'lumenUnsupported',
		LUMEN_START_FAILED: 'unavailable',
		CORE_ENGINE_UNSUPPORTED: 'unavailable',
		ROUTING_CONFLICT: 'routingConflict',
		INVALID_SERVER: 'invalidServer',
		INVALID_RULE: 'invalidRule',
		SUBSCRIPTION_INVALID: 'invalidSubscription',
		SUBSCRIPTION_FETCH_FAILED: 'subscriptionFetchFailed',
		STORAGE_FAILED: 'storage',
		STORAGE_VERSION: 'storage',
		HWID_NOT_SUPPORTED: 'hwidNotSupported',
		HWID_DEVICE_LIMIT: 'hwidDeviceLimit',
		HWID_UNAVAILABLE: 'hwidUnavailable',
		GEO_DATA_MISSING: 'geoDataMissing',
		GEO_DATA_CORRUPTED: 'geoDataMissing',
		INVALID_GEO_TAG: 'invalidGeoTag',
		INVALID_PRESET: 'invalidPreset',
		CORE_NOT_FOUND: 'unavailable',
		CORE_START_FAILED: 'unavailable',
		CORE_CRASHED: 'unavailable',
		TUN_FAILED: 'unavailable',
		TUN_ADAPTER_TIMEOUT: 'tunAdapterTimeout',
		RULE_UPDATE_FAILED: 'ruleUpdateFailed',
		PERMISSION_DENIED: 'unavailable',
		SERVER_UNREACHABLE: 'unavailable',
		TUN_NETWORK_UNREACHABLE: 'unavailable',
		SUBSCRIPTION_ACCESS_DENIED: 'invalidSubscription',
		NOT_CONNECTED: 'unavailable',
		CANCELLED: 'unavailable',
		NETWORK_CHANGED: 'unavailable',
		NETWORK_ERROR: 'unavailable',
		CORE_ROLLBACK_FAILED: 'unavailable',
		CORE_STOP_FAILED: 'unavailable',
		BUSY: 'unavailable',
		LIVE_SWITCH_FAILED: 'unavailable',
		IP_UNAVAILABLE: 'unavailable',
		INVALID_CONFIG: 'unavailable',
		DNS_UNAVAILABLE: 'unavailable',
		METRICS_UNAVAILABLE: 'unavailable',
		BACKUP_FAILED: 'storage',
		BACKUP_INVALID: 'storage',
		ALREADY_RUNNING: 'unavailable',
		UPDATE_FAILED: 'unavailable',
		UPDATE_UNAVAILABLE: 'unavailable',
		GEO_UPDATE_FAILED: 'geoDataMissing',
	};
	if (typeof code === 'string') code = nativeCodes[code] ?? code;
	return codes.includes(code as ErrorCode) ? (code as ErrorCode) : 'unknown';
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
