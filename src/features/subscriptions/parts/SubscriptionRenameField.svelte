<script lang="ts">
	import closeBoldIcon from '../../../assets/icons/close.svg';
	import checkIcon from '../../../assets/icons/check.svg';
	import pencilIcon from '../../../assets/icons/edit.svg';
	import { t } from '../../../lib/i18n';

	interface Props {
		name: string;
		inputEl?: HTMLInputElement | null;
		onSave: () => void;
		onCancel: () => void;
	}
	let {
		name = $bindable(),
		inputEl = $bindable(null),
		onSave,
		onCancel,
	}: Props = $props();
</script>

<div
	class="card-rename-inline"
	role="presentation"
	onclick={(e) => e.stopPropagation()}
	onkeydown={(e) => e.stopPropagation()}
>
	<img src={pencilIcon} alt="" class="rename-icon" />
	<input
		bind:this={inputEl}
		type="text"
		class="rename-input"
		bind:value={name}
		maxlength="80"
		autocomplete="off"
		spellcheck="false"
		onkeydown={(e) => {
			if (e.key === 'Enter') {
				e.preventDefault();
				onSave();
			} else if (e.key === 'Escape') {
				e.preventDefault();
				onCancel();
			}
		}}
	/>
	<button
		type="button"
		class="rename-action-btn check"
		onclick={onSave}
		aria-label={t('common.save')}
	>
		<img src={checkIcon} alt="" class="action-svg" />
	</button>
	<button
		type="button"
		class="rename-action-btn cancel"
		onclick={onCancel}
		aria-label={t('common.close')}
	>
		<img src={closeBoldIcon} alt="" class="action-svg" />
	</button>
</div>

<style>
	/* Figma "profile edit": the pencil, the name in 16px Medium, then check
	   and cross, all 28px white glyphs on the card itself. */
	.card-rename-inline {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		height: 28px;
		box-sizing: border-box;
	}
	.rename-icon {
		width: 28px;
		height: 28px;
		flex-shrink: 0;
	}
	.rename-input {
		flex: 1;
		min-width: 0;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		font-size: 16px;
		font-weight: 500;
		color: #ffffff !important;
		padding: 0 !important;
		letter-spacing: 0;
	}
	.rename-action-btn {
		width: 28px;
		height: 28px;
		min-height: 0;
		border-radius: 50%;
		background: transparent;
		border: none;
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		padding: 0;
		flex-shrink: 0;
		transition:
			background 150ms ease,
			transform 150ms ease;
	}
	.rename-action-btn.check {
		margin-left: 10px;
	}
	.rename-action-btn.cancel {
		margin-left: -2px;
	}
	.rename-action-btn:hover {
		background: rgba(255, 255, 255, 0.12);
	}
	.rename-action-btn:active {
		transform: scale(0.92);
	}
	.action-svg {
		display: block;
		width: 100%;
		height: 100%;
	}
</style>
