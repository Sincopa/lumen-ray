<script lang="ts">
	import { t } from '../lib/i18n';
	import searchIcon from '../assets/search.svg';

	interface Props {
		value?: string;
		placeholder: string;
		onClose: () => void;
	}

	let { value = $bindable(''), placeholder, onClose }: Props = $props();
</script>

<div class="search-field">
	<img src={searchIcon} alt="" />
	<input
		type="search"
		bind:value
		aria-label={t('common.search')}
		{placeholder}
	/>
	<button
		type="button"
		aria-label={t('common.close')}
		onclick={() => {
			value = '';
			onClose();
		}}
	>
		×
	</button>
</div>

<style>
	.search-field {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-shrink: 0;
		height: 48px;
		padding: 0 7px 0 16px;
		border: 1px solid transparent;
		border-radius: 24px;
		background: #00000038;
		backdrop-filter: blur(14px);
		transition: border-color 180ms ease;
	}

	.search-field:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
	}

	img {
		width: 18px;
		height: 18px;
		filter: brightness(0) invert(1);
		opacity: 0.65;
	}

	input[type='search'] {
		flex: 1;
		width: 0 !important;
		min-width: 0;
		min-height: 0 !important;
		border: 0 !important;
		outline: 0;
		padding: 0 !important;
		background: transparent !important;
		font-size: 14px;
	}

	input::placeholder {
		color: #ffffff80;
	}

	input:focus-visible {
		outline: none !important;
	}

	input::-webkit-search-cancel-button {
		display: none;
	}

	button {
		display: grid;
		place-items: center;
		width: 34px;
		height: 34px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: #ffffff12;
		color: #ffffffb8;
		font-size: 22px;
		line-height: 1;
	}

	button:hover {
		background: #ffffff24;
	}
</style>
