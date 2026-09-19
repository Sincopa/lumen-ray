<script lang="ts">
	interface Props {
		label: string;
		icon: string;
		variant?: 'default' | 'danger';
		disabled?: boolean;
		onclick?: (e: MouseEvent) => void;
	}

	let {
		label,
		icon,
		variant = 'default',
		disabled = false,
		onclick,
	}: Props = $props();
</script>

<button
	class="menu-item"
	class:danger={variant === 'danger'}
	{disabled}
	{onclick}
	type="button"
>
	<span class="menu-item-icon-circle">
		<img src={icon} alt="" />
	</span>
	<span class="menu-item-label">{label}</span>
</button>

<style>
	.menu-item {
		width: 100%;
		height: 40px;
		min-height: 0;
		border-radius: 14px;
		background: rgba(255, 255, 255, 0.04);
		border: 0;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 0 12px 0 8px;
		text-align: left;
		font-size: 13.5px;
		font-weight: 500;
		color: #ffffff;
		cursor: pointer;
		transition: background 0.18s ease;
		box-sizing: border-box;
	}

	.menu-item:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.12);
	}

	.menu-item.danger:hover:not(:disabled) {
		background: rgba(243, 57, 57, 0.2);
		color: #ff8080;
	}

	.menu-item:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.menu-item-icon-circle {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.1);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		color: #ffffff;
	}

	.menu-item-icon-circle img {
		width: 15px;
		height: 15px;
		filter: brightness(0) invert(1);
	}

	.menu-item-label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
