<script lang="ts">
	import { onDestroy } from 'svelte';
	import copyIcon from '../../../assets/copy.svg';
	import backIcon from '../../../assets/back.svg';
	import { t } from '../../../lib/i18n';
	import type { Subscription } from '../../../lib/services/catalog';
	import DesignOverlay from '../../../components/DesignOverlay.svelte';
	import IconButton from '../../../components/IconButton.svelte';
	import MaterialIcon from '../../../components/MaterialIcon.svelte';

	interface Props {
		subscription: Subscription;
		source: string;
		error: string;
		onClose: () => void;
	}
	let { subscription, source, error, onClose }: Props = $props();

	let copyState = $state<'idle' | 'copied' | 'failed'>('idle');
	let copyResetTimer: ReturnType<typeof setTimeout> | undefined;
	onDestroy(() => {
		if (copyResetTimer) clearTimeout(copyResetTimer);
	});

	async function copySource() {
		if (!source) return;
		copyState = 'idle';
		try {
			if (navigator.clipboard?.writeText) {
				await navigator.clipboard.writeText(source);
			} else {
				const textarea = document.createElement('textarea');
				textarea.value = source;
				textarea.style.position = 'fixed';
				textarea.style.opacity = '0';
				document.body.appendChild(textarea);
				textarea.select();
				const copied = document.execCommand('copy');
				textarea.remove();
				if (!copied) throw new Error('Clipboard unavailable');
			}
			copyState = 'copied';
			if (copyResetTimer) clearTimeout(copyResetTimer);
			copyResetTimer = setTimeout(() => {
				copyState = 'idle';
			}, 1800);
		} catch {
			copyState = 'failed';
		}
	}
</script>

<DesignOverlay
	labelledby="share-title"
	describedby="share-hint"
	onDismiss={onClose}
>
	<div class="share-content">
		<div class="share-copy">
			<h2 id="share-title">
				{t('subscriptions.shareTitle', { name: subscription.name })}
			</h2>
			<p id="share-hint" class="share-hint">
				{t('subscriptions.shareHint')}
			</p>
		</div>
		{#if source}
			<div class="source-field">
				<input
					readonly
					value={source}
					aria-label={t('subscriptions.source')}
					onfocus={(event) => event.currentTarget.select()}
				/>
				<button
					class="copy-icon-button"
					class:copied={copyState === 'copied'}
					type="button"
					onclick={copySource}
					aria-label={copyState === 'copied'
						? t('common.copied')
						: t('common.copy')}
					title={copyState === 'copied' ? t('common.copied') : t('common.copy')}
				>
					<span class="copy-icon">
						{#if copyState === 'copied'}
							<MaterialIcon name="check" size={20} />
						{:else}
							<img src={copyIcon} alt="" width="34" height="34" />
						{/if}
					</span>
				</button>
			</div>
			{#if copyState === 'failed'}
				<p class="copy-error" role="alert">{t('subscriptions.copyFailed')}</p>
			{/if}
		{:else if error}
			<p class="share-error" role="alert">{error}</p>
		{:else}
			<p class="share-loading">{t('common.loading')}</p>
		{/if}
	</div>
	{#snippet footer()}
		<IconButton
			design
			icon={backIcon}
			ariaLabel={t('common.back')}
			size="lg"
			variant="surface"
			onclick={onClose}
		/>
	{/snippet}
</DesignOverlay>

<style>
	/* Figma "profile share": left-aligned copy, the link field 183px below it. */
	.share-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
		text-align: left;
	}
	.share-copy {
		display: flex;
		flex-direction: column;
		gap: 16px;
		margin-bottom: 171px;
	}
	.share-copy h2 {
		margin: 0;
		font-size: 32px;
		font-weight: 400;
		line-height: 32px;
	}
	.share-hint,
	.share-loading {
		margin: 0;
		color: #ffffff80;
		font-size: 16px;
		line-height: 20.8px;
		letter-spacing: -0.32px;
		text-box-trim: trim-both;
		text-box-edge: cap alphabetic;
	}
	.source-field {
		display: flex;
		align-items: center;
		gap: 24px;
		width: 100%;
		height: 64px;
		padding: 7px 7px 7px 24px;
		border-radius: 123px;
		background: #ffffff0d;
		box-shadow: var(--design-edge);
		box-sizing: border-box;
	}
	.source-field input {
		flex: 1;
		min-width: 0;
		padding: 0;
		border: 0;
		background: transparent;
		color: #fff;
		font-size: 18px;
		line-height: 18px;
		letter-spacing: -0.36px;
		text-overflow: ellipsis;
		outline: none;
	}
	.copy-icon-button {
		display: grid;
		place-items: center;
		flex: 0 0 50px;
		width: 50px;
		height: 50px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		/* 5% glass with a light falling from the top. */
		background: linear-gradient(#ffffff1a, #99999900), #ffffff0d;
		color: #fff;
		transition:
			transform 180ms var(--ease-spring),
			background-color 220ms ease,
			color 220ms ease;
	}
	.copy-icon-button:hover {
		background: linear-gradient(#ffffff1a, #99999900), #ffffff1a;
	}
	.copy-icon-button:active {
		transform: scale(0.92);
	}
	.copy-icon-button.copied {
		background: var(--color-success);
		color: #10210d;
	}
	.copy-icon {
		display: grid;
		place-items: center;
	}
	.copy-icon-button.copied .copy-icon {
		animation: copy-confirm 420ms var(--ease-spring);
	}
	@keyframes copy-confirm {
		0% {
			transform: scale(0.55) rotate(-18deg);
			opacity: 0;
		}
		55% {
			transform: scale(1.18) rotate(3deg);
			opacity: 1;
		}
		100% {
			transform: scale(1) rotate(0);
			opacity: 1;
		}
	}
	.copy-error,
	.share-error {
		margin: 0;
		border-radius: 14px;
		background: rgba(243, 57, 57, 0.12);
		padding: 10px 12px;
		font-size: 12px;
		line-height: 1.4;
	}
	@media (prefers-reduced-motion: reduce) {
		.copy-icon-button.copied .copy-icon {
			animation: none;
		}
	}
</style>
