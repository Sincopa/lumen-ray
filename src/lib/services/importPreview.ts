export function importPreview(raw: string): {
	input: string;
	description: string;
} {
	let input = raw.trim();
	if (!input || input.length > 65536) throw new Error('INVALID_IMPORT');
	if (input.startsWith('lumen://')) {
		const link = new URL(input);
		if (
			link.hostname !== 'import' ||
			(link.pathname && link.pathname !== '/') ||
			link.username ||
			link.password
		)
			throw new Error('INVALID_IMPORT');
		input = link.searchParams.get('url') ?? '';
	}
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
