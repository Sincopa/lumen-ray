<script lang="ts">
	import type { Snippet } from 'svelte';
	import chevronIcon from '../../assets/chevron.svg';

	interface Props {
		label: string;
		description?: string;
		class?: string;
		showChevron?: boolean;
		onclick?: () => void;
		children?: Snippet;
	}

	let {
		label,
		description,
		class: className = '',
		showChevron = false,
		onclick,
		children,
	}: Props = $props();
</script>

{#if onclick}
	<button type="button" class="setting-item clickable {className}" {onclick}>
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
			{#if showChevron}
				<img
					src={chevronIcon}
					alt=""
					width="16"
					height="16"
					class="chevron-icon"
					aria-hidden="true"
				/>
			{/if}
		</div>
	</button>
{:else}
	<div class="setting-item {className}">
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
			{#if showChevron}
				<img
					src={chevronIcon}
					alt=""
					width="16"
					height="16"
					class="chevron-icon"
					aria-hidden="true"
				/>
			{/if}
		</div>
	</div>
{/if}

<style>
	.setting-item {
		width: 100%;
		min-height: 48px;
		background: transparent;
		border: none;
		border-bottom: 1px solid rgba(255, 255, 255, 0.06);
		padding: 8px 16px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		box-sizing: border-box;
		text-align: left;
		transition: background 0.15s ease;
	}

	:global(.settings-group > .setting-item:first-child),
	:global(.settings-group > button.setting-item:first-child) {
		border-top-left-radius: 16px;
		border-top-right-radius: 16px;
	}

	:global(.settings-group > .setting-item:last-child),
	:global(.settings-group > button.setting-item:last-child) {
		border-bottom-left-radius: 16px;
		border-bottom-right-radius: 16px;
		border-bottom: none;
	}

	.setting-item.clickable {
		cursor: pointer;
	}

	.setting-item.clickable:hover {
		background: rgba(0, 0, 0, 0.4);
	}

	.setting-item.clickable:active {
		background: rgba(0, 0, 0, 0.5);
	}

	.item-text {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.label {
		font-family: var(--font-family-base);
		font-size: 14px;
		font-weight: 500;
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
		gap: 8px;
		flex-shrink: 0;
		align-self: center;
	}

	.chevron-icon {
		width: 14px;
		height: 14px;
		transform: translateX(0) rotate(-90deg);
		opacity: 0.65;
		flex-shrink: 0;
		align-self: center;
		display: block;
		transition:
			opacity 0.15s ease,
			transform 0.15s ease;
	}

	.setting-item.clickable:hover .chevron-icon {
		opacity: 1;
		transform: translateX(2px) rotate(-90deg);
	}
</style>
