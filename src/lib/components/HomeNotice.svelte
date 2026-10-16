<script lang="ts">
	import bellIcon from '../../assets/bell-ring.svg';

	interface Props {
		icon: 'update' | 'bell';
		text: string;
		actionLabel?: string;
		actionDisabled?: boolean;
		onAction?: () => void;
	}

	let {
		icon,
		text,
		actionLabel,
		actionDisabled = false,
		onAction,
	}: Props = $props();
</script>

<!-- Figma 202:4868 / 202:4966 — "Component 31" notice pill under the header. -->
<div class="home-notice" class:has-action={!!actionLabel} role="status">
	<span class="notice-icon" aria-hidden="true">
		{#if icon === 'update'}
			<svg width="32" height="32" viewBox="0 0 32 32" fill="none">
				<path
					d="M16 8.6v10.2M11.9 15.1l4.1 4.1 4.1-4.1"
					stroke="#fff"
					stroke-width="1.6"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M8.5 20v2.4c0 .9.7 1.6 1.6 1.6h11.8c.9 0 1.6-.7 1.6-1.6V20"
					stroke="#fff"
					stroke-width="1.6"
					stroke-linecap="round"
				/>
			</svg>
		{:else}
			<img src={bellIcon} alt="" width="32" height="32" class="bell" />
		{/if}
	</span>
	<span class="notice-text">{text}</span>
	{#if actionLabel}
		<button
			type="button"
			class="notice-action"
			disabled={actionDisabled}
			onclick={() => onAction?.()}
		>
			{actionLabel}
		</button>
	{/if}
</div>

<style>
	.home-notice {
		display: flex;
		align-items: center;
		gap: 8px;
		max-width: 100%;
		height: 48px;
		padding: 8px 18px 8px 8px;
		border-radius: 16px;
		background: #00000026;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.home-notice.has-action {
		width: 304px;
		padding-right: 10px;
	}
	.notice-icon {
		display: grid;
		place-items: center;
		flex: 0 0 32px;
		width: 32px;
		height: 32px;
	}
	.bell {
		filter: brightness(0) invert(1);
	}
	.notice-text {
		min-width: 0;
		overflow: hidden;
		color: #fff;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.has-action .notice-text {
		margin-right: auto;
		color: #ffffff80;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.3px;
	}
	.notice-action {
		flex-shrink: 0;
		margin-left: 4px;
		height: 32px;
		padding: 8px 12px;
		border: 0;
		border-radius: 8px;
		background: #00000040;
		color: #fff;
		font-size: 12px;
		font-weight: 500;
		line-height: 15.6px;
		letter-spacing: -0.2px;
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.notice-action:active:not(:disabled) {
		transform: scale(0.96);
	}
	@media (hover: hover) and (pointer: fine) {
		.notice-action:hover:not(:disabled) {
			background: #00000066;
		}
	}
</style>
