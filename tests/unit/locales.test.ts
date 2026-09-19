import { it, expect } from 'vitest';
import ru from '../../src/locales/ru.json';
import en from '../../src/locales/en.json';
import { i18n } from '../../src/lib/i18n/i18n.svelte';
function keys(value: unknown, prefix = ''): string[] {
	return value && typeof value === 'object'
		? Object.entries(value).flatMap(([key, val]) =>
				keys(val, prefix + '.' + key),
			)
		: [prefix];
}
it('provides matching Russian and English keys', () => {
	expect(keys(ru).sort()).toEqual(keys(en).sort());
});

it('resolves every locale string and preserves missing keys and interpolation', () => {
	for (const [language, dictionary] of [
		['ru', ru],
		['en', en],
	] as const) {
		i18n.setLanguage(language);
		function check(value: object, prefix = '') {
			for (const [key, entry] of Object.entries(value)) {
				const path = prefix ? `${prefix}.${key}` : key;
				if (typeof entry === 'string') {
					expect(i18n.t(path)).toBe(entry);
					const params = { count: 3, name: '$& example' };
					let expected = entry;
					for (const [name, replacement] of Object.entries(params)) {
						expected = expected.split(`{${name}}`).join(String(replacement));
					}
					expect(i18n.t(path, params)).toBe(expected);
				} else if (entry && typeof entry === 'object') check(entry, path);
			}
		}
		check(dictionary);
		expect(i18n.t('missing.key')).toBe('missing.key');
		expect(i18n.t('common')).toBe('common');
	}
	i18n.setLanguage('ru');
});
