<script lang="ts">
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
	<div class="input-pill" class:invalid>
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
				<svg
					width="32"
					height="32"
					viewBox="0 0 32 32"
					fill="none"
					aria-hidden="true"
				>
					<path
						d="m10.6 10.6 10.8 10.8m0-10.8L10.6 21.4"
						stroke="#fff"
						stroke-width="1.6"
						stroke-linecap="round"
					/>
				</svg>
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
		letter-spacing: -0.3px;
	}
	.input-pill {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: 60px;
		padding: 0 14px 0 24px;
		border-radius: 24px;
		background: var(--design-panel-open);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		box-sizing: border-box;
		transition:
			background-color 180ms ease,
			box-shadow 180ms ease;
	}
	.input-pill:focus-within {
		box-shadow: inset 0 0 0 1px #ffffff1f;
	}
	/* An invalid value tints the whole pill red. */
	.input-pill.invalid {
		background: #ff6c6726;
	}
	.input-prefix {
		flex-shrink: 0;
		color: #ffffff4d;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
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
		letter-spacing: -0.3px;
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
