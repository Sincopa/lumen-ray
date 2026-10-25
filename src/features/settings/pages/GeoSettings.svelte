<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '../../../lib/i18n';
	import {
		getGeoDatabaseInfo,
		updateGeoDatabases,
		type GeoDatabaseInfo,
	} from '../../../lib/services/geoDatabase';
	import ActionCard from '../ui/ActionCard.svelte';
	import SettingHint from '../ui/SettingHint.svelte';

	let info = $state<GeoDatabaseInfo | null>(null);
	let updating = $state(false);
	let updateSuccess = $state(false);
	let error = $state<string | null>(null);

	onMount(() => void loadInfo());

	async function loadInfo() {
		try {
			info = await getGeoDatabaseInfo();
			if (!info.valid && info.error) {
				error = info.error;
			}
		} catch {
			error = 'GEO_DATA_MISSING';
		}
	}

	async function handleUpdate() {
		updating = true;
		error = null;
		updateSuccess = false;
		try {
			info = await updateGeoDatabases();
			if (!info.valid && info.error) {
				error = info.error;
			} else {
				updateSuccess = true;
				setTimeout(() => {
					updateSuccess = false;
				}, 4000);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'GEO_DATA_MISSING';
		} finally {
			updating = false;
		}
	}

	function hintText(): string | undefined {
		if (error) return t('errors.' + error) || t('settings.geoDataCorrupted');
		if (info) {
			const date = info.updatedAt
				? ' · ' + new Date(info.updatedAt).toLocaleDateString()
				: '';
			return `${info.source}${date}`;
		}
		return undefined;
	}
</script>

<ActionCard
	label={t('settings.currentVersion')}
	value={info?.version || '—'}
	action={updating
		? t('settings.downloadingUpdate')
		: updateSuccess
			? t('settings.latestVersionInstalled')
			: t('settings.updateDatabases')}
	onclick={handleUpdate}
	disabled={updating}
	success={updateSuccess}
	error={Boolean(error || (info && !info.valid))}
/>
<SettingHint text={hintText()} alert={Boolean(error)} />
