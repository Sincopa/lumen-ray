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

	const examples = {
		geosite: ['category-ru', 'antifilter', 'antifilter-community'],
		geoip: ['ru', 'private', 'antifilter'],
	} as const;

	const capitalize = (value: string) =>
		value.charAt(0).toUpperCase() + value.slice(1);
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
<div class="geo-examples">
	<span class="example-label">{t('rules.examplesLabel')}</span>
	<div class="example-chips">
		{#each examples[matcher] as ex (ex)}
			<button
				type="button"
				class="example-tag"
				class:active={name.trim().toLowerCase() === ex}
				onclick={() => {
					name = ex;
					onEdit();
				}}
			>
				{capitalize(ex)}
			</button>
		{/each}
	</div>
</div>
