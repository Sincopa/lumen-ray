import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import ts from 'typescript';

// Compile into memory so the benchmark needs no extra runtime or emitted files.
function moduleUrl(source) {
	const output = ts.transpileModule(source, {
		compilerOptions: {
			target: ts.ScriptTarget.ES2022,
			module: ts.ModuleKind.ESNext,
		},
	}).outputText;
	return `data:text/javascript;base64,${Buffer.from(output).toString('base64')}`;
}

const errors = moduleUrl(
	await readFile(
		new URL('../src/lib/services/errors.ts', import.meta.url),
		'utf8',
	),
);
const source = (
	await readFile(
		new URL('../src/lib/services/catalog.ts', import.meta.url),
		'utf8',
	)
)
	.replace(
		"'svelte/store'",
		JSON.stringify(import.meta.resolve('svelte/store')),
	)
	.replace("'./errors'", JSON.stringify(errors));
const { CatalogService, initialCatalog } = await import(moduleUrl(source));

const catalog = initialCatalog();
catalog.servers = Array.from({ length: 2000 }, (_, index) => ({
	...catalog.servers[0],
	id: `server-${index}`,
}));
catalog.selectedServerId = 'server-0';
catalog.rules = Array.from({ length: 1000 }, (_, index) => ({
	id: `existing-${index}`,
	name: `existing-${index}.com`,
	isDomain: true,
	serverId: `server-${index}`,
	enabled: true,
}));
const input = JSON.stringify(catalog);
const additions = Array.from({ length: 500 }, (_, index) => ({
	id: `new-${index}`,
	name: `new-${index}.com`,
	isDomain: true,
	serverId: `server-${index}`,
	enabled: true,
}));

function run(batch) {
	let writes = 0;
	let serializedCharacters = 0;
	let saved = '';
	const service = new CatalogService({
		getItem: () => input,
		setItem: (_, value) => {
			writes++;
			serializedCharacters += value.length;
			saved = value;
		},
	});
	const start = performance.now();
	if (batch) service.saveRules(additions);
	else for (const rule of additions) service.saveRule(rule);
	return {
		milliseconds: performance.now() - start,
		writes,
		serializedCharacters,
		saved,
	};
}

run(false);
run(true);
const sequential = [];
const batched = [];
for (let round = 0; round < 5; round++) {
	const first = run(round % 2 === 0);
	const second = run(round % 2 !== 0);
	assert.equal(first.saved, second.saved);
	sequential.push(round % 2 === 0 ? second : first);
	batched.push(round % 2 === 0 ? first : second);
}
function summarize(results) {
	const median = results
		.map((result) => result.milliseconds)
		.sort((a, b) => a - b)[2];
	return {
		medianMs: Number(median.toFixed(3)),
		writes: results[0].writes,
		serializedCharacters: results[0].serializedCharacters,
	};
}
console.log(
	JSON.stringify(
		{
			workload: { servers: 2000, existingRules: 1000, addedRules: 500 },
			storage:
				'in-memory; includes JSON serialization, excludes disk latency and DOM rendering',
			identicalSavedCatalog: true,
			sequential: summarize(sequential),
			batched: summarize(batched),
		},
		null,
		2,
	),
);
