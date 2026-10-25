<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { runtime } from '../../../lib/runtime';
	import { t } from '../../../lib/i18n';
	import SettingHint from '../ui/SettingHint.svelte';

	let opening = $state(false);
	let failed = $state(false);

	async function openLogConsole() {
		if (!runtime.isTauri || opening) return;
		opening = true;
		failed = false;
		try {
			await invoke('open_log_console');
		} catch {
			failed = true;
		} finally {
			opening = false;
		}
	}
</script>

<div class="settings-group action-card">
	<p class="card-text">{t('settings.logConsoleDesc')}</p>
	<button
		type="button"
		class="wide-btn"
		disabled={!runtime.isTauri || opening}
		onclick={() => void openLogConsole()}
	>
		{opening ? t('common.loading') : t('settings.openLogConsole')}
	</button>
</div>
{#if failed}
	<SettingHint text={t('settings.logConsoleError')} alert />
{/if}
