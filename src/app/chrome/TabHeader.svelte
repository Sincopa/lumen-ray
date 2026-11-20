<script lang="ts">
	import { onDestroy } from 'svelte';
	import Morph from '../../components/Morph.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import { t } from '../../lib/i18n';
	import { morphWidth, swap } from '../../lib/motion';
	import { tooltip } from '../../lib/actions/tooltip';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
	import searchIcon from '../../assets/icons/search.svg';
	import crossIcon from '../../assets/icons/close.svg';
	import { tabHeader, type HeaderSlots } from '../tabHeader.svelte';

	// Keep the last content while the header leaves after its view released it.
	let lastSlots: HeaderSlots | null = null;
	const slots = $derived.by(() => {
		if (tabHeader.slots) lastSlots = tabHeader.slots;
		return lastSlots;
	});

	// Search lives in the header so that it survives the morph between tabs.
	const searchTask = createDeferredTask();
	onDestroy(searchTask.cancel);
	let searching = $state(false);
	let searchClosing = $state(false);
	let searchInputEl = $state<HTMLInputElement | null>(null);
	let rowEl = $state<HTMLElement | null>(null);
	// The field grows out of the magnifier and shrinks back into it.
	let searchOrigin = $state('inset(50% 50% 50% 50% round 999px)');

	function openSearch(event: MouseEvent) {
		const button = event.currentTarget as HTMLElement | null;
		if (button && rowEl) {
			const b = button.getBoundingClientRect();
			const p = rowEl.getBoundingClientRect();
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
			tabHeader.query = '';
		}, 320);
	}

	// A tab without search, or a new tab, starts with the field closed.
	let searchKey = '';
	$effect(() => {
		const key = slots?.searchPlaceholder ? slots.key : '';
		if (key === searchKey) return;
		searchKey = key;
		searchTask.cancel();
		searching = false;
		searchClosing = false;
	});
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && searching) closeSearch();
	}}
/>

