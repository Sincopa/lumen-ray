import { t } from '../i18n';
import type { Server } from '../services/catalog';

export function serverDisplayName(
	server: Pick<Server, 'name' | 'countryCode'>,
): string {
	const name = server.name?.trim();
	if (name) return name;
	const code = server.countryCode.trim();
	const key = `servers.countries.${code.toLowerCase()}`;
	const translated = t(key);
	return translated === key ? code.toUpperCase() || '—' : translated;
}
