<script lang="ts">
	import reloadIcon from '../../../assets/reload.svg';
	import menuIcon from '../../../assets/menu.svg';
	import chevronIcon from '../../../assets/chevron-up.svg';
	import { t } from '../../../lib/i18n';
	import IconButton from '../../../components/IconButton.svelte';

	interface Props {
		name: string;
		busy: boolean;
		menuOpen: boolean;
		expanded: boolean;
		onRefresh: () => void;
		onToggleMenu: (event: MouseEvent) => void;
		onToggleExpanded: () => void;
	}
	let {
		name,
		busy,
		menuOpen,
		expanded,
		onRefresh,
		onToggleMenu,
		onToggleExpanded,
	}: Props = $props();
</script>

<IconButton
	design
	icon={reloadIcon}
	ariaLabel={t('common.reload')}
	disabled={busy}
	class={busy ? 'card-refresh refreshing' : 'card-refresh'}
	onclick={onRefresh}
/>
<IconButton
	design
	icon={menuIcon}
	ariaLabel={t('common.menu')}
	active={menuOpen}
	expanded={menuOpen}
	onclick={onToggleMenu}
/>
<IconButton
	design
	icon={chevronIcon}
	ariaLabel={name}
	{expanded}
	class="card-chevron"
	onclick={onToggleExpanded}
/>

<style>
	:global(.card-chevron) {
		transform: rotate(180deg);
		transition: transform 200ms var(--ease-out);
	}
	:global(.card-chevron.expanded) {
		transform: rotate(0);
	}
	/* Only the card being refreshed spins; the others stay available. */
	:global(.card-refresh.refreshing .btn-icon) {
		animation: card-refresh-spin 900ms linear infinite;
	}
	@keyframes -global-card-refresh-spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		:global(.card-chevron) {
			transition: none;
		}
		:global(.card-refresh.refreshing .btn-icon) {
			animation: none;
		}
	}
</style>