{#if slots}
	<div class="tab-header">
		<!-- Figma "top buttons tun": the profile pill and the buttons pill. -->
		<div class="header-row" bind:this={rowEl}>
			<div class="title-pill" class:header-hidden={searching}>
				{#key slots.title}
					<span class="title-text" in:swap>{slots.title}</span>
				{/key}
				<StatusDot variant="active" />
			</div>
			<div
				class="actions-pill"
				class:dim={slots.dimActions}
				class:header-hidden={searching}
			>
				{#if slots.searchPlaceholder}
					<span class="morph-slot" in:morphWidth out:morphWidth>
						<button
							type="button"
							class="header-btn"
							aria-label={t('common.search')}
							use:tooltip={t('common.search')}
							onclick={openSearch}
						>
							<img src={searchIcon} alt="" width="28" height="28" />
						</button>
					</span>
				{/if}
				<Morph key={slots.key} anchor="end" class="header-morph">
					<span class="actions-set">{@render slots.actions?.()}</span>
				</Morph>
			</div>

			{#if slots.searchPlaceholder}
				<div
					class="header-search"
					class:search-open={searching && !searchClosing}
					class:search-closing={searchClosing}
					style:--search-origin={searchOrigin}
				>
					<img src={searchIcon} alt="" class="search-icon" />
					<input
						bind:this={searchInputEl}
						type="text"
						class="search-input"
						placeholder={slots.searchPlaceholder}
						bind:value={tabHeader.query}
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
			{/if}
		</div>
		{@render slots.popover?.()}
	</div>
{/if}

<style>
	.tab-header {
		position: relative;
		z-index: 20;
		flex-shrink: 0;
		width: 100%;
		padding-bottom: 16px;
	}
	.header-row {
		position: relative;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		width: 100%;
		height: var(--design-tab-header-height);
	}
	.title-pill,
	.actions-pill {
		display: flex;
		align-items: center;
		height: 100%;
		border-radius: 24px;
		background: var(--design-panel);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			opacity 220ms ease,
			filter 220ms ease;
	}
	/* 14px name and the 8px dot, 17px in from every side. */
	.title-pill {
		gap: 10px;
		min-width: 0;
		padding: 0 17px;
		font-size: 14px;
		line-height: 1;
	}
	.title-text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		line-height: 1.3;
	}
	.actions-pill {
		flex-shrink: 0;
		padding: 0 8px;
	}
	.header-hidden {
		opacity: 0;
		filter: blur(6px);
		pointer-events: none;
	}
	.morph-slot {
		display: inline-flex;
		box-sizing: border-box;
		justify-content: center;
		flex-shrink: 0;
		padding-right: 6px;
	}
	.actions-set {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	/* Buttons are 34px hit areas around 28px icons. */
	.header-btn,
	.actions-pill :global(.icon-btn.design) {
		display: grid;
		place-items: center;
		width: 34px;
		height: 34px;
		min-width: 34px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: transparent;
		transition:
			background-color 160ms ease,
			opacity 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.header-btn img,
	.actions-pill :global(.icon-btn.design .btn-icon),
	.actions-pill :global(.icon-btn.design > img) {
		display: block;
		width: 28px;
		height: 28px;
		max-width: none;
		max-height: none;
	}
	.dim .header-btn,
	.dim :global(.icon-btn.design:not(:disabled)) {
		opacity: 0.5;
	}
	.header-btn:active {
		transform: scale(0.92);
	}
	@media (hover: hover) and (pointer: fine) {
		.header-btn:hover {
			background: #ffffff14;
		}
		.dim .header-btn:hover,
		.dim :global(.icon-btn.design:hover:not(:disabled)) {
			opacity: 1;
		}
	}

	.header-search {
		position: absolute;
		inset: 0;
		padding: 0 6px 0 14px;
		display: flex;
		align-items: center;
		gap: 10px;
		box-sizing: border-box;
		/* The field is a lighter pill that grows out of the magnifier. */
		background: var(--design-panel-open);
		border-radius: 24px;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		clip-path: var(--search-origin);
		opacity: 0;
		pointer-events: none;
		transition:
			clip-path 260ms var(--ease-out),
			opacity 140ms ease 120ms;
	}
	.header-search > * {
		opacity: 0;
		filter: blur(6px);
		transition:
			opacity 160ms ease,
			filter 160ms ease;
	}
	.header-search.search-open {
		clip-path: inset(0 round 24px);
		opacity: 1;
		pointer-events: auto;
		transition:
			clip-path var(--morph-duration) var(--ease-morph),
			opacity 80ms ease;
	}
	.header-search.search-open > * {
		opacity: 1;
		filter: blur(0);
		transition:
			opacity 260ms ease 140ms,
			filter 260ms ease 140ms;
	}
	.header-search.search-closing {
		pointer-events: none;
	}
	.search-icon {
		width: 28px;
		height: 28px;
		object-fit: contain;
		opacity: 0.6;
		flex-shrink: 0;
	}
	.search-input {
		flex: 1;
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
		min-width: 0;
	}
	.search-input::placeholder {
		color: rgba(255, 255, 255, 0.4);
	}
	.close-search-btn {
		width: 32px;
		height: 32px;
		min-height: 0;
		border-radius: 50%;
		background: #ffffff;
		border: none;
		padding: 0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: transform 160ms var(--ease-spring);
	}
	.close-search-btn:hover {
		transform: scale(1.06);
	}
	.close-search-btn:active {
		transform: scale(0.94);
	}
	.close-icon {
		width: 24px;
		height: 24px;
		object-fit: contain;
		filter: brightness(0);
	}

	@media (prefers-reduced-motion: reduce) {
		.header-search,
		.header-search.search-open {
			clip-path: none;
			transition: opacity 120ms ease;
		}
	}
</style>
