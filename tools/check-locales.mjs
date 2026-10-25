import { readFileSync } from 'node:fs';

const localesDir = new URL('../src/locales/', import.meta.url);
const base = 'en';
const others = ['ru'];

const flatten = (value, prefix = '') =>
	Object.entries(value).flatMap(([key, child]) => {
		const path = prefix ? `${prefix}.${key}` : key;
		return child && typeof child === 'object' ? flatten(child, path) : [[path, String(child)]];
	});

const load = (locale) =>
	new Map(flatten(JSON.parse(readFileSync(new URL(`${locale}.json`, localesDir), 'utf8'))));

const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort().join(',');

const reference = load(base);
const problems = [];

for (const locale of others) {
	const messages = load(locale);

	for (const key of reference.keys()) {
		if (!messages.has(key)) problems.push(`${locale}: missing key "${key}"`);
	}
	for (const key of messages.keys()) {
		if (!reference.has(key)) problems.push(`${locale}: unknown key "${key}"`);
	}
	for (const [key, text] of messages) {
		const source = reference.get(key);
		if (source !== undefined && placeholders(source) !== placeholders(text)) {
			problems.push(`${locale}: placeholders differ for "${key}" (${placeholders(source) || 'none'} vs ${placeholders(text) || 'none'})`);
		}
	}
}

if (problems.length > 0) {
	console.error(problems.join('\n'));
	console.error(`\n${problems.length} locale problem(s) found.`);
	process.exit(1);
}

console.log(`Locales match: ${reference.size} keys in ${[base, ...others].join(', ')}.`);
