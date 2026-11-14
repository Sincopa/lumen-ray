<script lang="ts">
	import { t } from '../../../lib/i18n';
	import DesignTextField from '../../../components/DesignTextField.svelte';

	interface Props {
		matcher: 'geosite' | 'geoip';
		name: string;
		invalid: boolean;
		onEdit: () => void;
		onPaste: (event: ClipboardEvent) => void;
	}
	let {
		matcher,
		name = $bindable(),
		invalid,
		onEdit,
		onPaste,
	}: Props = $props();
</script>

<DesignTextField
	id="geo-input"
	label={t('rules.geoCategory', {
		type: matcher === 'geosite' ? 'GeoSite' : 'GeoIP',
	})}
	prefix={matcher === 'geosite' ? 'geosite:' : 'geoip:'}
	placeholder={matcher === 'geosite' ? 'category-ru' : 'ru'}
	bind:value={name}
	{invalid}
	oninput={onEdit}
	onpaste={onPaste}
/>
