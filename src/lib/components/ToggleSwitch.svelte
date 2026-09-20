<script lang="ts">
	interface Props {
		checked: boolean;
		mode?: 'switch' | 'checkbox';
		disabled?: boolean;
		ariaLabel?: string;
		class?: string;
		onchange?: (checked: boolean) => void;
	}

	let {
		checked = $bindable(false),
		mode = 'switch',
		disabled = false,
		ariaLabel,
		class: className = '',
		onchange,
	}: Props = $props();

	function toggle() {
		if (disabled) return;
		checked = !checked;
		onchange?.(checked);
	}
</script>

{#if mode === 'switch'}
	<button
		type="button"
		class="switch-btn {className}"
		class:enabled={checked}
		role="switch"
		aria-checked={checked}
		aria-label={ariaLabel}
		{disabled}
		onclick={toggle}
	>
		<span class="slider" class:checked></span>
	</button>
{:else}
	<label class="switch {className}">
		<input
			type="checkbox"
			{disabled}
			aria-label={ariaLabel}
			bind:checked
			onchange={(e) => onchange?.(e.currentTarget.checked)}
		/>
		<span class="slider"></span>
	</label>
{/if}

<style>
	.switch-btn {
		position: relative;
		display: inline-block;
		width: 44px;
		height: 24px;
		padding: 0;
		border: none;
		background: transparent;
		cursor: pointer;
		flex-shrink: 0;
	}

	.switch-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.switch {
		position: relative;
		display: inline-block;
		width: 44px;
		height: 24px;
		flex-shrink: 0;
	}

	.switch input {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		margin: 0;
		padding: 0;
		opacity: 0.001;
		cursor: pointer;
		z-index: 2;
	}

	.slider {
		position: absolute;
		cursor: pointer;
		inset: 0;
		background-color: rgba(0, 0, 0, 0.35);
		transition: background-color 0.25s ease;
		border-radius: 24px;
	}

	.slider:before {
		position: absolute;
		content: '';
		height: 18px;
		width: 18px;
		left: 3px;
		bottom: 3px;
		background-color: #ffffff;
		transition: transform 0.25s cubic-bezier(0.22, 1, 0.36, 1);
		border-radius: 50%;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
	}

	input:checked + .slider,
	.slider.checked {
		background-color: rgba(0, 0, 0, 0.35);
	}

	input:checked + .slider:before,
	.slider.checked:before {
		transform: translateX(20px);
	}

	input:disabled + .slider {
		opacity: 0.6;
		cursor: not-allowed;
	}
</style>
