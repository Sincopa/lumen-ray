#!/usr/bin/env node
// Writes the updater manifest (latest.json) from the signed assets on a
// release, once every platform has uploaded. The Windows and macOS builds run
// at the same time, so they no longer merge it into the draft one by one.
//
//   node src-tauri/build/updater-manifest.mjs v0.2.15           print it
//   node src-tauri/build/updater-manifest.mjs v0.2.15 --apply   upload it to the release
//
// Needs an authenticated `gh`. GH_REPO / GITHUB_REPOSITORY pick the repo in CI.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const tag = args.find((a) => !a.startsWith('--'));
const apply = args.includes('--apply');
if (!tag || !/^v\d+\.\d+\.\d+$/.test(tag)) {
	console.error('usage: node src-tauri/build/updater-manifest.mjs vX.Y.Z [--apply]');
	process.exit(2);
}
const version = tag.slice(1);

const gh = (...a) => execFileSync('gh', a, { encoding: 'utf8' });
const repo =
	process.env.GH_REPO ||
	process.env.GITHUB_REPOSITORY ||
	JSON.parse(gh('repo', 'view', '--json', 'nameWithOwner')).nameWithOwner;

// `gh release view` also sees drafts, which the REST tag lookup does not.
const { assets } = JSON.parse(gh('release', 'view', tag, '--repo', repo, '--json', 'assets'));
const byName = new Map(assets.map((asset) => [asset.name, asset]));

// The updater downloads the tag's public file URL. It works once the release
// is published, which is also when releases/latest starts serving this
// manifest. The API asset URL it used before counts against GitHub's 60
// unauthenticated API requests an hour per address, which users behind a
// shared VPN exit run out of. The signature is the .sig file's text.
// Releases up to v0.2.18 had the version in the file name.
function entry(name, legacy) {
	const file = byName.has(name) ? name : legacy;
	if (!byName.has(file)) throw new Error(`${tag} has no ${name}`);
	if (!byName.has(`${file}.sig`)) throw new Error(`${tag} has no ${file}.sig`);
	const signature = gh('release', 'download', tag, '--repo', repo, '--pattern', `${file}.sig`, '--output', '-');
	const url = `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(file)}`;
	return { signature: signature.trim(), url };
}

const windows = entry('Lumen_x64-setup.exe', `Lumen_${version}_x64-setup.exe`);
const mac = entry('Lumen_universal.app.tar.gz', `Lumen_${version}_universal.app.tar.gz`);
const manifest = {
	version,
	notes: '',
	pub_date: new Date().toISOString(),
	// Same keys tauri-action wrote: the NSIS installer serves both Windows
	// entries, and the universal app every macOS architecture.
	platforms: {
		'windows-x86_64': windows,
		'windows-x86_64-nsis': windows,
		'darwin-aarch64': mac,
		'darwin-x86_64': mac,
		'darwin-universal': mac,
		'darwin-aarch64-app': mac,
		'darwin-x86_64-app': mac,
		'darwin-universal-app': mac,
	},
};
const json = `${JSON.stringify(manifest, null, 2)}\n`;

if (!apply) {
	process.stdout.write(json);
} else {
	const file = join(mkdtempSync(join(tmpdir(), 'lumen-manifest-')), 'latest.json');
	writeFileSync(file, json);
	gh('release', 'upload', tag, file, '--repo', repo, '--clobber');
	console.log(`latest.json for ${tag}: ${Object.keys(manifest.platforms).length} platforms`);
}
