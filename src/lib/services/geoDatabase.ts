import { invoke } from '@tauri-apps/api/core';
import { runtime } from '../runtime';

export interface GeoDatabaseInfo {
	version: string;
	source: string;
	lastUpdated: string | null;
	geoipSize: number;
	geositeSize: number;
	valid: boolean;
	error: string | null;
}

export async function getGeoDatabaseInfo(): Promise<GeoDatabaseInfo> {
	if (!runtime.isTauri) {
		return {
			version: '202410152325',
			source: 'NetworkKeeper/v2ray-rules-ru',
			lastUpdated: new Date().toISOString(),
			geoipSize: 2200000,
			geositeSize: 4500000,
			valid: true,
			error: null,
		};
	}
	return invoke<GeoDatabaseInfo>('get_geo_database_info');
}

export async function updateGeoDatabases(): Promise<GeoDatabaseInfo> {
	if (!runtime.isTauri) {
		return {
			version: '202410152325',
			source: 'NetworkKeeper/v2ray-rules-ru',
			lastUpdated: new Date().toISOString(),
			geoipSize: 2200000,
			geositeSize: 4500000,
			valid: true,
			error: null,
		};
	}
	return invoke<GeoDatabaseInfo>('update_geo_databases');
}

