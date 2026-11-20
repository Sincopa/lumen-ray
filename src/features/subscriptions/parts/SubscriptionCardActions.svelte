<script lang="ts">
	import reloadIcon from '../../../assets/icons/reload.svg';
	import moreIcon from '../../../assets/icons/more.svg';
	import { t } from '../../../lib/i18n';
	import IconButton from '../../../components/IconButton.svelte';

	interface Props {
		busy: boolean;
		menuOpen: boolean;
		onRefresh: () => void;
		onToggleMenu: (event: MouseEvent) => void;
	}
	let { busy, menuOpen, onRefresh, onToggleMenu }: Props = $props();
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
	icon={moreIcon}
	ariaLabel={t('common.menu')}
	active={menuOpen}
	expanded={menuOpen}
	onclick={onToggleMenu}
/>

<style>
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
		:global(.card-refresh.refreshing .btn-icon) {
			animation: none;
		}
	}
</style>
