<script lang="ts">
	import { onDestroy } from 'svelte';
	import ServersView from '../../servers/ServersView.svelte';
	import IconButton from '../../../components/IconButton.svelte';
	import { t } from '../../../lib/i18n';
	import { createDeferredTask } from '../../../lib/utils/deferredTask';
	import backIcon from '../../../assets/back.svg';
	import searchIcon from '../../../assets/search.svg';
	import crossIcon from '../../../assets/close.svg';
	import checkIcon from '../../../assets/check.svg';

	interface Props {
		selectedId: string;
		onSelect: (server: {
			id: string;
			countryCode: string;
			name: string;
		}) => void;
		onConfirm: () => void;
		/** Shown when the picker is opened from inside the rule editor. */
		onBack?: () => void;
	}
	let { selectedId, onSelect, onConfirm, onBack }: Props = $props();

	const searchTask = createDeferredTask();
	onDestroy(searchTask.cancel);
	let query = $state('');
	let searching = $state(false);
	let searchClosing = $state(false);
	let searchInputEl = $state<HTMLInputElement | null>(null);
	let barEl = $state<HTMLElement | null>(null);
	// The field unrolls from the magnifier across the bar and rolls back into it.
	let searchOrigin = $state('inset(0 100% 0 0 round 999px)');

	function openSearch(event: MouseEvent) {
		const button = event.currentTarget as HTMLElement | null;
		if (button && barEl) {
			const b = button.getBoundingClientRect();
			const p = barEl.getBoundingClientRect();
			searchOrigin =
				`inset(${b.top - p.top}px ${p.right - b.right}px ` +
				`${p.bottom - b.bottom}px ${b.left - p.left}px round 999px)`;
		}
		searching = true;
		searchClosing = false;
		searchTask.schedule(() => searchInputEl?.focus(), 180);
	}

	function closeSearch() {
		searchClosing = true;
		searchTask.schedule(() => {
			searching = false;
			searchClosing = false;
			query = '';
		}, 320);
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && searching && !searchClosing) {
			event.preventDefault();
			closeSearch();
		}
	}}
/>

<div class="server-picker-view">
	<ServersView {query} {selectedId} {onSelect} compact />
	<div class="design-action-bar picker-bar" bind:this={barEl}>
		<div class="bar-buttons" class:bar-hidden={searching && !searchClosing}>
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
				onclick={openSearch}
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
		<div
			class="picker-search"
			class:search-open={searching && !searchClosing}
			style:--search-origin={searchOrigin}
		>
			<img src={searchIcon} alt="" class="search-icon" />
			<input
				bind:this={searchInputEl}
				type="text"
				class="search-input"
				placeholder={t('servers.searchPlaceholder')}
				bind:value={query}
				autocomplete="off"
				spellcheck="false"
				tabindex={searching ? 0 : -1}
			/>
			<button
				type="button"
				class="close-search-btn"
				onclick={closeSearch}
				aria-label={t('common.close')}
				tabindex={searching ? 0 : -1}
			>
				<img src={crossIcon} alt="" class="close-icon" />
			</button>
		</div>
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
	.bar-buttons {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		height: 100%;
		transition:
			opacity 200ms ease,
			transform 200ms ease;
	}
	.bar-buttons.bar-hidden {
		opacity: 0;
		transform: scale(0.96);
		pointer-events: none;
	}
	/* Same field as the tab header search, sized to the bottom bar. */
	.picker-search {
		position: absolute;
		inset: 0;
		padding: 0 8px 0 14px;
		display: flex;
		align-items: center;
		gap: 10px;
		box-sizing: border-box;
		background: var(--design-panel-open);
		-webkit-backdrop-filter: blur(16px);
		backdrop-filter: blur(16px);
		border-radius: 999px;
		clip-path: var(--search-origin);
		opacity: 0;
		pointer-events: none;
		transition:
			clip-path 260ms var(--ease-out),
			opacity 140ms ease 120ms;
	}
	.picker-search > * {
		opacity: 0;
		filter: blur(6px);
		transition:
			opacity 160ms ease,
			filter 160ms ease;
	}
	.picker-search.search-open {
		clip-path: inset(0 round 999px);
		opacity: 1;
		pointer-events: auto;
		transition:
			clip-path var(--morph-duration) var(--ease-morph),
			opacity 80ms ease;
	}
	.picker-search.search-open > * {
		opacity: 1;
		filter: blur(0);
		transition:
			opacity 260ms ease 140ms,
			filter 260ms ease 140ms;
	}
	.search-icon {
		width: 28px;
		height: 28px;
		object-fit: contain;
		filter: brightness(0) invert(1);
		opacity: 0.6;
		flex-shrink: 0;
	}
	.search-input {
		flex: 1;
		min-width: 0;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		border-radius: 0 !important;
		color: #ffffff;
		font-size: 14px;
		font-family: inherit;
		padding: 0 !important;
		-webkit-appearance: none;
		appearance: none;
	}
	.search-input::placeholder {
		color: rgba(255, 255, 255, 0.4);
	}
	.close-search-btn {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: #ffffff;
		border: none;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		flex-shrink: 0;
		transition: transform 160ms var(--ease-spring);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
	}
	.close-search-btn:hover {
		transform: scale(1.06);
	}
	.close-search-btn:active {
		transform: scale(0.94);
	}
	.close-icon {
		width: 14px;
		height: 14px;
		object-fit: contain;
		filter: brightness(0);
	}
	@media (prefers-reduced-motion: reduce) {
		.picker-search,
		.picker-search.search-open {
			clip-path: none;
			transition: opacity 120ms ease;
		}
	}
</style>
