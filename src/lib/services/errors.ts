export type ErrorCode =
	| 'unavailable'
	| 'timeout'
	| 'invalidServer'
	| 'invalidSubscription'
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
		'unavailable',
		'timeout',
		'invalidServer',
		'invalidSubscription',
		'invalidRule',
		'storage',
	];
	const code =
		error instanceof ServiceError
			? error.code
			: typeof error === 'object' && error !== null && 'code' in error
				? error.code
				: error;
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
