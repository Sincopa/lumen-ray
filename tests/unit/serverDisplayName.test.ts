import { describe, expect, it } from 'vitest';
import { serverDisplayName } from '../../src/lib/utils/serverDisplayName';

describe('serverDisplayName', () => {
	it('uses the real server name even when the country has no translation', () => {
		expect(serverDisplayName({ name: 'Warsaw #1', countryCode: 'pl' })).toBe(
			'Warsaw #1',
		);
	});

	it('falls back to a translated country or its code, never a raw key', () => {
		expect(serverDisplayName({ name: ' ', countryCode: 'fi' })).toBe(
			'Финляндия',
		);
		expect(serverDisplayName({ countryCode: 'zz' })).toBe('ZZ');
		expect(serverDisplayName({ countryCode: 'pl' })).toBe('PL');
	});
});
