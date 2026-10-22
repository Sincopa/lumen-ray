<script lang="ts">
	import { onDestroy } from 'svelte';
	import IconButton from '../../components/IconButton.svelte';
	import Morph from '../../components/Morph.svelte';
	import StatusDot from '../../components/StatusDot.svelte';
	import { t } from '../../lib/i18n';
	import {
		morphGrow,
		morphOut,
		morphWidth,
		reveal,
		swap,
	} from '../../lib/motion';
	import { createDeferredTask } from '../../lib/utils/deferredTask';
	import searchIcon from '../../assets/search.svg';
	import crossIcon from '../../assets/close.svg';
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
	let pillEl = $state<HTMLElement | null>(null);
	// The field grows out of the magnifier and shrinks back into it.
	let searchOrigin = $state('inset(50% 50% 50% 50% round 999px)');

	function openSearch(event: MouseEvent) {
		const button = event.currentTarget as HTMLElement | null;
		if (button && pillEl) {
			const b = button.getBoundingClientRect();
			const p = pillEl.getBoundingClientRect();
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
		<div class="header-row">
			<div class="header-pill" class:has-body={!!slots.body} bind:this={pillEl}>
				<!-- Title row and rename field share one cell and morph into each other. -->
				<div class="header-line" class:header-hidden={searching}>
					{#if slots.replace}
						<div class="replace" in:morphGrow out:morphOut>
							{@render slots.replace()}
						</div>
					{:else}
						<div class="line-normal" in:swap out:morphOut>
							<div class="header-title">
								{#key slots.title}
									<span class="title-text" in:swap>{slots.title}</span>
								{/key}
								<StatusDot variant="active" />
							</div>
							<div class="header-actions">
								{#if slots.searchPlaceholder}
									<span class="morph-slot" transition:morphWidth>
										<IconButton
											design
											icon={searchIcon}
											ariaLabel={t('common.search')}
											onclick={openSearch}
										/>
									</span>
								{/if}
								<Morph key={slots.key} anchor="end" class="header-morph">
									<span class="actions-set">{@render slots.actions?.()}</span>
								</Morph>
							</div>
						</div>
					{/if}
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

				{#if slots.body}
					<div class="header-body" transition:reveal>
						{@render slots.body()}
					</div>
				{/if}
			</div>

			{#if slots.trailing}
				<span class="morph-slot trailing" transition:morphWidth>
					{@render slots.trailing()}
				</span>
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
		padding-bottom: 12px;
	}
	.header-row {
		display: flex;
		align-items: flex-start;
		width: 100%;
	}
	.header-pill {
		position: relative;
		flex: 1;
		min-width: 0;
		min-height: var(--design-header-height);
		/* Half the height: a pill when collapsed, a card when it grows. */
		border-radius: calc(var(--design-header-height) / 2);
		background: var(--design-panel);
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		overflow: hidden;
		transition: background-color 240ms ease;
	}
	.header-pill.has-body {
		background: var(--design-panel-open);
	}
	/* One grid cell: the leaving row and the arriving one overlap. */
	.header-line {
		display: grid;
		align-items: center;
		height: var(--design-header-height);
		padding: 0 20px 0 23px;
		box-sizing: border-box;
		transition:
			opacity 220ms ease,
			filter 220ms ease;
	}
	.header-line > * {
		grid-area: 1 / 1;
		min-width: 0;
	}
	.header-line.header-hidden {
		opacity: 0;
		filter: blur(6px);
		pointer-events: none;
	}
	.line-normal {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
	}
	.replace {
		display: flex;
		align-items: center;
		width: 100%;
		height: 100%;
	}
	.header-title {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		font-family: 'Satoshi', var(--font-family-base);
		font-size: 16px;
		font-weight: 500;
		letter-spacing: -0.2px;
	}
	.title-text {
		line-height: 1.3;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.header-actions {
		display: flex;
		align-items: center;
		flex-shrink: 0;
		height: 100%;
	}
	.morph-slot {
		display: inline-flex;
		box-sizing: border-box;
		justify-content: center;
		flex-shrink: 0;
	}
	.header-actions .morph-slot {
		padding-right: 6px;
	}
	.morph-slot.trailing {
		align-self: center;
		justify-content: flex-end;
		padding-left: 8px;
	}
	.actions-set {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.header-body {
		padding: 0 20px 20px 23px;
	}

	.header-search {
		position: absolute;
		left: 0;
		top: 0;
		width: 100%;
		height: var(--design-header-height);
		padding: 0 8px 0 20px;
		display: flex;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		/* The field is a lighter pill that grows out of the magnifier. */
		background: var(--design-panel-open);
		border-radius: calc(var(--design-header-height) / 2);
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
		clip-path: inset(0 round calc(var(--design-header-height) / 2));
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
		filter: brightness(0) invert(1);
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
		border-radius: 50%;
		background: #ffffff;
		border: none;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
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

	@media (max-width: 350px) {
		.header-line {
			padding-left: 16px;
			padding-right: 12px;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.header-search,
		.header-search.search-open {
			clip-path: none;
			transition: opacity 120ms ease;
		}
	}
</style>
