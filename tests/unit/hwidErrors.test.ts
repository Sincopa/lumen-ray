import { describe, it, expect } from 'vitest';
import { errorCode, ServiceError } from '../../src/lib/services/errors';
import ru from '../../src/locales/ru.json';
import en from '../../src/locales/en.json';

describe('HWID error mapping and localization', () => {
	it('maps native HWID error codes correctly', () => {
		expect(errorCode('HWID_NOT_SUPPORTED')).toBe('hwidNotSupported');
		expect(errorCode('HWID_DEVICE_LIMIT')).toBe('hwidDeviceLimit');
		expect(errorCode('HWID_UNAVAILABLE')).toBe('hwidUnavailable');

		expect(errorCode({ code: 'HWID_NOT_SUPPORTED' })).toBe('hwidNotSupported');
		expect(errorCode({ code: 'HWID_DEVICE_LIMIT' })).toBe('hwidDeviceLimit');
		expect(errorCode({ code: 'HWID_UNAVAILABLE' })).toBe('hwidUnavailable');

		expect(errorCode(new ServiceError('hwidNotSupported'))).toBe(
			'hwidNotSupported',
		);
		expect(errorCode(new ServiceError('hwidDeviceLimit'))).toBe(
			'hwidDeviceLimit',
		);
		expect(errorCode(new ServiceError('hwidUnavailable'))).toBe(
			'hwidUnavailable',
		);
	});

	it('provides valid Russian and English messages for HWID errors', () => {
		expect(ru.errors.hwidNotSupported).toContain('идентификатор устройства');
		expect(ru.errors.hwidDeviceLimit).toContain('лимит устройств');
		expect(ru.errors.hwidUnavailable).toContain('идентификатор устройства');

		expect(en.errors.hwidNotSupported).toContain('device identifier');
		expect(en.errors.hwidDeviceLimit).toContain('Device limit reached');
		expect(en.errors.hwidUnavailable).toContain('device identifier');
	});
});
