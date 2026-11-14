#!/usr/bin/env node
// Builds the GitHub release body: a download table with one badge per asset
// that is actually on the release, plus a checksums/changelog footer.
//
//   node src-tauri/build/release-notes.mjs v0.2.14           print the body
//   node src-tauri/build/release-notes.mjs v0.2.14 --apply   write it to the release
//
// Needs an authenticated `gh`. GH_REPO / GITHUB_REPOSITORY pick the repo in CI.
import { execFileSync } from 'node:child_process';
import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const tag = args.find((a) => !a.startsWith('--'));
const apply = args.includes('--apply');
if (!tag || !/^v\d+\.\d+\.\d+$/.test(tag)) {
	console.error('usage: node src-tauri/build/release-notes.mjs vX.Y.Z [--apply]');
	process.exit(2);
}
const version = tag.slice(1);

const gh = (...a) => execFileSync('gh', a, { encoding: 'utf8' });
const repo =
	process.env.GH_REPO ||
	process.env.GITHUB_REPOSITORY ||
	JSON.parse(gh('repo', 'view', '--json', 'nameWithOwner')).nameWithOwner;

const release = JSON.parse(gh('release', 'view', tag, '--repo', repo, '--json', 'assets'));
const sizes = new Map(release.assets.map((a) => [a.name, a.size]));

// Shields.io static badge: "-" and "_" are separators, so they are doubled.
const shield = (s) => encodeURIComponent(s.replace(/-/g, '--').replace(/_/g, '__'));
// simple-icons dropped the Windows mark, so it goes in as an inline SVG.
const windowsLogo =
	'data:image/svg+xml;base64,' +
	Buffer.from(
		'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#fff" d="M3 3h8.5v8.5H3zm9.5 0H21v8.5h-8.5zM3 12.5h8.5V21H3zm9.5 0H21V21h-8.5z"/></svg>'
	).toString('base64');

const badge = ({ label, message, color, logo }) =>
	`https://img.shields.io/badge/${shield(label)}-${shield(message)}-${color}` +
	`?logo=${encodeURIComponent(logo)}&logoColor=white`;

const download = (file) =>
	`https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(file)}`;
const mb = (bytes) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;

const obtainium =
	'https://apps.obtainium.imranr.dev/redirect?r=obtainium://add/' +
	`https://github.com/${repo}`;

const groups = [
	{
		title: 'Windows · macOS',
		rows: [
			{
				os: 'Windows',
				items: [
					{ file: `Lumen-Ray-${version}-Setup.exe`, label: 'Setup', message: 'x64', color: '0078D4', logo: windowsLogo },
					{ file: `Lumen-Ray-${version}-Portable.zip`, label: 'Portable', message: 'x64', color: '5EA9E6', logo: windowsLogo }
				]
			},
			{
				os: 'macOS',
				items: [
					{ file: `Lumen_${version}_universal.dmg`, label: 'DMG', message: 'Apple Silicon + Intel', color: '111111', logo: 'apple' }
				]
			}
		]
	},
	{
		title: 'Android',
		rows: [
			{
				os: 'Android',
				items: [
					{ href: obtainium, needsApk: true, label: 'Get it on', message: 'Obtainium', color: 'EF7100', logo: 'android', title: 'Install and get updates through Obtainium' },
					{ file: `Lumen-${version}-universal.apk`, label: 'APK', message: 'universal', color: '3DDC84', logo: 'android' },
					{ file: `Lumen-${version}-arm64.apk`, label: 'APK', message: 'arm64-v8a', color: '168039', logo: 'android' },
					{ file: `Lumen-${version}-arm.apk`, label: 'APK', message: 'armeabi-v7a', color: '45BF55', logo: 'android' },
					{ file: `Lumen-${version}-x86_64.apk`, label: 'APK', message: 'x86_64', color: '45BF55', logo: 'android' }
				]
			}
		]
	}
];

const hasApk = release.assets.some((a) => a.name.endsWith('.apk'));
const missing = [];
const link = (item, os) => {
	const href = item.href ?? download(item.file);
	const title = item.title ?? `${item.file} · ${mb(sizes.get(item.file))}`;
	const alt = `${os} ${item.label} ${item.message}`;
	return `    <a href="${href}" title="${title}"><img src="${badge(item)}" alt="${alt}"></a>`;
};

const body = [];
for (const group of groups) {
	const rows = [];
	for (const row of group.rows) {
		const items = row.items.filter((item) => {
			if (item.needsApk) return hasApk;
			if (sizes.has(item.file)) return true;
			missing.push(item.file);
			return false;
		});
		if (items.length === 0) continue;
		rows.push(
			'        <tr>',
			`<td>${row.os}</td>`,
			'<td>',
			items.map((item) => link(item, row.os)).join('<br>\n'),
			'</td>',
			'        </tr>'
		);
	}
	if (rows.length === 0) continue;
	body.push(
		'        <tr>',
		`<td colspan="2" align="center"><strong>${group.title}</strong></td>`,
		'        </tr>',
		...rows
	);
}

if (body.length === 0) {
	console.error(`No downloadable assets found on ${tag}`);
	process.exit(1);
}
for (const file of missing) console.error(`warning: ${file} is not on ${tag}, skipped`);

const previous = JSON.parse(
	gh('release', 'list', '--repo', repo, '--exclude-drafts', '--limit', '20', '--json', 'tagName')
)
	.map((r) => r.tagName)
	.find((t) => /^v\d+\.\d+\.\d+$/.test(t) && compareVersions(t, tag) < 0);

function compareVersions(a, b) {
	const pa = a.slice(1).split('.').map(Number);
	const pb = b.slice(1).split('.').map(Number);
	for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] - pb[i];
	return 0;
}

const footer = [];
if (sizes.has('SHA256SUMS.txt')) {
	footer.push(`Android checksums: <a href="${download('SHA256SUMS.txt')}">SHA256SUMS.txt</a>`);
}
if (previous) {
	footer.push(
		`<a href="https://github.com/${repo}/compare/${previous}...${tag}">Changes since ${previous}</a>`
	);
}

const notes = [
	'<table>',
	'    <thead align="left">',
	'        <tr>',
	'<th>OS</th>',
	'<th>Download</th>',
	'        </tr>',
	'    </thead>',
	'    <tbody align="left">',
	...body,
	'    </tbody>',
	'</table>',
	...(footer.length ? ['', `<sub>${footer.join(' · ')}</sub>`] : []),
	''
].join('\n');

if (!apply) {
	process.stdout.write(notes);
} else {
	const file = join(mkdtempSync(join(tmpdir(), 'lumen-notes-')), 'notes.md');
	writeFileSync(file, notes);
	gh('release', 'edit', tag, '--repo', repo, '--notes-file', file);
	console.log(`Updated notes on ${repo} ${tag}`);
}
