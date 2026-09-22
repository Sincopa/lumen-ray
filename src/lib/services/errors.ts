export type ErrorCode =
	| 'routingConflict'
	| 'unavailable'
	| 'timeout'
	| 'invalidServer'
	| 'invalidSubscription'
	| 'subscriptionFetchFailed'
	| 'invalidRule'
	| 'storage'
	| 'unknown';

export class ServiceError extends Error {
	constructor(public code: ErrorCode) {
		super(code);
	}
}

export function errorCode(error: unknown): ErrorCode {
	const codes: ErrorCode[] = [
		'routingConflict',
		'unavailable',
		'timeout',
		'invalidServer',
		'invalidSubscription',
		'subscriptionFetchFailed',
		'invalidRule',
		'storage',
	];
	let code =
		error instanceof ServiceError
			? error.code
			: typeof error === 'object' && error !== null && 'code' in error
				? error.code
				: error;
	const nativeCodes: Record<string, ErrorCode> = {
		ROUTING_CONFLICT: 'routingConflict',
		INVALID_SERVER: 'invalidServer',
		INVALID_RULE: 'invalidRule',
		SUBSCRIPTION_INVALID: 'invalidSubscription',
		SUBSCRIPTION_FETCH_FAILED: 'subscriptionFetchFailed',
		STORAGE_FAILED: 'storage',
		STORAGE_VERSION: 'storage',
		CORE_NOT_FOUND: 'unavailable',
		CORE_START_FAILED: 'unavailable',
		CORE_CRASHED: 'unavailable',
		TUN_FAILED: 'unavailable',
		PERMISSION_DENIED: 'unavailable',
		SERVER_UNREACHABLE: 'unavailable',
		BUSY: 'unavailable',
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
