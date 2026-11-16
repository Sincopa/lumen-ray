/** Checks a link from a `lumen://` link; the backend already took it out of the wrapper. */
export function importPreview(raw: string): {
	input: string;
	description: string;
} {
	const input = raw.trim();
	if (!input || input.length > 65536) throw new Error('INVALID_IMPORT');
	const url = new URL(input);
	if (
		![
			'https:',
			'http:',
			'vless:',
			'vmess:',
			'trojan:',
			'ss:',
			'hysteria2:',
			'hy2:',
			'socks:',
			'socks5:',
			'http-proxy:',
		].includes(url.protocol)
	)
		throw new Error('INVALID_IMPORT');
	if (
		(url.protocol === 'http:' || url.protocol === 'https:') &&
		(url.username || url.password)
	)
		throw new Error('INVALID_IMPORT');
	// Never render credentials, subscription tokens, paths or fragments in the preview.
	const description = ['https:', 'http:'].includes(url.protocol)
		? `${url.protocol}//${url.hostname}`
		: url.protocol.slice(0, -1).toUpperCase();
	return { input, description };
}
