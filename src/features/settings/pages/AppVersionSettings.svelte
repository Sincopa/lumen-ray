<script lang="ts">
	import { applicationUpdate } from '../../../lib/services/applicationUpdate';
	import { t } from '../../../lib/i18n';
	import ActionCard from '../ui/ActionCard.svelte';

	const updateState = applicationUpdate.state;

	function label(): string {
		switch ($updateState.status) {
			case 'checking':
				return t('settings.checkingUpdates');
			case 'downloading':
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
		if ($updateState.status === 'available') {
			void applicationUpdate.install();
		} else {
			void applicationUpdate.check();
		}
	}
</script>

<ActionCard
	label={t('settings.currentVersion')}
	value={$updateState.currentVersion}
	action={label()}
	onclick={handleAction}
	disabled={['checking', 'downloading'].includes($updateState.status)}
	success={$updateState.status === 'latest'}
/>
