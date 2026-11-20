<script lang="ts">
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';
	import IconButton from '../../components/IconButton.svelte';
	import DesignTextField from '../../components/DesignTextField.svelte';
	import backIcon from '../../assets/icons/back.svg';
	import checkIcon from '../../assets/icons/check-dark.svg';
	import chevronIcon from '../../assets/icons/chevron-down-thin.svg';
	import { reveal } from '../../lib/motion';

	interface Props {
		onClose: () => void;
		onSuccess?: () => void;
		/** Link handed over by a `lumen://` deep link. */
		initialSource?: string;
	}

	let { onClose, onSuccess, initialSource = '' }: Props = $props();

	let name = $state('');
	let source = $state('');
	// Figma "add profile": the name hides under "Дополнительные настройки".
	let advanced = $state(false);

	// The field shows an https:// prefix, so it is left out of the value.
	const stripHttps = (value: string) => value.replace(/^\s*https:\/\//i, '');

	$effect(() => {
		if (!initialSource) return;
		source = stripHttps(initialSource);
	});
	let error = $state('');
	let busy = $state(false);

	async function submit() {
		if (busy) return;
		let cleanSource = source
			.trim()
			.replace(/^\uFEFF/, '')
			.replace(/^["'<]|["'>]$/g, '')
			.trim();
		if (!cleanSource) {
			error = t('errors.invalidSubscription');
			return;
		}
		if (
			!cleanSource.includes('://') &&
			!cleanSource.startsWith('{') &&
			!cleanSource.startsWith('[') &&
			!cleanSource.includes('proxies:') &&
			cleanSource.includes('.') &&
			!/\s/.test(cleanSource)
		) {
			cleanSource = `https://${cleanSource}`;
		}
		busy = true;
		error = '';
		try {
			await catalog.importSubscription(cleanSource, name.trim() || undefined);
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

<section class="add-profile-view" aria-label={t('subscriptions.addProfile')}>
	<div class="form-container">
		<h2 class="screen-title">
			<span class="title-lead">{t('subscriptions.addTitleLead')}</span>
			<span>{t('subscriptions.addTitle')}</span>
		</h2>
		<DesignTextField
			id="profile-domain-link"
			label={t('subscriptions.profileLink')}
			prefix="https://"
			bind:value={source}
			invalid={!!error}
			oninput={() => {
				source = stripHttps(source);
				error = '';
			}}
		/>
		{#if error}
			<p class="error-msg" role="alert">{error}</p>
		{/if}
		<button
			type="button"
			class="advanced-toggle"
			aria-expanded={advanced}
			onclick={() => (advanced = !advanced)}
		>
			<span>{t('subscriptions.advanced')}</span>
			<img
				src={chevronIcon}
				alt=""
				width="24"
				height="24"
				class:collapsed={!advanced}
			/>
		</button>
		{#if advanced}
			<div class="advanced-fields" transition:reveal>
				<DesignTextField
					id="profile-name"
					label={t('common.name')}
					placeholder={t('subscriptions.namePlaceholder')}
					bind:value={name}
					maxlength={80}
					spellcheck
				/>
			</div>
		{/if}
	</div>

	<div class="design-action-bar">
		<IconButton
			design
			icon={backIcon}
			ariaLabel={t('common.back')}
			size="lg"
			variant="surface"
			onclick={onClose}
		/>
		<IconButton
			design
			icon={checkIcon}
			ariaLabel={t('common.save')}
			size="lg"
			variant="primary"
			disabled={busy || !source.trim()}
			onclick={submit}
		/>
	</div>
</section>

<style>
	.add-profile-view {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		height: 100%;
		min-height: 0;
		padding-top: 16px;
		box-sizing: border-box;
	}
	/* Title at y=88, the link 24px under it, the toggle 16px under the link. */
	.form-container {
		display: flex;
		flex-direction: column;
		gap: 16px;
		width: 100%;
		padding: 0 9px 96px;
		overflow-y: auto;
		box-sizing: border-box;
	}
	.form-container > :global(.screen-title) {
		margin-bottom: 8px;
		padding: 0;
	}
	/* "Дополнительные настройки" and the thin arrow, both at 50%. */
	.advanced-toggle {
		display: flex;
		align-items: center;
		align-self: flex-start;
		height: 24px;
		min-height: 0;
		margin-left: 27px;
		padding: 0;
		border: 0;
		background: none;
		color: #ffffff80;
		font-size: 12px;
		letter-spacing: -0.24px;
		transition: color 160ms ease;
	}
	.advanced-toggle img {
		opacity: 0.5;
		transition:
			transform 220ms var(--ease-out),
			opacity 160ms ease;
	}
	.advanced-toggle img.collapsed {
		transform: rotate(180deg);
	}
	@media (hover: hover) and (pointer: fine) {
		.advanced-toggle:hover {
			color: #ffffffcc;
		}
		.advanced-toggle:hover img {
			opacity: 0.8;
		}
	}
	/* 12px under the toggle (the column gap is 16). */
	.advanced-fields {
		display: flex;
		flex-direction: column;
		margin-top: -4px;
	}
	/* Provider errors are red text under the field. */
	.error-msg {
		margin: -6px 0 0;
		padding: 0 8px;
		color: #ff6c67;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.2px;
	}
</style>
