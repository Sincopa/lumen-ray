import { invoke } from '@tauri-apps/api/core';
import { runtime } from '../runtime';

export interface GeoDatabaseInfo {
	version: string;
	source: string;
	updatedAt: number;
	status: string;
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
			updatedAt: Date.now(),
			status: 'ready',
			geoipSize: 536598,
			geositeSize: 17015282,
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
			updatedAt: Date.now(),
			status: 'ready',
			geoipSize: 536598,
			geositeSize: 17015282,
			valid: true,
			error: null,
		};
	}
	return invoke<GeoDatabaseInfo>('update_geo_databases');
}
