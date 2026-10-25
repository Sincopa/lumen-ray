<script lang="ts">
	import { runtime } from '../../../lib/runtime';
	import { nativeSettings } from '../../../lib/services/settings';
	import { coreUpdate } from '../../../lib/services/coreUpdate';
	import { t } from '../../../lib/i18n';
	import ActionCard from '../ui/ActionCard.svelte';

	const updateState = coreUpdate.state;

	function label(): string {
		switch ($updateState.status) {
			case 'checking':
				return t('settings.checkingUpdates');
			case 'installing':
				return t('settings.downloadingUpdate');
			case 'available':
				return t('settings.downloadUpdate', {
					version: $updateState.version ?? '',
				});
			case 'latest':
				return t('settings.latestVersionInstalled');
			case 'error':
				return t('common.retry');
			default:
				return t('settings.checkUpdates');
		}
	}

	function handleAction() {
		if ($updateState.status === 'available') void coreUpdate.install();
		else void coreUpdate.check();
	}
</script>

<ActionCard
	label={t('settings.currentVersion')}
	value={runtime.isTauri
		? $nativeSettings.coreVersion
		: t('settings.mockReady')}
	action={label()}
	onclick={handleAction}
	disabled={!runtime.isTauri ||
		['checking', 'installing'].includes($updateState.status)}
	success={$updateState.status === 'latest'}
/>
