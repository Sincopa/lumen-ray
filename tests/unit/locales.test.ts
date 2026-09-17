import { it, expect } from 'vitest';
import ru from '../../src/locales/ru.json';
import en from '../../src/locales/en.json';
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
