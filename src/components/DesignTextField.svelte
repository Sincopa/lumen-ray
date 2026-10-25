<script lang="ts">
	import clearIcon from '../assets/clear.svg';
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
</script>

<!-- Labelled input pill. -->
<div class="design-field">
	<label class="field-label" for={id}>{label}</label>
	<div class="input-pill" class:filled={!!value} class:invalid>
		{#if prefix}<span class="input-prefix">{prefix}</span>{/if}
		<input
			bind:this={input}
			{id}
			class="text-input"
			type="text"
			bind:value
			{oninput}
			{onpaste}
			{placeholder}
			{maxlength}
			{spellcheck}
			autocomplete="off"
			aria-invalid={invalid}
		/>
		{#if value}
			<button
				type="button"
				class="clear-btn"
				aria-label={t('rules.clear')}
				onclick={() => {
					value = '';
					oninput?.();
					input?.focus();
				}}
			>
				<img src={clearIcon} alt="" width="32" height="32" />
			</button>
		{/if}
	</div>
</div>

<style>
	.design-field {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
	}
	.field-label {
		padding: 0 8px;
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		line-height: 18.2px;
		letter-spacing: -0.28px;
		/* Figma measures the 12px gap from the cap height. */
		text-box-trim: trim-both;
		text-box-edge: cap alphabetic;
	}
	.input-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 60px;
		padding: 0 18px 0 24px;
		border-radius: 24px;
		background: var(--design-panel);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		box-sizing: border-box;
		transition:
			background-color 180ms ease,
			box-shadow 180ms ease;
	}
	/* A filled field lifts to 5% white over a 20% black base. */
	.input-pill.filled {
		background: linear-gradient(#ffffff0d, #ffffff0d), #00000033;
	}
	.input-pill:focus-within {
		box-shadow: inset 0 0 0 1px #ffffff1f;
	}
	/* An invalid value tints the whole pill red. */
	.input-pill.invalid {
		background: linear-gradient(#ff6c671f, #ff6c671f), #00000033;
	}
	.input-prefix {
		flex-shrink: 0;
		color: #ffffff40;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.24px;
		user-select: none;
	}
	.text-input {
		flex: 1;
		min-width: 0;
		height: 100%;
		padding: 0 !important;
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
		appearance: none;
	}
	.text-input::placeholder {
		color: #ffffff4d;
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
		transition: background-color 160ms ease;
	}
	.clear-btn:hover {
		background: #ffffff2e;
	}
</style>
