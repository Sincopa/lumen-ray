<script lang="ts">
	import clearIcon from '../assets/icons/close.svg';
	import { t } from '../lib/i18n';

	interface Props {
		id: string;
		label: string;
		value: string;
		prefix?: string;
		placeholder?: string;
		invalid?: boolean;
		maxlength?: number;
		spellcheck?: boolean;
		oninput?: () => void;
		onpaste?: (event: ClipboardEvent) => void;
	}

	let {
		id,
		label,
		value = $bindable(),
		prefix,
		placeholder,
		invalid = false,
		maxlength,
		spellcheck = false,
		oninput,
		onpaste,
	}: Props = $props();

	let input = $state<HTMLInputElement>();
	let focused = $state(false);
	const filled = $derived(!!value);
	// Figma "txt select": an empty field without a prefix shows only its label,
	// centred at 14px; focus or a value lifts it into the 12px caption.
	const prompt = $derived(!prefix && !filled && !focused);

	// The prefix and the paddings around the input focus it too.
	function focusInput(event: PointerEvent) {
		const target = event.target as HTMLElement;
		if (target === input || target.closest('button')) return;
		event.preventDefault();
		input?.focus();
	}
</script>

<!-- Figma Component 58: the label sits inside the pill, above the value. -->
<div
	class="design-field"
	class:filled
	class:prompt
	class:invalid
	role="presentation"
	onpointerdown={focusInput}
>
	<div class="field-body">
		<label class="field-label" for={id}>{label}</label>
		<div class="value-row">
			{#if prefix}<span class="input-prefix">{prefix}</span>{/if}
			<input
				bind:this={input}
				{id}
				class="text-input"
				type="text"
				bind:value
				{oninput}
				{onpaste}
				onfocus={() => (focused = true)}
				onblur={() => (focused = false)}
				{placeholder}
				{maxlength}
				{spellcheck}
				autocomplete="off"
				aria-invalid={invalid}
			/>
		</div>
	</div>
	<!-- Stays in the layout and fades with the value, like Smart Animate. -->
	<button
		type="button"
		class="clear-btn"
		aria-label={t('rules.clear')}
		tabindex={filled ? 0 : -1}
		aria-hidden={!filled}
		onclick={() => {
			value = '';
			oninput?.();
			input?.focus();
		}}
	>
		<img src={clearIcon} alt="" width="32" height="32" />
	</button>
</div>

<style>
	.design-field {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 60px;
		padding: 0 14px 0 24px;
		border-radius: 24px;
		/* 2% white empty. */
		background-color: var(--design-panel);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		box-sizing: border-box;
		cursor: text;
		transition: background-color var(--field-duration) var(--ease-field);
	}
	/* Figma stacks 5% white over 20% black; one color keeps it animatable. */
	.design-field.filled {
		background-color: rgba(53, 53, 53, 0.24);
	}
	/* The same black base under a 12% red tint. */
	.design-field.invalid {
		background-color: rgba(104, 44, 42, 0.3);
	}

	/* Geist centers its caps in a 1.3em line, so a line box centered on the
	   Figma cap box lands on it: label caps at y 18-27, value caps at 33-43. */
	.field-body {
		position: relative;
		flex: 1;
		min-width: 0;
		height: 100%;
	}
	/* Laid out as the 14px prompt and shrunk into the caption, so the move
	   between them is one transform, like the prototype's Smart Animate. */
	.field-label {
		position: absolute;
		top: 20.9px;
		left: 0;
		right: 0;
		overflow: hidden;
		color: #fff;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.28px;
		text-overflow: ellipsis;
		white-space: nowrap;
		pointer-events: none;
		transform: translateY(-7.5px) scale(0.8571);
		transform-origin: left center;
		transition:
			transform var(--field-duration) var(--ease-field),
			color var(--field-duration) var(--ease-field);
	}
	.filled .field-label {
		color: #ffffff40;
	}
	.prompt .field-label {
		color: #ffffff80;
		transform: none;
	}
	.prompt .text-input::placeholder {
		color: transparent;
	}
	.value-row {
		display: flex;
		align-items: flex-start;
		gap: 2px;
		height: 100%;
	}
	.input-prefix {
		flex-shrink: 0;
		padding-top: 28.9px;
		color: #ffffff40;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.28px;
		user-select: none;
	}
	/* Full height, so a click anywhere below the label lands in the input. */
	.text-input {
		flex: 1;
		min-width: 0;
		height: 100%;
		padding: 28.9px 0 12.9px !important;
		border: 0 !important;
		border-radius: 0 !important;
		background: transparent !important;
		box-shadow: none !important;
		outline: none !important;
		color: #fff;
		font-family: inherit;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.28px;
		box-sizing: border-box;
		appearance: none;
	}
	.text-input::placeholder {
		color: #ffffff40;
	}

	.clear-btn {
		display: grid;
		place-items: center;
		flex: 0 0 32px;
		width: 32px;
		height: 32px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 12px;
		background: #ffffff1a;
		cursor: pointer;
		opacity: 0;
		visibility: hidden;
		transition:
			opacity var(--field-duration) var(--ease-field),
			visibility 0s var(--field-duration),
			background-color 160ms ease;
	}
	.filled .clear-btn {
		opacity: 1;
		visibility: visible;
		transition:
			opacity var(--field-duration) var(--ease-field),
			visibility 0s,
			background-color 160ms ease;
	}
	.clear-btn:hover {
		background: #ffffff2e;
	}
</style>
