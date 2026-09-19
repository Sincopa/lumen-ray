<script lang="ts">
	import { catalog } from '../../services';
	import { errorCode } from '../../services/errors';
	import { t } from '../../i18n';
	import IconButton from '../../components/IconButton.svelte';
	import chevronIcon from '../../../assets/chevron.svg';
	import checkIcon from '../../../assets/check.svg';

	interface Props {
		onClose: () => void;
		onSuccess?: () => void;
	}

	let { onClose, onSuccess }: Props = $props();

	let name = $state('');
	let source = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit() {
		if (busy) return;
		if (!source.trim()) {
			error = t('errors.invalidSubscription');
			return;
		}
		busy = true;
		error = '';
		try {
			await catalog.importSubscription(
				source.trim(),
				name.trim() || t('common.name'),
			);
			source = '';
			name = '';
			onSuccess?.();
			onClose();
		} catch (e) {
			error = t('errors.' + errorCode(e));
		} finally {
			busy = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			void submit();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<section
	class="add-profile-view"
	aria-label={t('subscriptions.addProfile') || 'Добавление профиля'}
>
	<div class="form-container">
		<div class="field-group">
			<label class="field-label" for="profile-name">
				{t('common.name')}
			</label>
			<div class="input-pill">
				<input
					id="profile-name"
					class="text-input"
					type="text"
					bind:value={name}
					placeholder={t('common.name')}
					autocomplete="off"
					maxlength="80"
				/>
			</div>
		</div>

		<div class="field-group">
			<label class="field-label" for="profile-domain-link">
				{t('subscriptions.domainLink') || 'Ссылка на домен'}
			</label>
			<div class="input-pill" class:active-pill={source.length > 0}>
				<input
					id="profile-domain-link"
					class="text-input"
					type="text"
					bind:value={source}
					placeholder={t('subscriptions.domainLinkPlaceholder') ||
						'Добавьте ссылку на подписку'}
					autocomplete="off"
					spellcheck="false"
					required
				/>
			</div>
		</div>

		{#if error}
			<p class="error-msg" role="alert">{error}</p>
		{/if}
	</div>

	<div class="action-bar">
		<IconButton
			icon={chevronIcon}
			ariaLabel={t('common.close')}
			size="lg"
			variant="surface"
			class="back-round"
			onclick={onClose}
		/>
		<IconButton
			icon={checkIcon}
			ariaLabel={t('common.save')}
			size="lg"
			variant="primary"
			disabled={busy}
			onclick={submit}
		/>
	</div>
</section>

<style>
	.add-profile-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		flex: 1;
		min-height: 0;
		justify-content: space-between;
		box-sizing: border-box;
		padding-top: 12px;
		position: relative;
	}

	.form-container {
		display: flex;
		flex-direction: column;
		gap: 24px;
		width: 100%;
	}

	.field-group {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
	}

	.field-label {
		font-size: 14px;
		font-weight: 500;
		color: #ffffff;
		letter-spacing: -0.2px;
	}

	.input-pill {
		width: 100%;
		height: 60px;
		border-radius: 24px;
		background: rgba(0, 0, 0, 0.2);
		border: 1px solid transparent;
		-webkit-backdrop-filter: blur(14px);
		backdrop-filter: blur(14px);
		box-sizing: border-box;
		padding: 0 20px;
		display: flex;
		align-items: center;
		transition:
			border-color 180ms ease,
			box-shadow 180ms ease;
	}

	.input-pill:focus-within {
		border-color: rgba(255, 255, 255, 0.35);
		box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.12);
	}

	.text-input {
		width: 100%;
		height: 100%;
		background: transparent !important;
		border: none !important;
		outline: none !important;
		box-shadow: none !important;
		border-radius: 0 !important;
		font-size: 14px;
		font-family: inherit;
		color: #ffffff;
		padding: 0 !important;
		-webkit-appearance: none;
		appearance: none;
	}

	.text-input::placeholder {
		color: rgba(255, 255, 255, 0.45);
	}

	.error-msg {
		margin: 0;
		padding: 8px 12px;
		background: rgba(212, 42, 42, 0.2);
		border: none;
		border-radius: 16px;
		color: #ff8e8e;
		font-size: 13px;
		text-align: center;
	}

	.action-bar {
		display: flex;
		width: 100%;
		justify-content: space-between;
		align-items: center;
		flex-shrink: 0;
		padding: 0 4px;
		margin-bottom: 33px;
	}

	:global(.back-round .btn-icon) {
		transform: rotate(90deg) !important;
	}
</style>
