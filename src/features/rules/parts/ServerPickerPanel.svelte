<script lang="ts">
	import ServersView from '../../servers/ServersView.svelte';
	import IconButton from '../../../components/IconButton.svelte';
	import { t } from '../../../lib/i18n';
	import backIcon from '../../../assets/back.svg';
	import searchIcon from '../../../assets/search.svg';
	import checkIcon from '../../../assets/check.svg';

	interface Props {
		selectedId: string;
		searchSignal: number;
		onSelect: (server: {
			id: string;
			countryCode: string;
			name: string;
		}) => void;
		onSearch: () => void;
		onConfirm: () => void;
		/** Shown when the picker is opened from inside the rule editor. */
		onBack?: () => void;
	}
	let {
		selectedId,
		searchSignal,
		onSelect,
		onSearch,
		onConfirm,
		onBack,
	}: Props = $props();
</script>

<div class="server-picker-view">
	<ServersView {searchSignal} {selectedId} {onSelect} compact />
	<div class="design-action-bar picker-bar">
		{#if onBack}
			<IconButton
				design
				icon={backIcon}
				ariaLabel={t('common.back')}
				size="lg"
				variant="surface"
				onclick={onBack}
			/>
		{/if}
		<IconButton
			design
			icon={searchIcon}
			ariaLabel={t('common.search')}
			size="lg"
			variant="surface"
			onclick={onSearch}
		/>
		<IconButton
			design
			icon={checkIcon}
			ariaLabel={t('common.save')}
			size="lg"
			variant="primary"
			onclick={onConfirm}
		/>
	</div>
</div>

<style>
	.server-picker-view {
		position: relative;
		height: 100%;
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		gap: 10px;
		overflow: hidden;
	}
</style>
