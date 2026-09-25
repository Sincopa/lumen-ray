import { invoke } from '@tauri-apps/api/core';
export interface NativeProcess {
	id: string;
	pid: number;
	name: string;
	detail: string;
	icon?: string;
}
export function getProcesses(): Promise<NativeProcess[]> {
	return invoke('get_processes');
}

export function getRuleIcons(names: string[]): Promise<Record<string, string>> {
	return invoke('get_rule_icons', { names });
}
