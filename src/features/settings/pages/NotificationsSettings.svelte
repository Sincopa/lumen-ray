<script lang="ts">
	import SettingRow from '../../../components/SettingRow.svelte';
	import ToggleSwitch from '../../../components/ToggleSwitch.svelte';
	import { runtime } from '../../../lib/runtime';
	import {
		nativeSettings,
		updateNativeSettings,
	} from '../../../lib/services/settings';
	import { t } from '../../../lib/i18n';
</script>

<div class="settings-group">
	<SettingRow label={t('settings.notifyDisconnect')}>
		<ToggleSwitch
			mode="checkbox"
			disabled={!runtime.isTauri}
			ariaLabel={t('settings.notifyDisconnect')}
			checked={$nativeSettings.notifyDisconnect}
			onchange={(value) => updateNativeSettings({ notifyDisconnect: value })}
		/>
	</SettingRow>
	<SettingRow label={t('settings.notifyReconnect')}>
		<ToggleSwitch
			mode="checkbox"
			disabled={!runtime.isTauri}
			ariaLabel={t('settings.notifyReconnect')}
			checked={$nativeSettings.notifyReconnect}
			onchange={(value) => updateNativeSettings({ notifyReconnect: value })}
		/>
	</SettingRow>
	<SettingRow label={t('settings.notifyExpiry')}>
		<ToggleSwitch
			mode="checkbox"
			disabled={!runtime.isTauri}
			ariaLabel={t('settings.notifyExpiry')}
			checked={$nativeSettings.notifyExpiry}
			onchange={(value) => updateNativeSettings({ notifyExpiry: value })}
		/>
	</SettingRow>
	<SettingRow label={t('settings.notifyUpdate')}>
		<ToggleSwitch
			mode="checkbox"
			disabled={!runtime.isTauri}
			ariaLabel={t('settings.notifyUpdate')}
			checked={$nativeSettings.notifyUpdate}
			onchange={(value) => updateNativeSettings({ notifyUpdate: value })}
		/>
	</SettingRow>
</div>
