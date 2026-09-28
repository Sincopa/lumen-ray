import { describe, expect, it } from 'vitest';
import { importPreview } from '../../src/lib/services/importPreview';
describe('import preview', () => {
	it('unwraps a deep link without exposing its subscription token', () => {
		const result = importPreview(
			'lumen://import?url=' +
				encodeURIComponent('https://example.org/private-token?key=secret'),
		);
		expect(result.description).toBe('https://example.org');
		expect(result.input).toContain('private-token');
	});
	it('rejects external actions, nested imports and credential URLs', () => {
		for (const input of [
			'javascript:alert(1)',
			'file:///secret',
			'lumen://connect?url=https://example.org',
			'lumen://import?url=lumen://import',
			'https://user:password@example.org',
		])
			expect(() => importPreview(input)).toThrow();
	});
	it('does not display server credentials or names', () => {
		expect(
			importPreview('vless://secret@example.org:443#private-name').description,
		).toBe('VLESS');
	});
});
