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
	/* Figma toggle: 60x28 track, 24px grey knob; the track turns white when on. */
	.switch-btn,
	.switch {
		position: relative;
		display: inline-block;
		width: 60px;
		height: 28px;
		flex-shrink: 0;
	}

	.switch-btn {
		padding: 0;
		border: none;
		background: transparent;
		cursor: pointer;
	}

	.switch-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
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
		background-color: rgba(255, 255, 255, 0.02);
		box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
		transition:
			background-color 0.25s ease,
			box-shadow 0.25s ease;
		border-radius: 123px;
	}

	.slider:before {
		position: absolute;
		content: '';
		height: 24px;
		width: 24px;
		left: 2px;
		top: 2px;
		background-color: #7e7e7e;
		transition: transform 0.25s cubic-bezier(0.22, 1, 0.36, 1);
		border-radius: 50%;
		box-shadow: 0 2px 7.7px rgba(0, 0, 0, 0.25);
	}

	input:checked + .slider,
	.slider.checked {
		background-color: #fff;
		box-shadow: none;
	}

	input:checked + .slider:before,
	.slider.checked:before {
		transform: translateX(32px);
	}

	input:disabled + .slider {
		opacity: 0.6;
		cursor: not-allowed;
	}

	input:focus-visible + .slider,
	.switch-btn:focus-visible .slider {
		outline: 2px solid rgba(255, 255, 255, 0.6);
		outline-offset: 2px;
	}
</style>
