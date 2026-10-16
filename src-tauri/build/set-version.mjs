import { readFileSync, writeFileSync } from 'node:fs';

const version = process.argv[2]?.trim();
if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
	console.error('Usage: npm run release:version -- 0.2.0');
	process.exit(1);
}

function updateJson(path, update) {
	const value = JSON.parse(readFileSync(path, 'utf8'));
	update(value);
	writeFileSync(path, `${JSON.stringify(value, null, '\t')}\n`);
}

updateJson('package.json', (value) => {
	value.version = version;
});
updateJson('package-lock.json', (value) => {
	value.version = version;
	if (value.packages?.['']) value.packages[''].version = version;
});
updateJson('src-tauri/tauri.conf.json', (value) => {
	value.version = version;
});

const cargoPath = 'src-tauri/Cargo.toml';
const cargo = readFileSync(cargoPath, 'utf8');
const updatedCargo = cargo.replace(
	/(\[package\][\s\S]*?\nversion\s*=\s*)"[^"]+"/,
	`$1"${version}"`,
);
if (updatedCargo === cargo) {
	throw new Error('Could not update src-tauri/Cargo.toml');
}
writeFileSync(cargoPath, updatedCargo);

const cargoLockPath = 'src-tauri/Cargo.lock';
const cargoLock = readFileSync(cargoLockPath, 'utf8');
const updatedCargoLock = cargoLock.replace(
	/(\[\[package\]\]\r?\nname = "app"\r?\nversion = ")[^"]+"/,
	(_match, prefix) => `${prefix}${version}"`,
);
if (updatedCargoLock === cargoLock) {
	throw new Error('Could not update src-tauri/Cargo.lock');
}
writeFileSync(cargoLockPath, updatedCargoLock);

console.log(`Lumen version updated to ${version}.`);
