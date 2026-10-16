<script lang="ts">
	import chevronIcon from '../assets/chevron-right-small.svg';
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		description?: string;
		/** Current value shown as a chip before the chevron. */
		value?: string;
		class?: string;
		showChevron?: boolean;
		onclick?: () => void;
		children?: Snippet;
	}

	let {
		label,
		description,
		value,
		class: className = '',
		showChevron = false,
		onclick,
		children,
	}: Props = $props();
</script>

{#snippet content()}
	<div class="item-text">
		<span class="label">{label}</span>
		{#if description}
			<span class="desc">{description}</span>
		{/if}
	</div>
	<div class="control-area">
		{#if children}
			{@render children()}
		{/if}
		{#if value}
			<span class="value-chip">{value}</span>
		{/if}
		{#if showChevron}
			<span class="chevron" aria-hidden="true">
				<img src={chevronIcon} alt="" width="6" height="12" />
			</span>
		{/if}
	</div>
{/snippet}

{#if onclick}
	<button type="button" class="setting-item clickable {className}" {onclick}>
		{@render content()}
	</button>
{:else}
	<div class="setting-item {className}">
		{@render content()}
	</div>
{/if}

<style>
	.setting-item {
		position: relative;
		width: 100%;
		min-height: 54px;
		background: transparent;
		border: none;
		padding: 8px 0;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		text-align: left;
		color: #fff;
	}

	.setting-item.clickable {
		cursor: pointer;
	}

	.item-text {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}

	.label {
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 400;
		letter-spacing: -0.02em;
		color: #ffffff;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.desc {
		font-family: var(--font-family-base);
		font-size: 12px;
		color: rgba(255, 255, 255, 0.45);
	}

	.control-area {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		flex-shrink: 0;
		align-self: center;
	}

	.value-chip {
		display: inline-flex;
		align-items: center;
		height: 33px;
		padding: 0 12px;
		border-radius: 123px;
		background: rgba(0, 0, 0, 0.15);
		font-family: var(--font-family-base);
		font-size: 12px;
		white-space: nowrap;
		color: #fff;
	}

	.chevron {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		opacity: 0.5;
		transition:
			opacity 0.15s ease,
			transform 0.15s ease;
	}

	@media (hover: hover) and (pointer: fine) {
		.setting-item.clickable:hover .chevron {
			opacity: 0.9;
			transform: translateX(2px);
		}
	}
	.setting-item.clickable:active .chevron {
		transform: translateX(2px);
	}
</style>
