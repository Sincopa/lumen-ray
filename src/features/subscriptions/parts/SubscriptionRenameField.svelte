<script lang="ts">
	import closeBoldIcon from '../../../assets/close-bold.svg';
	import checkIcon from '../../../assets/check.svg';
	import pencilIcon from '../../../assets/pencil.svg';
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
	/* A lighter pill, so the title visibly unrolls into a field. */
	.card-rename-inline {
		display: flex;
		align-items: center;
		gap: 10px;
		width: calc(100% + 18px);
		height: 44px;
		margin: 0 -8px 0 -10px;
		padding: 0 4px 0 3px;
		border-radius: 999px;
		background: #ffffff0d;
		box-sizing: border-box;
	}
	.rename-icon {
		width: 28px;
		height: 28px;
		flex-shrink: 0;
	}
	.rename-input {
		flex: 1;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		font-size: 16px;
		font-weight: 400;
		color: #ffffff !important;
		padding: 0 !important;
		letter-spacing: 0;
	}
	.rename-action-btn {
		width: 28px;
		height: 28px;
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
	.rename-action-btn:hover {
		background: rgba(255, 255, 255, 0.12);
		transform: scale(1.08);
	}
	.rename-action-btn.check:hover {
		background: rgba(134, 225, 111, 0.2);
	}
	.rename-action-btn.cancel:hover {
		background: rgba(243, 57, 57, 0.2);
	}
	.rename-action-btn.check {
		width: 34px;
		height: 34px;
		margin: -3px 0;
	}
	.action-svg {
		display: block;
		width: 100%;
		height: 100%;
		opacity: 0.85;
	}
	.rename-action-btn.check .action-svg {
		filter: brightness(0) invert(1);
	}
	.rename-action-btn:hover .action-svg {
		opacity: 1;
	}
</style>
