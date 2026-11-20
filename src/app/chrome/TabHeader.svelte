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
	let actionsEl = $state<HTMLElement | null>(null);
	// The buttons' glass pill is what turns into the field and back: the field
	// is clipped to the pill's box and springs open to the whole row.
	let searchOrigin = $state('inset(0px 0px 0px 60% round 24px)');

	/** Offsets in layout pixels, so the clip stays right under the UI zoom. */
	function pillInset() {
		if (!actionsEl || !rowEl) return searchOrigin;
		const left = actionsEl.offsetLeft;
		const right = rowEl.offsetWidth - left - actionsEl.offsetWidth;
		return `inset(0px ${Math.max(0, right)}px 0px ${left}px round 24px)`;
	}

	function openSearch() {
		searchOrigin = pillInset();
		searching = true;
		searchClosing = false;
		searchTask.schedule(() => searchInputEl?.focus(), 160);
	}

	function closeSearch() {
		searchOrigin = pillInset();
		searchClosing = true;
		// The pill fades back in under the shrinking field, so there is no gap
		// between the field leaving and the buttons returning.
		searchTask.schedule(() => {
			searching = false;
			searchClosing = false;
			tabHeader.query = '';
		}, 400);
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
			<div class="title-pill" class:header-hidden={searching && !searchClosing}>
				{#key slots.title}
					<span class="title-text" in:swap>{slots.title}</span>
				{/key}
				<StatusDot variant="active" />
			</div>
			<div
				class="actions-pill"
				class:header-hidden={searching && !searchClosing}
				bind:this={actionsEl}
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
	/* Coming back (search closed): the pills condense out of a blur while the
	   field shrinks into the buttons' pill. */
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
			opacity 220ms ease 90ms,
			filter 300ms var(--ease-smooth) 90ms,
			transform 300ms var(--ease-smooth) 90ms;
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
	/* Figma "top button": the buttons rest at 50% and light up on hover. */
	.actions-pill {
		--icon-rest: 0.5;
		flex-shrink: 0;
		padding: 0 8px;
	}
	/* Leaving (search opened): quick, so the field is not drawn over them. */
	.header-hidden {
		opacity: 0;
		filter: blur(4px);
		transform: scale(0.97);
		pointer-events: none;
		transition:
			opacity 140ms ease-out,
			filter 160ms ease-out,
			transform 200ms ease-out;
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
	/* Buttons are 34px hit areas around 28px icons. Hover changes the icon,
	   never draws a circle behind it. */
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
		transition: transform 160ms var(--ease-spring);
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
	.header-btn img {
		opacity: var(--icon-rest);
		transition: opacity var(--hover-duration) var(--ease-hover);
	}
	.header-btn:active {
		transform: scale(0.92);
	}
	@media (hover: hover) and (pointer: fine) {
		.header-btn:hover img {
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
		/* Closing: the glass springs back to the pill without overshoot (an
		   overshooting clip would stall against the row's edge), then hands
		   over to the pill fading in beneath it. */
		transition:
			clip-path 380ms var(--ease-smooth),
			opacity 120ms ease 210ms;
	}
	.header-search > * {
		opacity: 0;
		filter: blur(6px);
		transition:
			opacity 120ms ease-out,
			filter 140ms ease-out;
	}
	/* Opening: the pill's glass grows to the full row; its content condenses
	   out of a blur once the shape is under way. */
	.header-search.search-open {
		clip-path: inset(0px 0px 0px 0px round 24px);
		opacity: 1;
		pointer-events: auto;
		transition:
			clip-path var(--smooth-duration) var(--ease-smooth),
			opacity 60ms ease;
	}
	.header-search.search-open > * {
		opacity: 1;
		filter: blur(0);
		transition:
			opacity 240ms ease 90ms,
			filter 300ms var(--ease-smooth) 90ms;
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
		.title-pill,
		.actions-pill,
		.header-hidden {
			filter: none;
			transform: none;
		}
	}
</style>
