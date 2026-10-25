<script lang="ts">
	import { runtime } from '../../../lib/runtime';
	import { resetNetworkSettings } from '../../../lib/services/settings';
	import { t } from '../../../lib/i18n';

	let resetting = $state(false);
	let success = $state(false);

	async function handleResetProxy() {
		if (!runtime.isTauri || resetting) return;
		resetting = true;
		success = false;
		try {
			await resetNetworkSettings();
			success = true;
			setTimeout(() => {
				success = false;
			}, 3000);
		} catch (e) {
			console.error('Failed to reset proxy:', e);
		} finally {
			resetting = false;
		}
	}
</script>

<div class="settings-group action-card">
	<p class="card-text">{t('settings.resetProxyDesc')}</p>
	<button
		type="button"
		class="wide-btn"
		class:success
		disabled={!runtime.isTauri || resetting}
		onclick={handleResetProxy}
	>
		{success ? t('settings.proxyResetSuccess') : t('settings.resetProxyAction')}
	</button>
</div>
